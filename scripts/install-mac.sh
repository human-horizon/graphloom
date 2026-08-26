#!/bin/zsh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

(cd analyzers/go && go build -o graphloom-analyze .)
(cd analyzers/ts && pnpm install --silent && pnpm run --silent build)
pnpm install --silent

pnpm typecheck
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
pnpm tauri build --bundles app

killall graphloom 2>/dev/null || true
sleep 1
rm -rf /Applications/Graphloom.app
ditto src-tauri/target/release/bundle/macos/Graphloom.app /Applications/Graphloom.app
open -n /Applications/Graphloom.app

printf 'Installed /Applications/Graphloom.app\n'
