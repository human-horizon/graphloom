# Graphloom

AI превращает исходный код в интерактивное визуальное объяснение его поведения.
Целевые языки: Go, TypeScript. Десктоп-приложение на Tauri (Rust + SolidJS).

## Архитектура

```
Repository → Language Analyzer → Unified Code Model → AI Semantic Analyzer
→ Visualization DSL → Validator → Layout Engine → Renderer (self-contained HTML)
```

- **Анализаторы (sidecar):** `analyzers/go` (go/packages + go/types), `analyzers/ts` (ts-morph).
  Оба выводят Unified Code Model JSON в stdout.
- **AI** (любой OpenAI-совместимый endpoint: llama.cpp, Ollama) получает только UCM
  и отвечает строгим Visualization DSL (`src-tauri/src/dsl.schema.json`).
- **Validator** сверяет DSL с UCM: никаких выдуманных файлов, символов и связей.
- **Renderer** — детерминированный layered layout, офлайн HTML в `.graphloom/`.

## Запуск

```bash
# собрать анализаторы
cd analyzers/go && go build -o graphloom-analyze . && cd ../..
pnpm --dir analyzers/ts install && pnpm --dir analyzers/ts build

pnpm install
pnpm tauri dev
```

## Тесты

```bash
cargo test --manifest-path src-tauri/Cargo.toml   # unit + E2E против mock-LLM
go test ./...                                     # в analyzers/go
pnpm --dir analyzers/ts test                      # vitest
```

E2E не требует живого LLM: `fixtures/mock-llm/server.py` — OpenAI-совместимый stub,
отдающий эталонный DSL из `fixtures/mock-llm/responses/`.

## Возможности

- Карта проекта (пакеты/модули, классификация по палитре пользователя, тест-покрытие)
- Flow функции: семантические шаги, decision-ветки, side effects, привязка к строкам кода
- Слои Flow / Calls / Data / State / Effects, поиск, back/forward, раскрытие callee
- Конструктор типов элементов (палитра)
- Запуск тестов проекта (unit/integration/e2e) из приложения
