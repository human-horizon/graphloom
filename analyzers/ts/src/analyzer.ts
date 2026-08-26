import { existsSync } from "node:fs"
import { collectEntities, type EntityModel } from "./entities.js"
import { dirname, extname, isAbsolute, join, relative, resolve } from "node:path"
import {
    ArrowFunction,
    CallExpression,
    ClassDeclaration,
    FunctionDeclaration,
    FunctionExpression,
    InterfaceDeclaration,
    MethodDeclaration,
    Node,
    Project,
    SourceFile,
    SyntaxKind,
    VariableDeclaration
} from "ts-morph"

export type SymbolKind = "function" | "method" | "type" | "interface"
export type EffectKind = "network" | "database" | "file_system" | "queue" | "log" | "other"

export interface SourceRange {
    file: string
    start_line: number
    end_line: number
}

export interface PackageModel {
    id: string
    name: string
    dir: string
    files: string[]
}

export interface SymbolModel {
    id: string
    kind: SymbolKind
    name: string
    package: string
    source: SourceRange
    signature: string
    is_exported: boolean
    is_async: boolean
}

export interface CallModel {
    from: string
    to: string
    source: SourceRange
}

export interface EffectModel {
    symbol: string
    kind: EffectKind
    detail: string
    source: SourceRange
}

export type { EntityModel }

export interface UnifiedCodeModel {
    language: "typescript"
    packages: PackageModel[]
    symbols: SymbolModel[]
    calls: CallModel[]
    effects: EffectModel[]
    entities?: EntityModel[]
}

type CallableNode = FunctionDeclaration | MethodDeclaration | ArrowFunction | FunctionExpression
type SymbolNode = CallableNode | ClassDeclaration | InterfaceDeclaration | VariableDeclaration

interface ImportInfo {
    module: string
    localNames: Set<string>
}

interface SymbolIndex {
    declarations: Map<Node, string>
    symbols: SymbolModel[]
}

const EFFECT_MODULES: Array<{ kind: EffectKind, modules: string[] }> = [
    { kind: "network", modules: ["axios", "node-fetch", "undici"] },
    { kind: "file_system", modules: ["fs", "node:fs", "fs/promises", "node:fs/promises"] },
    { kind: "queue", modules: ["bull", "bullmq", "amqplib", "rabbitmq"] },
    { kind: "database", modules: ["prisma", "@prisma/client", "knex", "pg", "mysql", "mysql2", "sequelize", "typeorm"] }
]

export function analyzeDirectory(inputDirectory: string): UnifiedCodeModel {
    const directory = resolve(inputDirectory)
    if (!existsSync(directory)) {
        throw new Error(`Analysis directory does not exist: ${directory}`)
    }

    const project = createProject(directory)
    const sourceFiles = getProjectSourceFiles(project, directory)
    const packageMap = collectPackages(sourceFiles, directory)
    const symbolIndex = collectSymbols(sourceFiles, directory)
    const calls: CallModel[] = []
    const effects: EffectModel[] = []

    for (const sourceFile of sourceFiles) {
        const imports = collectImports(sourceFile)
        for (const call of sourceFile.getDescendantsOfKind(SyntaxKind.CallExpression)) {
            const from = findEnclosingSymbol(call, symbolIndex.declarations)
            if (!from) {
                continue
            }

            calls.push({
                from,
                to: resolveCallTarget(call, symbolIndex.declarations, directory),
                source: getSourceRange(call, directory)
            })

            const effectKind = detectEffect(call, imports)
            if (effectKind) {
                effects.push({
                    symbol: from,
                    kind: effectKind,
                    detail: call.getExpression().getText(),
                    source: getSourceRange(call, directory)
                })
            }
        }
    }

    return {
        language: "typescript",
        packages: Array.from(packageMap.values()).sort((left, right) => left.id.localeCompare(right.id)),
        symbols: symbolIndex.symbols.sort((left, right) => left.id.localeCompare(right.id)),
        calls: calls.sort(compareSourceRecords),
        effects: effects.sort(compareSourceRecords),
        entities: collectEntities(sourceFiles, directory, symbolIndex.declarations)
    }
}

function createProject(directory: string): Project {
    const tsconfigPath = join(directory, "tsconfig.json")
    if (existsSync(tsconfigPath)) {
        return new Project({
            tsConfigFilePath: tsconfigPath,
            skipAddingFilesFromTsConfig: false
        })
    }

    const project = new Project({
        compilerOptions: {
            allowJs: false,
            noEmit: true,
            skipLibCheck: true
        }
    })
    project.addSourceFilesAtPaths(join(directory, "**/*.{ts,tsx}"))
    return project
}

