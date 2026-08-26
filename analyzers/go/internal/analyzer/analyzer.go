package analyzer

import (
	"encoding/json"
	"fmt"
	"go/ast"
	"go/token"
	"go/types"
	"os"
	"path/filepath"
	"sort"
	"strings"

	"golang.org/x/tools/go/packages"
)

// SemanticEntity is a node in the scope tree produced by the analyzer.
type SemanticEntity struct {
	ID        string      `json:"id"`
	Kind      string      `json:"kind"`
	Name      string      `json:"name,omitempty"`
	Label     string      `json:"label,omitempty"`
	Symbol    string      `json:"symbol,omitempty"`
	Callee    string      `json:"callee,omitempty"`
	Condition string      `json:"condition,omitempty"`
	Source    SourceRange `json:"source"`
	ParentID  string      `json:"parent_id,omitempty"`
	Children  []string    `json:"children,omitempty"`
}

// UnifiedCodeModel is the JSON contract shared by language analyzers.
type UnifiedCodeModel struct {
	Language string             `json:"language"`
	Packages []Package          `json:"packages"`
	Symbols  []Symbol           `json:"symbols"`
	Calls    []Call             `json:"calls"`
	Effects  []ExternalEffect   `json:"effects"`
	Entities []SemanticEntity   `json:"entities,omitempty"`
}

type Package struct {
	ID    string   `json:"id"`
	Name  string   `json:"name"`
	Dir   string   `json:"dir"`
	Files []string `json:"files"`
}

type Symbol struct {
	ID         string      `json:"id"`
	Kind       string      `json:"kind"`
	Name       string      `json:"name"`
	Package    string      `json:"package"`
	Source     SourceRange `json:"source"`
	Signature  string      `json:"signature"`
	IsExported bool        `json:"is_exported"`
	IsAsync    bool        `json:"is_async"`
}

type SourceRange struct {
	File      string `json:"file"`
	StartLine int    `json:"start_line"`
	EndLine   int    `json:"end_line"`
}

type Call struct {
	From   string      `json:"from"`
	To     string      `json:"to"`
	Source SourceRange `json:"source"`
}

type ExternalEffect struct {
	Symbol string      `json:"symbol"`
	Kind   string      `json:"kind"`
	Detail string      `json:"detail"`
	Source SourceRange `json:"source"`
}

// Analyze loads and analyzes Go packages below dir.
func Analyze(dir string) (UnifiedCodeModel, error) {
	absDir, err := filepath.Abs(dir)
	if err != nil {
		return UnifiedCodeModel{}, fmt.Errorf("resolve project directory: %w", err)
	}

	fset := token.NewFileSet()
	cfg := &packages.Config{
		Dir:  absDir,
		Mode: packages.NeedName | packages.NeedFiles | packages.NeedSyntax | packages.NeedTypes | packages.NeedTypesInfo | packages.NeedImports | packages.NeedDeps,
		Fset: fset,
	}
	loaded, err := packages.Load(cfg, "./...")
	if err != nil {
		return UnifiedCodeModel{}, fmt.Errorf("load packages: %w", err)
	}
	if len(loaded) == 0 {
		return UnifiedCodeModel{}, fmt.Errorf("no Go packages found in %s", absDir)
	}

	projectPackages := make([]*packages.Package, 0, len(loaded))
	for _, pkg := range loaded {
		if len(pkg.Errors) > 0 {
			return UnifiedCodeModel{}, packageErrors(pkg)
		}
		if isProjectPackage(pkg, absDir) {
			projectPackages = append(projectPackages, pkg)
		}
	}
	if len(projectPackages) == 0 {
		return UnifiedCodeModel{}, fmt.Errorf("no project Go packages found in %s", absDir)
	}

	sort.Slice(projectPackages, func(i, j int) bool { return projectPackages[i].PkgPath < projectPackages[j].PkgPath })
	projectIDs := make(map[string]struct{}, len(projectPackages))
	for _, pkg := range projectPackages {
		projectIDs[pkg.PkgPath] = struct{}{}
	}

	model := UnifiedCodeModel{
		Language: "go",
		Packages: make([]Package, 0, len(projectPackages)),
		Symbols:  make([]Symbol, 0),
		Calls:    make([]Call, 0),
		Effects:  make([]ExternalEffect, 0),
	}

	symbolsByObject := make(map[types.Object]string)
	symbolsByID := make(map[string]struct{})
	functions := make(map[string]ast.Node)
	for _, pkg := range projectPackages {
		model.Packages = append(model.Packages, packageModel(pkg, absDir))
		collectSymbols(pkg, absDir, symbolsByObject, symbolsByID, &model, functions)
	}

	for _, pkg := range projectPackages {
		collectCallsAndEffects(pkg, absDir, projectIDs, symbolsByObject, symbolsByID, functions, &model)
		collectEntities(pkg, absDir, projectIDs, symbolsByObject, &model.Entities)
	}

	sortModel(&model)
	return model, nil
}

