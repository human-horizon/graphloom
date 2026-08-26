# Graphloom

AI превращает исходный код в интерактивное визуальное объяснение его поведения.
Целевые языки: Go, TypeScript. Десктоп-приложение на Tauri (Rust + SolidJS).

## Архитектура

```
Repository → Language Analyzer (entities) → Unified Code Model → AI Labeler
→ Semantic Scope DSL → Validator → Layout Engine → Renderer (self-contained HTML)
```

- **Анализаторы (sidecar):** `analyzers/go` (go/packages + go/ast), `analyzers/ts` (ts-morph).
  Оба извлекают **семантические сущности** (function, call, if/else, loop, return, variable, type, interface)
  со stable ID и AST-диапазонами.
- **AI** (любой OpenAI-совместимый endpoint) получает только дерево сущностей и **только лейблит** их:
  не придумывает новые узлы и не меняет структуру.
- **Validator** сверяет DSL с UCM: никаких выдуманных файлов, символов и связей.
- **Renderer** — древовидный scope layout, офлайн HTML в `.graphloom/`, кликабельные cross-ссылки между файлами.

## Запуск

```bash
./scripts/install-mac.sh   # собирает sidecar-ы, тесты, tauri build и устанавливает в /Applications
```

Или вручную:

```bash
pnpm install
cd analyzers/go && go build -o graphloom-analyze . && cd ../..
pnpm --dir analyzers/ts install && pnpm --dir analyzers/ts build
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

- **Project map:** дерево пакетов → файлов → экспортированных символов с AI-лейблами и cross-ссылками (входящими и исходящими).
- **File map:** scope tree функций, вызовов, условий и переменных внутри одного файла.
- **Flow функции:** семантические шаги, decision-ветки, side effects, привязка к строкам кода.
- **Cross-ссылки:** клик по call/символу открывает целевой файл и строит Flow целевой функции.
- **Слои** Flow / Calls / Data / State / Effects, поиск, back/forward, раскрытие callee.
- **DSL persistence:** каждый отчёт сохраняется как `.dsl.json` — можно перерисовать без LLM.
- **Конструктор типов элементов** (палитра).
- **Запуск тестов проекта** (unit/integration/e2e) из приложения.
- **Инкрементная генерация:** повторный `Update` и `Project map` используют кэш, если исходники/настройки не менялись.
