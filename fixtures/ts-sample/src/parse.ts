import type { UserInput } from "./types.js"

export function parseInput(raw: string): UserInput {
    const [name, email] = raw.split(":")
    return { name, email }
}
