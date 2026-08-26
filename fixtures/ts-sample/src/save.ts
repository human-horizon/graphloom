import { query } from "pg"
import type { User, UserInput } from "./types.js"

export async function saveUser(input: UserInput): Promise<User> {
    await query("INSERT INTO users (name, email) VALUES ($1, $2)", [input.name, input.email])
    return { id: input.email, ...input }
}