func packageErrors(pkg *packages.Package) error {
	messages := make([]string, 0, len(pkg.Errors))
	for _, packageError := range pkg.Errors {
		messages = append(messages, packageError.Error())
	}
	return fmt.Errorf("package %s: %s", pkg.PkgPath, strings.Join(messages, "; "))
}

func isProjectPackage(pkg *packages.Package, root string) bool {
	for _, file := range append(append([]string{}, pkg.GoFiles...), pkg.CompiledGoFiles...) {
		if isWithinRoot(file, root) {
			return true
		}
	}
	return false
}

func isWithinRoot(path string, root string) bool {
	relative, err := filepath.Rel(root, path)
	if err != nil {
		return false
	}
	return relative != ".." && !strings.HasPrefix(relative, ".."+string(filepath.Separator)) && !filepath.IsAbs(relative)
}

func addPackageTestFiles(pkg *packages.Package, root string, seen map[string]struct{}, files *[]string) {
	if len(pkg.GoFiles) == 0 {
		return
	}
	entries, err := os.ReadDir(filepath.Dir(pkg.GoFiles[0]))
	if err != nil {
		return
	}
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), "_test.go") {
			continue
		}
		path := filepath.Join(filepath.Dir(pkg.GoFiles[0]), entry.Name())
		if !isWithinRoot(path, root) {
			continue
		}
		relative := relativePath(root, path)
		if _, ok := seen[relative]; ok {
			continue
		}
		seen[relative] = struct{}{}
		*files = append(*files, relative)
	}
}

func packageModel(pkg *packages.Package, root string) Package {
	files := make([]string, 0, len(pkg.GoFiles)+len(pkg.CompiledGoFiles))
	seen := make(map[string]struct{})
	for _, file := range append(append([]string{}, pkg.GoFiles...), pkg.CompiledGoFiles...) {
		if !strings.HasSuffix(file, ".go") || !isWithinRoot(file, root) {
			continue
		}
		relative := relativePath(root, file)
		if _, ok := seen[relative]; ok {
			continue
		}
		seen[relative] = struct{}{}
		files = append(files, relative)
	}
	addPackageTestFiles(pkg, root, seen, &files)
	sort.Strings(files)
	dir := ""
	if len(files) > 0 {
		dir = filepath.ToSlash(filepath.Dir(files[0]))
		if dir == "." {
			dir = ""
		}
	}
	return Package{ID: pkg.PkgPath, Name: pkg.Name, Dir: dir, Files: files}
}