function getProjectSourceFiles(project: Project, directory: string): SourceFile[] {
    const nodeModulesDirectory = `${join(directory, "node_modules")}${join(directory, "node_modules").endsWith("/") ? "" : "/"}`
    return project.getSourceFiles()
        .filter(sourceFile => {
            const filePath = sourceFile.getFilePath()
            const extension = extname(filePath)
            return !filePath.startsWith(nodeModulesDirectory)
                && (extension === ".ts" || extension === ".tsx")
                && !sourceFile.isDeclarationFile()
        })
        .sort((left, right) => left.getFilePath().localeCompare(right.getFilePath()))
}

function collectPackages(sourceFiles: SourceFile[], directory: string): Map<string, PackageModel> {
    const packages = new Map<string, PackageModel>()

    for (const sourceFile of sourceFiles) {
        const file = toRelativePath(directory, sourceFile.getFilePath())
        const packageDirectory = normalizePath(dirname(file))
        const packageName = packageDirectory === "." ? "." : packageDirectory.split("/").at(-1) ?? "."
        const existing = packages.get(packageDirectory)
        if (existing) {
            existing.files.push(file)
            continue
        }

        packages.set(packageDirectory, {
            id: packageDirectory,
            name: packageName,
            dir: packageDirectory,
            files: [file]
        })
    }

    for (const packageModel of packages.values()) {
        packageModel.files.sort()
    }
    return packages
}

function collectSymbols(sourceFiles: SourceFile[], directory: string): SymbolIndex {
    const declarations = new Map<Node, string>()
    const symbols: SymbolModel[] = []

    for (const sourceFile of sourceFiles) {
        const file = toRelativePath(directory, sourceFile.getFilePath())
        const packageName = normalizePath(dirname(file))
        const register = (node: SymbolNode, id: string, kind: SymbolKind, name: string, isExported: boolean, isAsync: boolean, signature: string) => {
            if (declarations.has(node)) {
                return
            }
            declarations.set(node, id)
            symbols.push({
                id,
                kind,
                name,
                package: packageName,
                source: getSourceRange(node, directory),
                signature,
                is_exported: isExported,
                is_async: isAsync
            })
        }

        for (const declaration of sourceFile.getDescendantsOfKind(SyntaxKind.FunctionDeclaration)) {
            const name = declaration.getName()
            if (!name) {
                continue
            }
            register(
                declaration,
                `${file}:${name}`,
                "function",
                name,
                declaration.isExported(),
                isAsyncFunction(declaration),
                getCallableSignature(name, declaration)
            )
        }

        for (const declaration of sourceFile.getDescendantsOfKind(SyntaxKind.MethodDeclaration)) {
            const name = declaration.getName()
            const classDeclaration = declaration.getFirstAncestorByKind(SyntaxKind.ClassDeclaration)
            const className = classDeclaration?.getName()
            if (!name || !className) {
                continue
            }
            register(
                declaration,
                `${file}:${className}.${name}`,
                "method",
                name,
                classDeclaration?.isExported() ?? false,
                isAsyncFunction(declaration),
                getCallableSignature(name, declaration)
            )
        }

        for (const declaration of sourceFile.getDescendantsOfKind(SyntaxKind.ClassDeclaration)) {
            const name = declaration.getName()
            if (!name) {
                continue
            }
            register(
                declaration,
                `${file}:${name}`,
                "type",
                name,
                declaration.isExported(),
                false,
                `class ${name}`
            )
        }

        for (const declaration of sourceFile.getDescendantsOfKind(SyntaxKind.InterfaceDeclaration)) {
            const name = declaration.getName()
            register(
                declaration,
                `${file}:${name}`,
                "interface",
                name,
                declaration.isExported(),
                false,
                `interface ${name}`
            )
        }

        for (const declaration of sourceFile.getDescendantsOfKind(SyntaxKind.VariableDeclaration)) {
            const initializer = declaration.getInitializer()
            if (!initializer || (!Node.isArrowFunction(initializer) && !Node.isFunctionExpression(initializer))) {
                continue
            }
            const name = declaration.getName()
            const id = `${file}:${name}`
            const functionNode = initializer as ArrowFunction | FunctionExpression
            declarations.set(functionNode, id)
            register(
                declaration,
                id,
                "function",
                name,
                isVariableExported(declaration),
                isAsyncFunction(functionNode),
                getCallableSignature(name, functionNode)
            )
        }
    }

    return { declarations, symbols }
}

