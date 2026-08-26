#!/usr/bin/env node

import { resolve } from "node:path"
import { analyzeDirectory } from "./analyzer.js"

const inputDirectory = process.argv[2]

if (!inputDirectory) {
    console.error("Usage: node dist/analyze.js <dir>")
    process.exitCode = 1
} else {
    try {
        const model = analyzeDirectory(resolve(inputDirectory))
        process.stdout.write(`${JSON.stringify(model)}\n`)
    } catch (error) {
        const message = error instanceof Error ? error.message : String(error)
        console.error(message)
        process.exitCode = 1
    }
}
