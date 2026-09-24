import {
    CallExpression,
    ForInStatement,
    ForOfStatement,
    ForStatement,
    IfStatement,
    Node,
    ReturnStatement,
    SourceFile,
    Statement,
    SwitchStatement,
    SyntaxKind,
    WhileStatement
} from "ts-morph"
import type { SourceRange } from "./analyzer.js"

export interface EntityModel {
    id: string
    kind: string
    name?: string
    label?: string
    symbol?: string
    callee?: string
    condition?: string
    code?: string
    coverage?: boolean
    source: SourceRange
    parent_id?: string
    children?: string[]
}

export function collectEntities(
    sourceFiles: SourceFile[],
    directory: string,
    declarations: Map<Node, string>
): EntityModel[] {
    const entities: EntityModel[] = []

    for (const sourceFile of sourceFiles) {
        const file = toRelativePath(directory, sourceFile.getFilePath())
        const fileId = `file:${file}`
        const fileEntity: EntityModel = {
            id: fileId,
            kind: "file",
            name: file,
            source: {
                file,
                start_line: 1,
                end_line: sourceFile.getEndLineNumber()
            }
        }
        entities.push(fileEntity)

        for (const symbol of sourceFile.getDescendantsOfKind(SyntaxKind.FunctionDeclaration)) {
            const name = symbol.getName()
            if (!name) continue
            const id = entityId("function", file, symbol.getStartLineNumber(), name)
            const symbolId = declarations.get(symbol)
            entities.push({
                id,
                kind: "function",
                name,
                symbol: symbolId,
                source: sourceRange(symbol, directory),
                parent_id: fileId
            })
            walkBody(symbol.getBody(), id, file, entities, declarations, directory)
        }

        for (const symbol of sourceFile.getDescendantsOfKind(SyntaxKind.MethodDeclaration)) {
            const name = symbol.getName()
            if (!name) continue
            const className = symbol.getFirstAncestorByKind(SyntaxKind.ClassDeclaration)?.getName()
            const symbolId = declarations.get(symbol)
            const id = entityId("method", file, symbol.getStartLineNumber(), name)
            entities.push({
                id,
                kind: "method",
                name,
                symbol: symbolId,
                source: sourceRange(symbol, directory),
                parent_id: fileId
            })
            walkBody(symbol.getBody(), id, file, entities, declarations, directory)
        }

        for (const symbol of sourceFile.getDescendantsOfKind(SyntaxKind.ClassDeclaration)) {
            const name = symbol.getName()
            if (!name) continue
            const symbolId = declarations.get(symbol)
            entities.push({
                id: entityId("type", file, symbol.getStartLineNumber(), name),
                kind: "type",
                name,
                symbol: symbolId,
                source: sourceRange(symbol, directory),
                parent_id: fileId
            })
        }

        for (const symbol of sourceFile.getDescendantsOfKind(SyntaxKind.InterfaceDeclaration)) {
            const name = symbol.getName()
            const symbolId = declarations.get(symbol)
            entities.push({
                id: entityId("interface", file, symbol.getStartLineNumber(), name),
                kind: "interface",
                name,
                symbol: symbolId,
                source: sourceRange(symbol, directory),
                parent_id: fileId
            })
        }

        for (const statement of sourceFile.getStatements()) {
            if (Node.isVariableStatement(statement)) {
                for (const declaration of statement.getDeclarations()) {
                    const initializer = declaration.getInitializer()
                    if (!initializer || (!Node.isArrowFunction(initializer) && !Node.isFunctionExpression(initializer))) {
                        continue
                    }
                    const name = declaration.getName()
                    const symbolId = declarations.get(initializer)
                    const id = entityId("function", file, declaration.getStartLineNumber(), name)
                    entities.push({
                        id,
                        kind: "function",
                        name,
                        symbol: symbolId,
                        source: sourceRange(initializer, directory),
                        parent_id: fileId
                    })
                    walkBody(initializer.getBody(), id, file, entities, declarations, directory)
                }
            }
        }
    }

    return entities.sort((a, b) => a.id.localeCompare(b.id))
}

function walkBody(
    body: Node | undefined,
    parentId: string,
    file: string,
    entities: EntityModel[],
    declarations: Map<Node, string>,
    directory: string
) {
    if (!body || !Node.isBlock(body)) {
        return
    }
    for (const statement of body.getStatements()) {
        walkStatement(statement, parentId, file, entities, declarations, directory)
    }
}

