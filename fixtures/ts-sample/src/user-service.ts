import { sendEmail } from "./email.js"
import { parseInput } from "./parse.js"
import { saveUser } from "./save.js"
import type { User } from "./types.js"
import { validateInput } from "./validate.js"

export async function createUser(raw: string): Promise<User> {
    const input = parseInput(raw)
    validateInput(input)
    const user = await saveUser(input)
    await sendEmail(user.email)
    return user
}
