# Graphloom — семантический pipeline (пивот по vision-документу)

## Контекст

MVP-000 собран (Tauri + Solid + LLM по сырому тексту). Vision-документ (Google Doc
«AI-визуализация кода для Go и TypeScript») требует другую архитектуру:

```
Repository → Language Analyzer → Unified Code Model → AI Semantic Analyzer
→ Visualization DSL → Validator → Layout Engine → Renderer (HTML)
```

Ключевые принципы:
- AI **не** читает сырой код и **не** рисует HTML — он интерпретирует структуру,
  извлечённую статическим анализом (защита от галлюцинаций).
- Контракт AI↔renderer — строгий JSON DSL, валидируемый JSON Schema.
- Каждый узел привязан к файлу и строкам (`source reference`), каждая связь либо
  подтверждена статанализом, либо помечена `inferred`.
- Renderer детерминирован, офлайн, self-contained HTML.
- Многоуровневость: карта проекта → flow функции. Слои Flow / Calls / Data / State / Effects.

Сохраняем из MVP-000: Tauri-каркас, экраны Настройки/Конструктор/Отчёты, LLM-клиент,
палитру элементов, панель запуска тестов, хранение отчётов в `.graphloom/`.

## Цель

Реализовать полный семантический pipeline для Go и TypeScript:
карта проекта (пакеты/модули) + flow выбранной функции, оба — через
`Analyzer → UCM → AI → DSL → Validator → Renderer`, с валидацией source-ссылок.

## Что изменится

### Новое

1. `analyzers/go/` — Go-модуль, бинарь `graphloom-analyze` (sidecar):
   `go/packages` + `go/ast` + `go/types`, выводит UCM JSON в stdout.
2. `analyzers/ts/` — Node-скрипт на `ts-morph`, выводит UCM JSON в stdout.
3. `src-tauri/src/ucm.rs` — Unified Code Model (serde-типы).
4. `src-tauri/src/analyzer.rs` — запуск sidecar'ов, слияние UCM, кэш `.graphloom/ucm.json`.
5. `src-tauri/src/dsl.rs` — типы Visualization DSL.
6. `src-tauri/src/dsl.schema.json` — JSON Schema DSL.
7. `src-tauri/src/validate.rs` — валидация DSL: JSON Schema + source refs + ссылки узлов.
8. `src-tauri/src/pipeline.rs` — оркестрация `analyze_project` / `analyze_function`.
9. `fixtures/go-sample/`, `fixtures/ts-sample/` — эталонные мини-проекты для тестов.
10. `fixtures/mock-llm/` — mock OpenAI-endpoint (python3, stdlib), отдаёт эталонный DSL.

### Переработка

11. `src-tauri/src/llm.rs` — промпты принимают UCM JSON (не сырой код) и требуют DSL JSON.
12. `src-tauri/src/render.rs` — детерминированный layered layout, слои, source-панель,
    поиск, back/forward, раскрытие функции по клику.
13. `src-tauri/src/collect.rs` — больше не нужен для анализа; удаляем из pipeline
    (файл удалить, тестовая панель его не использует).
14. `src/screens/Reports.tsx` — выбор функции для flow, два режима отчёта.
15. `src/lib/ipc.ts` — новые команды.

### Без изменений

Настройки, палитра (классификация узлов карты проекта), testrun, HTML-отчёты в `.graphloom/`.

## Детали реализации

### Unified Code Model (`ucm.rs`)

```rust
pub struct UnifiedCodeModel {
    pub language: String,            // "go" | "typescript" | "mixed"
    pub packages: Vec<Package>,      // пакеты/модули
    pub symbols: Vec<Symbol>,
    pub calls: Vec<Call>,
    pub effects: Vec<ExternalEffect>,
}
pub struct Package { pub id: String, pub name: String, pub dir: String, pub files: Vec<String> }
pub struct Symbol {
    pub id: String,                  // "pkg.Func" / "file.ts:funcName"
    pub kind: SymbolKind,            // Function | Method | Type | Interface | Variable
    pub name: String, pub package: String,
    pub source: SourceRange,
    pub signature: String,
    pub is_exported: bool,
    pub is_async: bool,
}
pub struct SourceRange { pub file: String, pub start_line: u32, pub end_line: u32 }
pub struct Call { pub from: String, pub to: String, pub source: SourceRange } // symbol ids
pub struct ExternalEffect {
    pub symbol: String,
    pub kind: EffectKind,            // Network | Database | FileSystem | Queue | Log | Other
    pub detail: String, pub source: SourceRange,
}
```