func collectSymbols(pkg *packages.Package, root string, symbolsByObject map[types.Object]string, symbolsByID map[string]struct{}, model *UnifiedCodeModel, functions map[string]ast.Node) {
	for fileIndex, file := range pkg.Syntax {
		if fileIndex >= len(pkg.GoFiles) {
			break
		}
		filePath := pkg.GoFiles[fileIndex]
		ast.Inspect(file, func(node ast.Node) bool {
			switch declaration := node.(type) {
			case *ast.FuncDecl:
				object, ok := pkg.TypesInfo.Defs[declaration.Name].(*types.Func)
				if !ok || object == nil {
					return false
				}
				id := functionID(object)
				kind := "function"
				if declaration.Recv != nil {
					kind = "method"
				}
				addSymbol(Symbol{
					ID:         id,
					Kind:       kind,
					Name:       object.Name(),
					Package:    pkg.PkgPath,
					Source:     sourceRange(pkg.Fset, root, filePath, declaration),
					Signature:  typeString(pkg, object.Type()),
					IsExported: object.Exported(),
					IsAsync:    false,
				}, object, symbolsByObject, symbolsByID, model)
				functions[id] = declaration
				return false
			case *ast.TypeSpec:
				object, ok := pkg.TypesInfo.Defs[declaration.Name]
				if !ok || object == nil {
					return false
				}
				kind := ""
				switch declaration.Type.(type) {
				case *ast.StructType:
					kind = "type"
				case *ast.InterfaceType:
					kind = "interface"
				}
				if kind == "" {
					return false
				}
				addSymbol(Symbol{
					ID:         packageSymbolID(pkg.PkgPath, object.Name()),
					Kind:       kind,
					Name:       object.Name(),
					Package:    pkg.PkgPath,
					Source:     sourceRange(pkg.Fset, root, filePath, declaration),
					Signature:  typeString(pkg, object.Type()),
					IsExported: object.Exported(),
					IsAsync:    false,
				}, object, symbolsByObject, symbolsByID, model)
				return false
			case *ast.GenDecl:
				if declaration.Tok != token.VAR && declaration.Tok != token.CONST {
					return true
				}
				for _, specification := range declaration.Specs {
					valueSpec, ok := specification.(*ast.ValueSpec)
					if !ok {
						continue
					}
					for _, name := range valueSpec.Names {
						object := pkg.TypesInfo.Defs[name]
						if object == nil {
							continue
						}
						addSymbol(Symbol{
							ID:         packageSymbolID(pkg.PkgPath, object.Name()),
							Kind:       "variable",
							Name:       object.Name(),
							Package:    pkg.PkgPath,
							Source:     sourceRange(pkg.Fset, root, filePath, valueSpec),
							Signature:  typeString(pkg, object.Type()),
							IsExported: object.Exported(),
							IsAsync:    false,
						}, object, symbolsByObject, symbolsByID, model)
					}
				}
			}
			return true
		})
	}
}

func addSymbol(symbol Symbol, object types.Object, symbolsByObject map[types.Object]string, symbolsByID map[string]struct{}, model *UnifiedCodeModel) {
	if _, exists := symbolsByID[symbol.ID]; exists {
		return
	}
	symbolsByID[symbol.ID] = struct{}{}
	symbolsByObject[object] = symbol.ID
	model.Symbols = append(model.Symbols, symbol)
}

func collectCallsAndEffects(pkg *packages.Package, root string, projectIDs map[string]struct{}, symbolsByObject map[types.Object]string, symbolsByID map[string]struct{}, functions map[string]ast.Node, model *UnifiedCodeModel) {
	for fileIndex, file := range pkg.Syntax {
		if fileIndex >= len(pkg.GoFiles) {
			break
		}
		filePath := pkg.GoFiles[fileIndex]
		ast.Inspect(file, func(node ast.Node) bool {
			declaration, ok := node.(*ast.FuncDecl)
			if !ok {
				return true
			}
			object, ok := pkg.TypesInfo.Defs[declaration.Name].(*types.Func)
			if !ok || object == nil || declaration.Body == nil {
				return false
			}
			from := functionID(object)
			ast.Inspect(declaration.Body, func(bodyNode ast.Node) bool {
				switch expression := bodyNode.(type) {
				case *ast.CallExpr:
					callee := calledObject(pkg.TypesInfo, expression.Fun)
					if callee == nil {
						return true
					}
					target := objectID(callee)
					if target == "" {
						return true
					}
					if _, isProject := projectIDs[calleePackagePath(callee)]; !isProject {
						target = externalObjectID(callee)
					}
					model.Calls = append(model.Calls, Call{From: from, To: target, Source: sourceRange(pkg.Fset, root, filePath, expression)})
					if kind, detail, hasEffect := callEffect(callee); hasEffect {
						model.Effects = append(model.Effects, ExternalEffect{Symbol: from, Kind: kind, Detail: detail, Source: sourceRange(pkg.Fset, root, filePath, expression)})
					}
					return true
				case *ast.SendStmt:
					model.Effects = append(model.Effects, ExternalEffect{Symbol: from, Kind: "queue", Detail: "channel send", Source: sourceRange(pkg.Fset, root, filePath, expression)})
				case *ast.GoStmt:
					model.Effects = append(model.Effects, ExternalEffect{Symbol: from, Kind: "other", Detail: "goroutine", Source: sourceRange(pkg.Fset, root, filePath, expression)})
				}
				return true
			})
			return false
		})
	}
}

func calledObject(info *types.Info, expression ast.Expr) types.Object {
	switch function := expression.(type) {
	case *ast.Ident:
		return info.Uses[function]
	case *ast.SelectorExpr:
		if selection := info.Selections[function]; selection != nil {
			return selection.Obj()
		}
		return info.Uses[function.Sel]
	default:
		return nil
	}
}

