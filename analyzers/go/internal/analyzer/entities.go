package analyzer

import (
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

// collectEntities walks AST and emits a scope tree for every project file.
func collectEntities(
	pkg *packages.Package,
	root string,
	projectIDs map[string]struct{},
	symbolsByObject map[types.Object]string,
	entities *[]SemanticEntity,
) {
	for fileIndex, file := range pkg.Syntax {
		if fileIndex >= len(pkg.GoFiles) {
			break
		}
		filePath := pkg.GoFiles[fileIndex]
		relPath := relativePath(root, filePath)

		fileID := entityID("file", relPath, 0, "")
		*entities = append(*entities, SemanticEntity{
			ID:       fileID,
			Kind:     "file",
			Name:     filepath.Base(relPath),
			Source:   SourceRange{File: relPath, StartLine: 1, EndLine: fileEndLine(pkg.Fset, filePath, file)},
			Children: []string{},
		})

		for _, decl := range file.Decls {
			switch d := decl.(type) {
			case *ast.FuncDecl:
				object, ok := pkg.TypesInfo.Defs[d.Name].(*types.Func)
				if !ok || object == nil {
					continue
				}
				id := functionID(object)
				funcEntity := SemanticEntity{
					ID:       entityID("func", relPath, d.Pos(), d.Name.Name),
					Kind:     "function",
					Name:     d.Name.Name,
					Symbol:   id,
					Source:   sourceRange(pkg.Fset, root, filePath, d),
					ParentID: fileID,
					Children: []string{},
				}
				if d.Recv != nil {
					funcEntity.Kind = "method"
				}
				*entities = append(*entities, funcEntity)
				if d.Body != nil {
					walkStmtList(pkg, root, filePath, relPath, funcEntity.ID, d.Body.List, entities, projectIDs, symbolsByObject)
				}

			case *ast.GenDecl:
				for _, spec := range d.Specs {
					switch s := spec.(type) {
					case *ast.TypeSpec:
						object := pkg.TypesInfo.Defs[s.Name]
						if object == nil {
							continue
						}
						var kind string
						switch s.Type.(type) {
						case *ast.StructType:
							kind = "type"
						case *ast.InterfaceType:
							kind = "interface"
						default:
							kind = "type"
						}
						*entities = append(*entities, SemanticEntity{
							ID:       entityID(kind, relPath, s.Pos(), s.Name.Name),
							Kind:     kind,
							Name:     s.Name.Name,
							Symbol:   packageSymbolID(pkg.PkgPath, object.Name()),
							Source:   sourceRange(pkg.Fset, root, filePath, s),
							ParentID: fileID,
							Children: []string{},
						})
					case *ast.ValueSpec:
						for _, name := range s.Names {
							object := pkg.TypesInfo.Defs[name]
							if object == nil {
								continue
							}
							*entities = append(*entities, SemanticEntity{
								ID:       entityID("variable", relPath, name.Pos(), name.Name),
								Kind:     "variable",
								Name:     name.Name,
								Symbol:   packageSymbolID(pkg.PkgPath, object.Name()),
								Source:   sourceRange(pkg.Fset, root, filePath, name),
								ParentID: fileID,
								Children: []string{},
							})
						}
					}
				}
			}
		}
	}
	sort.Slice(*entities, func(i, j int) bool { return (*entities)[i].ID < (*entities)[j].ID })
}

func walkStmtList(
	pkg *packages.Package,
	root string,
	filePath string,
	relPath string,
	parentID string,
	stmts []ast.Stmt,
	entities *[]SemanticEntity,
	projectIDs map[string]struct{},
	symbolsByObject map[types.Object]string,
) {
	for _, stmt := range stmts {
		walkStmt(pkg, root, filePath, relPath, parentID, stmt, entities, projectIDs, symbolsByObject)
	}
}

func walkStmt(
	pkg *packages.Package,
	root string,
	filePath string,
	relPath string,
	parentID string,
	stmt ast.Stmt,
	entities *[]SemanticEntity,
	projectIDs map[string]struct{},
	symbolsByObject map[types.Object]string,
) {
	switch s := stmt.(type) {
	case *ast.ExprStmt:
		var calls []*ast.CallExpr
		findCalls(s.X, &calls)
		for _, call := range calls {
			addCallEntity(pkg, root, filePath, relPath, parentID, call, entities, projectIDs)
		}
	case *ast.AssignStmt:
		var calls []*ast.CallExpr
		for _, expr := range s.Rhs {
			findCalls(expr, &calls)
		}
		for _, call := range calls {
			addCallEntity(pkg, root, filePath, relPath, parentID, call, entities, projectIDs)
		}
	case *ast.IfStmt:
		ifEntity := SemanticEntity{
			ID:         entityID("if", relPath, s.Pos(), ""),
			Kind:       "if",
			Name:       "if",
			Condition:  exprText(pkg.Fset, filePath, s.Cond),
			Source:     sourceRange(pkg.Fset, root, filePath, s),
			ParentID:   parentID,
			Children:   []string{},
		}
		*entities = append(*entities, ifEntity)
		if s.Init != nil {
			walkStmt(pkg, root, filePath, relPath, ifEntity.ID, s.Init, entities, projectIDs, symbolsByObject)
		}
		walkStmtList(pkg, root, filePath, relPath, ifEntity.ID, s.Body.List, entities, projectIDs, symbolsByObject)
		if s.Else != nil {
			elseEntity := SemanticEntity{
				ID:       entityID("else", relPath, s.Else.Pos(), ""),
				Kind:     "else",
				Name:     "else",
				Source:   sourceRange(pkg.Fset, root, filePath, s.Else),
				ParentID: ifEntity.ID,
				Children: []string{},
			}
			*entities = append(*entities, elseEntity)
			walkStmt(pkg, root, filePath, relPath, elseEntity.ID, s.Else, entities, projectIDs, symbolsByObject)
		}
	case *ast.ForStmt:
		loopEntity := SemanticEntity{
			ID:         entityID("loop", relPath, s.Pos(), ""),
			Kind:       "loop",
			Name:       "for",
			Condition:  exprText(pkg.Fset, filePath, s.Cond),
			Source:     sourceRange(pkg.Fset, root, filePath, s),
			ParentID:   parentID,
			Children:   []string{},
		}
		*entities = append(*entities, loopEntity)
		if s.Init != nil {
			walkStmt(pkg, root, filePath, relPath, loopEntity.ID, s.Init, entities, projectIDs, symbolsByObject)
		}
		walkStmtList(pkg, root, filePath, relPath, loopEntity.ID, s.Body.List, entities, projectIDs, symbolsByObject)
		if s.Post != nil {
			walkStmt(pkg, root, filePath, relPath, loopEntity.ID, s.Post, entities, projectIDs, symbolsByObject)
		}
	case *ast.RangeStmt:
		loopEntity := SemanticEntity{
			ID:         entityID("loop", relPath, s.Pos(), ""),
			Kind:       "loop",
			Name:       "range",
			Condition:  exprText(pkg.Fset, filePath, s.X),
			Source:     sourceRange(pkg.Fset, root, filePath, s),
			ParentID:   parentID,
			Children:   []string{},
		}
		*entities = append(*entities, loopEntity)
		walkStmtList(pkg, root, filePath, relPath, loopEntity.ID, s.Body.List, entities, projectIDs, symbolsByObject)
	case *ast.BlockStmt:
		walkStmtList(pkg, root, filePath, relPath, parentID, s.List, entities, projectIDs, symbolsByObject)
	case *ast.ReturnStmt:
		*entities = append(*entities, SemanticEntity{
			ID:       entityID("return", relPath, s.Pos(), ""),
			Kind:     "return",
			Name:     "return",
			Source:   sourceRange(pkg.Fset, root, filePath, s),
			ParentID: parentID,
		})
	case *ast.SwitchStmt:
		switchEntity := SemanticEntity{
			ID:         entityID("switch", relPath, s.Pos(), ""),
			Kind:       "switch",
			Name:       "switch",
			Condition:  exprText(pkg.Fset, filePath, s.Tag),
			Source:     sourceRange(pkg.Fset, root, filePath, s),
			ParentID:   parentID,
			Children:   []string{},
		}
		*entities = append(*entities, switchEntity)
		if s.Init != nil {
			walkStmt(pkg, root, filePath, relPath, switchEntity.ID, s.Init, entities, projectIDs, symbolsByObject)
		}
		for _, bodyStmt := range s.Body.List {
			if cc, ok := bodyStmt.(*ast.CaseClause); ok {
				caseEntity := SemanticEntity{
					ID:       entityID("case", relPath, cc.Pos(), ""),
					Kind:     "case",
					Name:     "case",
					Source:   sourceRange(pkg.Fset, root, filePath, cc),
					ParentID: switchEntity.ID,
					Children: []string{},
				}
				*entities = append(*entities, caseEntity)
				walkStmtList(pkg, root, filePath, relPath, caseEntity.ID, cc.Body, entities, projectIDs, symbolsByObject)
			}
		}
	case *ast.TypeSwitchStmt:
		switchEntity := SemanticEntity{
			ID:       entityID("switch", relPath, s.Pos(), ""),
			Kind:     "switch",
			Name:     "type switch",
			Source:   sourceRange(pkg.Fset, root, filePath, s),
			ParentID: parentID,
			Children: []string{},
		}
		*entities = append(*entities, switchEntity)
		if s.Assign != nil {
			walkStmt(pkg, root, filePath, relPath, switchEntity.ID, s.Assign, entities, projectIDs, symbolsByObject)
		}
		for _, bodyStmt := range s.Body.List {
			if cc, ok := bodyStmt.(*ast.CaseClause); ok {
				caseEntity := SemanticEntity{
					ID:       entityID("case", relPath, cc.Pos(), ""),
					Kind:     "case",
					Name:     "case",
					Source:   sourceRange(pkg.Fset, root, filePath, cc),
					ParentID: switchEntity.ID,
					Children: []string{},
				}
				*entities = append(*entities, caseEntity)
				walkStmtList(pkg, root, filePath, relPath, caseEntity.ID, cc.Body, entities, projectIDs, symbolsByObject)
			}
		}
	}
}

func findCalls(expr ast.Expr, out *[]*ast.CallExpr) {
	switch e := expr.(type) {
	case *ast.CallExpr:
		*out = append(*out, e)
		for _, arg := range e.Args {
			findCalls(arg, out)
		}
		findCalls(e.Fun, out)
	case *ast.ParenExpr:
		findCalls(e.X, out)
	case *ast.UnaryExpr:
		findCalls(e.X, out)
	case *ast.BinaryExpr:
		findCalls(e.X, out)
		findCalls(e.Y, out)
	case *ast.SelectorExpr:
		findCalls(e.X, out)
	case *ast.IndexExpr:
		findCalls(e.X, out)
		findCalls(e.Index, out)
	case *ast.SliceExpr:
		findCalls(e.X, out)
		findCalls(e.Low, out)
		findCalls(e.High, out)
		findCalls(e.Max, out)
	case *ast.TypeAssertExpr:
		findCalls(e.X, out)
	case *ast.StarExpr:
		findCalls(e.X, out)
	case *ast.KeyValueExpr:
		findCalls(e.Key, out)
		findCalls(e.Value, out)
	case *ast.CompositeLit:
		for _, elt := range e.Elts {
			findCalls(elt, out)
		}
	}
}

func addCallEntity(
	pkg *packages.Package,
	root string,
	filePath string,
	relPath string,
	parentID string,
	call *ast.CallExpr,
	entities *[]SemanticEntity,
	projectIDs map[string]struct{},
) {
	callee := calledObject(pkg.TypesInfo, call.Fun)
	if callee == nil {
		return
	}
	target := objectID(callee)
	isProject := false
	if target != "" {
		if _, ok := projectIDs[calleePackagePath(callee)]; ok {
			isProject = true
		}
	}
	if !isProject {
		return
	}
	*entities = append(*entities, SemanticEntity{
		ID:       entityID("call", relPath, call.Pos(), callee.Name()),
		Kind:     "call",
		Name:     callee.Name(),
		Callee:   target,
		Source:   sourceRange(pkg.Fset, root, filePath, call),
		ParentID: parentID,
	})
}

func entityID(kind, relPath string, pos token.Pos, name string) string {
	posInfo := ""
	if pos.IsValid() {
		posInfo = fmt.Sprintf(":%d", pos)
	}
	base := filepath.ToSlash(relPath)
	if name != "" {
		return fmt.Sprintf("%s:%s:%s%s", kind, base, name, posInfo)
	}
	return fmt.Sprintf("%s:%s%s", kind, base, posInfo)
}

func fileEndLine(fset *token.FileSet, filePath string, file *ast.File) int {
	f := fset.File(file.Pos())
	if f == nil {
		return 1
	}
	return f.LineCount()
}

func exprText(fset *token.FileSet, filePath string, expr ast.Expr) string {
	if expr == nil {
		return ""
	}
	data, err := os.ReadFile(filePath)
	if err != nil {
		return ""
	}
	start := fset.Position(expr.Pos()).Offset
	end := fset.Position(expr.End()).Offset
	if start < 0 || end > len(data) || end <= start {
		return ""
	}
	return strings.TrimSpace(string(data[start:end]))
}