function walkStatement(
    statement: Statement,
    parentId: string,
    file: string,
    entities: EntityModel[],
    declarations: Map<Node, string>,
    directory: string
) {
    if (Node.isIfStatement(statement)) {
        const id = entityId("if", file, statement.getStartLineNumber())
        entities.push({
            id,
            kind: "if",
            condition: statement.getExpression().getText(),
            code: statement.getText(),
            coverage: true,
            source: sourceRange(statement, directory),
            parent_id: parentId
        })
        walkNestedStatement(statement.getThenStatement(), id, file, entities, declarations, directory)
        const elseStatement = statement.getElseStatement()
        if (elseStatement) {
            if (Node.isIfStatement(elseStatement)) {
                walkStatement(elseStatement, parentId, file, entities, declarations, directory)
            } else {
                const elseId = entityId("else", file, elseStatement.getStartLineNumber())
                entities.push({
                    id: elseId,
                    kind: "else",
                    code: elseStatement.getText(),
                    coverage: true,
                    source: sourceRange(elseStatement, directory),
                    parent_id: id
                })
                walkBody(elseStatement.asKind(SyntaxKind.Block), elseId, file, entities, declarations, directory)
            }
        }
    } else if (Node.isSwitchStatement(statement)) {
        const id = entityId("switch", file, statement.getStartLineNumber())
        entities.push({
            id,
            kind: "switch",
            condition: statement.getExpression().getText(),
            code: statement.getText(),
            coverage: true,
            source: sourceRange(statement, directory),
            parent_id: parentId
        })
        for (const clause of statement.getCaseBlock().getClauses()) {
            const caseId = entityId("case", file, clause.getStartLineNumber())
            entities.push({
                id: caseId,
                kind: "case",
                condition: Node.isCaseClause(clause) ? clause.getExpression().getText() : "default",
                code: clause.getText(),
                coverage: true,
                source: sourceRange(clause, directory),
                parent_id: id
            })
            if (Node.isCaseClause(clause)) {
                for (const s of clause.getStatements()) {
                    walkStatement(s, caseId, file, entities, declarations, directory)
                }
            } else if (Node.isDefaultClause(clause)) {
                for (const s of clause.getStatements()) {
                    walkStatement(s, caseId, file, entities, declarations, directory)
                }
            }
        }
    } else if (Node.isForStatement(statement) || Node.isForOfStatement(statement) || Node.isForInStatement(statement) || Node.isWhileStatement(statement)) {
        const id = entityId("loop", file, statement.getStartLineNumber())
        entities.push({
            id,
            kind: "loop",
            condition: extractLoopCondition(statement),
            code: statement.getText(),
            coverage: true,
            source: sourceRange(statement, directory),
            parent_id: parentId
        })
        walkNestedStatement(statement.getStatement(), id, file, entities, declarations, directory)
    } else if (Node.isReturnStatement(statement)) {
        entities.push({
            id: entityId("return", file, statement.getStartLineNumber()),
            kind: "return",
            code: statement.getText(),
            coverage: true,
            source: sourceRange(statement, directory),
            parent_id: parentId
        })
    } else if (Node.isExpressionStatement(statement)) {
        const statementId = addStatementEntity(statement, parentId, file, entities, directory)
        const expr = statement.getExpression()
        if (Node.isCallExpression(expr)) {
            addCallEntity(expr, statementId, file, entities, declarations, directory)
        } else if (Node.isAwaitExpression(expr)) {
            const awaited = expr.getExpression()
            if (Node.isCallExpression(awaited)) {
                addCallEntity(awaited, statementId, file, entities, declarations, directory)
            }
        }
    } else if (Node.isVariableStatement(statement)) {
        const statementId = addStatementEntity(statement, parentId, file, entities, directory)
        for (const declaration of statement.getDeclarations()) {
            const initializer = declaration.getInitializer()
            if (initializer && (Node.isCallExpression(initializer) || Node.isAwaitExpression(initializer))) {
                const expr = Node.isAwaitExpression(initializer) ? initializer.getExpression() : initializer
                if (Node.isCallExpression(expr)) {
                    addCallEntity(expr, statementId, file, entities, declarations, directory)
                }
            } else {
                entities.push({
                    id: entityId("variable", file, declaration.getStartLineNumber(), declaration.getName()),
                    kind: "variable",
                    name: declaration.getName(),
                    source: sourceRange(declaration, directory),
                    parent_id: statementId
                })
            }
        }
    } else {
        addStatementEntity(statement, parentId, file, entities, directory)
    }
}

function walkNestedStatement(
    statement: Node | undefined,
    parentId: string,
    file: string,
    entities: EntityModel[],
    declarations: Map<Node, string>,
    directory: string
) {
    if (!statement) {
        return
    }
    if (Node.isBlock(statement)) {
        walkBody(statement, parentId, file, entities, declarations, directory)
        return
    }
    if (Node.isStatement(statement)) {
        walkStatement(statement, parentId, file, entities, declarations, directory)
    }
}

function addStatementEntity(
    statement: Statement,
    parentId: string,
    file: string,
    entities: EntityModel[],
    directory: string
): string {
    const id = entityId("statement", file, statement.getStartLineNumber(), String(statement.getStart()))
    entities.push({
        id,
        kind: "statement",
        code: statement.getText(),
        coverage: true,
        source: sourceRange(statement, directory),
        parent_id: parentId
    })
    return id
}

function addCallEntity(
    call: CallExpression,
    parentId: string,
    file: string,
    entities: EntityModel[],
    declarations: Map<Node, string>,
    directory: string
) {
    const expression = call.getExpression()
    const name = expression.getText()
    const symbolId = declarations.get(call.getExpression())
    entities.push({
        id: entityId("call", file, call.getStartLineNumber(), name),
        kind: "call",
        name,
        callee: symbolId ?? name,
        source: sourceRange(call, directory),
        parent_id: parentId
    })
}

function extractLoopCondition(statement: ForStatement | ForOfStatement | ForInStatement | WhileStatement): string {
    if (Node.isForStatement(statement)) {
        const cond = statement.getCondition()
        return cond?.getText() ?? ""
    }
    if (Node.isForOfStatement(statement) || Node.isForInStatement(statement)) {
        return statement.getExpression().getText()
    }
    if (Node.isWhileStatement(statement)) {
        return statement.getExpression().getText()
    }
    return ""
}

function entityId(kind: string, file: string, line: number, name?: string): string {
    const base = name ? `${kind}:${file}:${name}:${line}` : `${kind}:${file}:${line}`
    return base.replace(/\s+/g, "")
}

function sourceRange(node: Node, directory: string): SourceRange {
    return {
        file: toRelativePath(directory, node.getSourceFile().getFilePath()),
        start_line: node.getStartLineNumber(),
        end_line: node.getEndLineNumber()
    }
}

function toRelativePath(directory: string, filePath: string): string {
    const relative = filePath.replace(`${directory}/`, "").replaceAll("\\", "/")
    return relative
}
