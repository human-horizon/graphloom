import { describe, expect, it } from "vitest"
import { createUser } from "../src/user-service.js"

describe("createUser", () => {
    it("creates a user from raw input", async () => {
        const user = await createUser("Ada:ada@example.test")
        expect(user.email).toBe("ada@example.test")
    })
})