### Go-анализатор (`analyzers/go/main.go`)

- `packages.Load` с `NeedSyntax|NeedTypes|NeedTypesInfo|NeedDeps|NeedImports` по `./...`.
- Для каждого пакета: symbols (функции, методы, типы), calls (по `ast.CallExpr` +
  `types.Info.Uses`), effects (эвристика по вызовам `net/http`, `database/sql`, `os.`, `log.`, `context`-queue паттернам).
- Позиции — через `token.FileSet` → относительные пути, диапазоны строк.
- Флаги: `-dir <path>`, вывод UCM JSON в stdout, ошибки — в stderr, exit≠0.
- `go test` — табличные тесты на `fixtures/go-sample`.

### TS-анализатор (`analyzers/ts/`)

- `ts-morph`: `Project` по `tsconfig.json` (fallback — добавление `**/*.{ts,tsx}` без node_modules).
- Symbols: функции, методы, классы, интерфейсы; calls — по `CallExpression` +
  `getSymbol()`; async — `async`/`Promise.then`/`await`; effects — `fetch`, `fs.`, `axios`, `console/log` перехватчики.
- Вывод UCM JSON в stdout. Стиль: 4 пробела, без точек с запятой.
- Запуск: `node analyzers/ts/dist/analyze.js <dir>` (сборка `tsc`).
- Тест: vitest на `fixtures/ts-sample`.

### Сбор и слияние (`analyzer.rs`)

- `detect languages`: есть `go.mod` → Go; есть `package.json`/`tsconfig.json` → TS.
- Запуск sidecar'ов (`tokio::process`), парсинг stdout → `UnifiedCodeModel`.
- Оба языка → слияние, `language: "mixed"`.
- Кэш: `.graphloom/ucm.json` + mtime-маркер; команда `get_symbols(path)` отдаёт кэш или пересобирает.

### Visualization DSL (`dsl.rs` + `dsl.schema.json`)

```rust
pub struct Visualization {
    pub title: String,
    pub level: Level,                // Project | Function
    pub nodes: Vec<VizNode>,
    pub edges: Vec<VizEdge>,
}
pub struct VizNode {
    pub id: String,
    pub kind: NodeKind,              // Action|Decision|Call|Input|Output|State|Storage|
                                     // External|Loop|Error|Async|Group
    pub label: String,               // смысловая подпись от AI
    pub layer: Layer,                // Flow|Calls|Data|State|Effects
    pub source: Option<SourceRef>,   // обязателен для уровня Function
    pub element_type: Option<String>,// классификация по палитре (уровень Project)
    pub symbol: Option<String>,      // id символа из UCM
    pub tests: Option<TestCoverage>, // сохраняем покрытие из MVP-000
    pub confidence: Option<f64>,
    pub children: Vec<VizNode>,      // Group
    pub branches: Vec<Branch>,       // Decision
    pub data_in: Vec<String>, pub data_out: Vec<String>,
    pub effects: Vec<String>,
}
pub struct SourceRef { pub file: String, pub start_line: u32, pub end_line: u32 }
pub struct Branch { pub condition: String, pub target: String }
pub struct VizEdge {
    pub from: String, pub to: String,
    pub label: Option<String>,
    pub status: EdgeStatus,          // Verified | Inferred
}
```

### Validator (`validate.rs`)

- JSON Schema (`jsonschema` crate) — структурная валидация.
- Семантическая валидация против UCM:
  - каждый `source.file` существует в UCM, диапазон строк в пределах файла;
  - `symbol` ссылается на существующий `Symbol.id`;
  - все `edges[].from/to` и `branches[].target` — существующие node id;
  - `element_type` — из палитры или null.
