# 014 — Semantic scope DSL и переработка pipeline

## Проблема текущего подхода
1. **Flat graph** (`nodes[]` + `edges[]`) не выражает вложенность и scope. Получается каша из рёбер, сложно понять, где функция, где блок, где просто шаг.
2. **LLM получает голый исходный файл** и сама должна из него вытащить структуру. Это приводит к галлюцинациям, лишним узлам (`fmt.Printf`, `bytes.Contains`) и неточным номерам строк.
3. **Нет стабильных идентификаторов** сущностей. Связи висят в воздухе; невозможно сделать надёжный переход на другой файл.

## Цель
Пайплайн должен сам разбирать AST, выделять сущности и давать LLM только семантический срез. DSL должен быть деревом scope'ов, а не плоским графом.

## Новая архитектура

### 1. Analyzer v2: семантические сущности
Анализаторы Go/TS извлекают не просто symbols/calls, а **семантические сущности**:

```json
{
  "entities": [
    {
      "id": "github.com/starframe/abyss/cmd/analyze_habr.main",
      "kind": "function",
      "name": "main",
      "file": "cmd/analyze_habr/main.go",
      "range": { "start_line": 12, "end_line": 42 },
      "signature": "func main()",
      "doc": "...",
      "inputs": [],
      "outputs": [],
      "children": [
        { "kind": "call", "callee": "fetcher.Fetch", "line": 15 },
        { "kind": "decision", "condition": "err != nil", "line": 16, "true_branch": [...], "false_branch": [...] },
        { "kind": "call", "callee": "dom.Parse", "line": 19 }
      ]
    }
  ],
  "relations": [
    { "from": "...main", "to": "...fetcher.Fetch", "kind": "calls" }
  ]
}
```

Сущности: `function`, `method`, `type`, `interface`, `variable`, `block`, `loop`, `decision`, `call`, `return`, `error`, `effect`.
Для каждой сущности известен kind, range, parent, дети и связанный symbol ID.

### 2. DSL v2: дерево scope'ов
Каждый узел — это scope/сущность. У него есть `children`, которые упорядочены. Связи внутри scope не нужны как отдельные edges: они выводятся из порядка `children`.

```json
{
  "title": "cmd/analyze_habr/main.go",
  "level": "file",
  "uri": "file:cmd/analyze_habr/main.go",
  "kind": "file",
  "label": "Анализ страницы Habr.com",
  "children": [
    {
      "id": "func:cmd/analyze_habr/main:main",
      "kind": "function",
      "label": "Главная функция",
      "symbol": "github.com/starframe/abyss/cmd/analyze_habr.main",
      "range": { "file": "cmd/analyze_habr/main.go", "start_line": 12, "end_line": 42 },
      "children": [
        { "id": "step:main:1", "kind": "call", "label": "Загрузить HTML", "callee": "github.com/starframe/abyss/internal/fetcher.Fetch", "range": { ... } },
        { "id": "step:main:2", "kind": "decision", "label": "Обработка ошибки загрузки", "condition": "ошибка", "children": [ ... ] },
        { "id": "step:main:3", "kind": "call", "label": "Разобрать HTML", "callee": "github.com/starframe/abyss/internal/dom.Parse", "range": { ... } }
      ]
    }
  ],
  "cross_refs": [
    { "from": "step:main:1", "to": "function:internal/fetcher/fetcher.go:Fetch" },
    { "from": "step:main:3", "to": "function:internal/dom/dom.go:Parse" }
  ]
}
```

- `children` всегда упорядочены: сверху вниз, слева направо = порядок выполнения.
- `cross_refs` — только межфайловые/межфункциональные ссылки. Их можно кликать и переходить.
- Узлы не нуждаются в отдельном `edges[]` для последовательности.

### 3. LLM v2: только смысл, не структура
Qwen получает не исходный файл, а семантический JSON одной сущности:

```json
{
  "entity": { "id": "...", "kind": "function", "signature": "...", "children": [...] },
  "callees": [
    { "id": "...", "kind": "function", "signature": "...", "doc": "..." }
  ],
  "callers": [...]
}
```

Задача Qwen:
- написать человеческий `label` и `summary` для сущности и каждого ребёнка;
- выбрать `kind` (action/decision/call/error/async/input/output/state/storage/external/loop) для каждого шага;
- для `decision` — написать `condition` и подписать ветки;
- не придумывать новые сущности, не менять ID, не менять порядок детей.

### 4. Renderer v2
- Древовидный layout: function/scope раскрываются; дети рисуются внутри родителя или как следующий уровень.
- Call-узлы с `callee` кликабельны и ведут в другой файл/функцию.
- Decision рисуется как ромб/ветвление, но внутри родительского scope.
- Source panel использует точные range из analyzer, а не LLM.

### 5. Pipeline v2
1. **Analyze**: sidecar строит UCM + семантическое дерево сущностей.
2. **Segment**: pipeline разбивает файл на сущности (функции, методы).
3. **Describe**: для каждой сущности вызывается LLM с семантическим срезом. Результат — DSL-фрагмент.
4. **Assemble**: фрагменты склеиваются в file-level scope tree.
5. **Validate**: проверяем, что все ID стабильны, ranges корректны, cross_refs разрешимы.
6. **Render**: дерево → self-contained HTML.

## Критерии приёмки
- [x] Новый формат DSL v2 (scope tree) задокументирован в `dsl-v2.schema.json`.
- [x] Analyzer Go извлекает entities с AST-позициями (`go/ast`) и stable IDs.
- [x] Analyzer TS извлекает entities через `ts-morph`.
- [x] LLM получает семантический срез, не голый код; не создаёт лишних узлов.
- [x] Pipeline v2 разбивает файл на сущности и собирает scope tree.
- [x] Renderer v2 рисует дерево с кликабельными cross_refs и поддиаграммами.
- [x] `cargo test`, `clippy`, `pnpm typecheck/build` — зелёные.
- [x] Graphloom.app переустановлен и отчёты abyss выглядят чётко.