function collectImports(sourceFile: SourceFile): ImportInfo[] {
    return sourceFile.getImportDeclarations().map(importDeclaration => {
        const localNames = new Set<string>()
        const defaultImport = importDeclaration.getDefaultImport()
        if (defaultImport) {
            localNames.add(defaultImport.getText())
        }
        const namespaceImport = importDeclaration.getNamespaceImport()
        if (namespaceImport) {
            localNames.add(namespaceImport.getText())
        }
        for (const namedImport of importDeclaration.getNamedImports()) {
            localNames.add(namedImport.getAliasNode()?.getText() ?? namedImport.getName())
        }
        return {
            module: importDeclaration.getModuleSpecifierValue(),
            localNames
        }
    })
}

function resolveCallTarget(call: CallExpression, declarations: Map<Node, string>, directory: string): string {
    const symbol = call.getExpression().getSymbol()
    const localDeclaration = findLocalDeclaration(symbol, declarations, directory)
    if (localDeclaration) {
        return localDeclaration
    }
    return call.getExpression().getText()
}

function findLocalDeclaration(symbol: ReturnType<CallExpression["getExpression"]>["getSymbol"] extends (...args: never[]) => infer T ? T : never, declarations: Map<Node, string>, directory: string): string | undefined {
    if (!symbol) {
        return undefined
    }
    for (const declaration of symbol.getDeclarations()) {
        const id = declarations.get(declaration)
        if (id && isProjectNode(declaration, directory)) {
            return id
        }
    }
    const aliasedSymbol = symbol.getAliasedSymbol()
    if (aliasedSymbol && aliasedSymbol !== symbol) {
        return findLocalDeclaration(aliasedSymbol, declarations, directory)
    }
    return undefined
}

function detectEffect(call: CallExpression, imports: ImportInfo[]): EffectKind | undefined {
    const expression = call.getExpression().getText()
    const rootName = expression.split(".")[0]
    if (rootName === "fetch") {
        return "network"
    }
    if (rootName === "console") {
        return "log"
    }

    const importedModule = imports.find(importInfo => importInfo.localNames.has(rootName))?.module
    if (importedModule) {
        const normalizedModule = importedModule.toLowerCase()
        const effectModule = EFFECT_MODULES.find(entry => entry.modules.some(module => normalizedModule === module || normalizedModule.startsWith(`${module}/`)))
        if (effectModule) {
            return effectModule.kind
        }
    }

    if (rootName === "axios") {
        return "network"
    }
    if (rootName === "fs") {
        return "file_system"
    }
    if (rootName === "db" || rootName === "database") {
        return "database"
    }
    return undefined
}

function findEnclosingSymbol(node: CallExpression, declarations: Map<Node, string>): string | undefined {
    let current: Node | undefined = node.getParent()
    while (current) {
        const symbolId = declarations.get(current)
        if (symbolId && isCallableNode(current)) {
            return symbolId
        }
        current = current.getParent()
    }
    return undefined
}

function isCallableNode(node: Node): node is CallableNode {
    return Node.isFunctionDeclaration(node)
        || Node.isMethodDeclaration(node)
        || Node.isArrowFunction(node)
        || Node.isFunctionExpression(node)
}

function isAsyncFunction(node: CallableNode): boolean {
    if (node.isAsync()) {
        return true
    }
    return getReturnTypeText(node).startsWith("Promise")
}

function getCallableSignature(name: string, node: CallableNode): string {
    const parameters = node.getParameters().map(parameter => parameter.getText()).join(", ")
    return `${name}(${parameters}): ${getReturnTypeText(node)}`
}

function getReturnTypeText(node: CallableNode): string {
    return node.getReturnTypeNode()?.getText() ?? node.getReturnType().getText()
}

function isVariableExported(declaration: VariableDeclaration): boolean {
    const statement = declaration.getFirstAncestorByKind(SyntaxKind.VariableStatement)
    return statement?.isExported() ?? false
}

function getSourceRange(node: Node, directory: string): SourceRange {
    const sourceFile = node.getSourceFile()
    return {
        file: toRelativePath(directory, sourceFile.getFilePath()),
        start_line: node.getStartLineNumber(),
        end_line: node.getEndLineNumber()
    }
}

function isProjectNode(node: Node, directory: string): boolean {
    const filePath = node.getSourceFile().getFilePath()
    return isAbsolute(filePath) && filePath.startsWith(`${directory}/`)
}

function toRelativePath(directory: string, filePath: string): string {
    return normalizePath(relative(directory, filePath))
}

function normalizePath(filePath: string): string {
    return filePath.replaceAll("\\", "/")
}

function compareSourceRecords(left: { source: SourceRange }, right: { source: SourceRange }): number {
    return left.source.file.localeCompare(right.source.file) || left.source.start_line - right.source.start_line || left.source.end_line - right.source.end_line
}
