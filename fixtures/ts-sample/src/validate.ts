import type { UserInput } from "./types.js"

export function validateInput(input: UserInput): void {
    if (!input.email.includes("@")) {
        throw new Error("Invalid email")
    }
}
