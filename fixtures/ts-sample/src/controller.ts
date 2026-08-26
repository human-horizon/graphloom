import { createUser } from "./user-service.js"
import type { User } from "./types.js"

export async function createUserRequest(raw: string): Promise<User> {
    return createUser(raw)
}
