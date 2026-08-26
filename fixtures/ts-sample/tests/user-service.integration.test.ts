import { describe, expect, it } from "vitest"
import { createUser } from "../src/user-service.js"

describe("user creation flow", () => {
    it("returns the persisted identity", async () => {
        const user = await createUser("Grace:grace@example.test")
        expect(user.id).toBe("grace@example.test")
    })
})
