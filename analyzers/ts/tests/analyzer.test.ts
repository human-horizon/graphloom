import { fileURLToPath } from "node:url"
import { dirname, resolve } from "node:path"
import { describe, expect, it } from "vitest"
import { analyzeDirectory } from "../src/analyzer.js"

const fixtureDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "../../../fixtures/ts-sample")

describe("TypeScript semantic analyzer", () => {
    const model = analyzeDirectory(fixtureDirectory)

    it("finds createUser with its source range and async signature", () => {
        const symbol = model.symbols.find(candidate => candidate.id === "src/user-service.ts:createUser")

        expect(symbol).toBeDefined()
        expect(symbol?.source).toEqual({
            file: "src/user-service.ts",
            start_line: 7,
            end_line: 13
        })
        expect(symbol?.is_exported).toBe(true)
        expect(symbol?.is_async).toBe(true)
        expect(symbol?.signature).toContain("Promise")
    })

    it("resolves a caller to the local createUser symbol", () => {
        expect(model.calls).toContainEqual(expect.objectContaining({
            from: "src/controller.ts:createUserRequest",
            to: "src/user-service.ts:createUser"
        }))
    })

    it("marks the saveUser database call as an effect", () => {
        expect(model.effects).toContainEqual(expect.objectContaining({
            symbol: "src/save.ts:saveUser",
            kind: "database",
            detail: "query"
        }))
    })
})
