export async function sendEmail(email: string): Promise<void> {
    await fetch("https://example.test/welcome", {
        method: "POST",
        body: JSON.stringify({ email })
    })
}