- Связь, которой нет в `ucm.calls`, но заявлена как `Verified` → принудительно `Inferred`.
- Ошибки валидации → 1 ретрай LLM с текстом ошибок → затем ошибка пользователю.

### LLM (`llm.rs`)

- Два промпта:
  - `project_map_prompt(ucm, palette)` — узлы-Group по пакетам, классификация по палитре,
    связи по `ucm.calls` между пакетами, оценка тест-покрытия по test-файлам из UCM.
  - `function_flow_prompt(ucm, symbol_id)` — flow выбранной функции + flow функций,
    вызываемых напрямую (1 уровень), узлы со `source` обязательно.
- В промпт идёт: компактный UCM JSON + DSL-схема + требование strict JSON.
- Ретрай: 1 раз при невалидном JSON/схеме с текстом ошибок валидации.

### Renderer (`render.rs`)

- Детерминированный layout: layered (longest-path слои по `edges`), внутри слоя —
  порядок по barycenter, узлы стабильно сортируются по `id`. Никакого random.
- Self-contained HTML, тёмная тема:
  - верхняя панель: заголовок, переключатели слоёв (Flow/Calls/Data/State/Effects),
    поиск (подсветка пути узла), кнопки Back/Forward (история навигации);
  - узлы — rounded rect, цвет: `element_type` → палитра, иначе `kind` → встроенный стиль;
    пунктирная рамка у `Inferred`-связей;
  - клик по узлу → боковая панель: label, summary, **исходный код** (файл+строки,
    исходники зашиваются в HTML из UCM-диапазонов);
  - клик по узлу-`Call` с flow целевой функции → переход на её flow (Back возвращает);
  - сводка тест-покрытия сохраняется на уровне Project.
- Файл: `.graphloom/report-<level>-<timestamp>.html`.

### Pipeline (`pipeline.rs`) и команды

- `analyze_project(path)` → analyzer → UCM → project_map_prompt → DSL → validate →
  render → сохранить HTML. Возврат `{ reportPath, nodes, edges }`.
- `get_symbols(path)` → список функций/методов из UCM (id, name, package, file) —
  для выбора функции в UI.
- `analyze_function(path, symbol_id)` → function_flow_prompt → DSL → validate → render.
- Команды тестов и настроек — без изменений.

### Frontend

- `Reports.tsx`: после выбора папки — кнопка «Карта проекта» и select функции
  (`get_symbols`) + кнопка «Flow функции». Остальное без изменений.
- `ipc.ts`: типы `SymbolInfo { id, name, package, file }`, команды `getSymbols`,
  `analyzeFunction(path, symbolId)`.

### Верификация без живого LLM

- `fixtures/mock-llm/server.py` — OpenAI-совместимый stub (python3 stdlib):
  `/v1/models`, `/v1/chat/completions` → по маркеру промпта возвращает эталонный DSL
  (карта или flow) из `fixtures/mock-llm/responses/`.
- E2E-проверка pipeline: настройки → `http://127.0.0.1:8399/v1` → `analyze_project`
  на fixtures → HTML появляется и содержит узлы эталонного DSL.

## Критерии приёмки

- [x] `go build ./...` и `go test ./...` в `analyzers/go` — зелёные; UCM JSON на fixture содержит symbols/calls/effects
- [x] `pnpm --dir analyzers/ts build && pnpm --dir analyzers/ts test` — зелёные; UCM JSON на fixture корректен
- [x] `cargo test` в `src-tauri` — validator ловит: битый source ref, несуществующий node id, неизвестный element_type
- [x] `analyze_project` на fixture против mock-LLM → HTML в `.graphloom/`, узлы из эталонного DSL
- [x] `analyze_function` на fixture против mock-LLM → HTML flow с source-панелью
- [x] Layout детерминирован: два запуска render → байт-в-байт одинаковый HTML (тест `render_is_deterministic` + E2E)
- [x] `Inferred`-связи отображаются пунктиром; несуществующая связь не может быть `Verified` (тест `downgrades_unbacked_verified_edge`)
- [x] UI: выбор функции и запуск flow работают; отчёты открываются в просмотрщике
- [x] `cargo clippy`, `pnpm tsc --noEmit` — без ошибок
- [x] Палитра и панель тестов работают как раньше