func callEffect(object types.Object) (string, string, bool) {
	pkgPath := calleePackagePath(object)
	name := object.Name()
	if pkgPath == "net/http" {
		return "network", "net/http." + name, true
	}
	if pkgPath == "database/sql" || strings.HasSuffix(pkgPath, "/database") || strings.HasSuffix(pkgPath, "/db") || strings.EqualFold(receiverName(object), "DB") {
		return "database", "database operation: " + name, true
	}
	if pkgPath == "os" && isFileOperation(name) {
		return "file_system", "os." + name, true
	}
	if pkgPath == "log" || pkgPath == "log/slog" {
		return "log", pkgPath + "." + name, true
	}
	return "", "", false
}

func isFileOperation(name string) bool {
	switch name {
	case "Chdir", "Chmod", "Chown", "Create", "Mkdir", "MkdirAll", "Open", "OpenFile", "ReadFile", "Remove", "RemoveAll", "Rename", "Truncate", "WriteFile":
		return true
	default:
		return false
	}
}

func functionID(function *types.Func) string {
	packagePath := calleePackagePath(function)
	if function.Signature().Recv() == nil {
		return packageSymbolID(packagePath, function.Name())
	}
	return packagePath + "." + receiverName(function) + "." + function.Name()
}

func objectID(object types.Object) string {
	switch typed := object.(type) {
	case *types.Func:
		return functionID(typed)
	case *types.TypeName:
		return packageSymbolID(calleePackagePath(typed), typed.Name())
	default:
		if object == nil || object.Pkg() == nil {
			return object.Name()
		}
		return packageSymbolID(object.Pkg().Path(), object.Name())
	}
}

func externalObjectID(object types.Object) string {
	if object == nil {
		return ""
	}
	return objectID(object)
}

func calleePackagePath(object types.Object) string {
	if object == nil || object.Pkg() == nil {
		return "builtin"
	}
	return object.Pkg().Path()
}

func receiverName(function types.Object) string {
	typed, ok := function.(*types.Func)
	if !ok || typed.Signature().Recv() == nil {
		return ""
	}
	return namedTypeName(typed.Signature().Recv().Type())
}

func namedTypeName(typ types.Type) string {
	if pointer, ok := typ.(*types.Pointer); ok {
		typ = pointer.Elem()
	}
	if named, ok := typ.(*types.Named); ok {
		return named.Obj().Name()
	}
	return ""
}

func packageSymbolID(packagePath string, name string) string {
	return packagePath + "." + name
}

func typeString(pkg *packages.Package, typ types.Type) string {
	return types.TypeString(typ, types.RelativeTo(pkg.Types))
}

func sourceRange(fset *token.FileSet, root string, filePath string, node ast.Node) SourceRange {
	start := fset.Position(node.Pos())
	end := fset.Position(node.End())
	return SourceRange{File: relativePath(root, filePath), StartLine: start.Line, EndLine: end.Line}
}

func relativePath(root string, path string) string {
	relative, err := filepath.Rel(root, path)
	if err != nil {
		return filepath.ToSlash(path)
	}
	return filepath.ToSlash(relative)
}

func sortModel(model *UnifiedCodeModel) {
	sort.Slice(model.Symbols, func(i, j int) bool { return model.Symbols[i].ID < model.Symbols[j].ID })
	sort.Slice(model.Calls, func(i, j int) bool {
		left, right := model.Calls[i], model.Calls[j]
		if left.From != right.From {
			return left.From < right.From
		}
		if left.Source.File != right.Source.File {
			return left.Source.File < right.Source.File
		}
		return left.Source.StartLine < right.Source.StartLine
	})
	sort.Slice(model.Effects, func(i, j int) bool {
		left, right := model.Effects[i], model.Effects[j]
		if left.Symbol != right.Symbol {
			return left.Symbol < right.Symbol
		}
		if left.Source.File != right.Source.File {
			return left.Source.File < right.Source.File
		}
		return left.Source.StartLine < right.Source.StartLine
	})
}

// MarshalJSON keeps the contract stable for callers and guarantees arrays are present.
func (model UnifiedCodeModel) MarshalJSON() ([]byte, error) {
	type alias UnifiedCodeModel
	return json.Marshal(alias(model))
}
