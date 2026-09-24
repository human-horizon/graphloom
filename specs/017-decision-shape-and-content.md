# Условие как прямоугольник с подписью, а не ромб с пустым «Условие»

## Контекст

На схемах Graphloom блоки условий (`if`/`else`) рисуются ромбом, а контент — пустой. Analyzer передаёт `Condition` в UCM, semantic.rs подставляет условие в `summary` для ноды `Decision`, но:

1. LLM-метки в живом прогоне сейчас падают, поэтому `apply_labels` уходит в humanized fallback, который не сохраняет исходное условие и перезаписывает `label` на «Условие».
2. Renderer рисует `decision` как polygon (ромб), что выглядит не как часть потока.

## Цель

Сделать блок условия прямоугольным, читаемым, всегда содержащим реальный текст условия (`if <expr>`), даже когда LLM недоступен.

## Что изменится

1. `analyzers/go/internal/analyzer/entities.go` — подтвердить, что `IfStmt`/`Else` содержат ненулевой `Condition` и `Condition` не теряется на больших проектах.
2. `src-tauri/src/semantic.rs` — формировать `label` `if <condition>` / `else` для `Decision` и `loop <init>; <cond>; <post>` / `range <expr>` для `Loop`, сохранять условие и в label, и в summary.
3. `src-tauri/src/llm.rs` — fallback-метки (`humanize`) не должны затирать `summary` и должны выдавать человекочитаемый label для `if`/`else`/`loop`/`return`.
4. `src-tauri/src/render.html` — `decision` и `loop` рисовать как прямоугольник со скруглёнными углами (`<rect>`), `summary` показывать под label для всех видов нод.
5. `src-tauri/src/render.rs` — обновить проверки renderer-шаблона: ожидать наличие `polygon` не обязательно; ожидать `rect` и блок текста `nsummary` под нодой.

## Детали реализации

1. В `semantic::build_node`:
   - Для `kind == NodeKind::Decision` строить `label = format!("if {}", entity.condition)` (или `else` для empty condition у else-блока).
   - `summary` оставлять = `entity.condition` (повтор не страшен — есть где показать).
   - Для `kind == NodeKind::Loop` строить осмысленный label по `entity.name == "for"`/`"range"`.
   - Добавить `humanize` ветки: `"if" => "Условие"`, `"else" => "Иначе"`, `"loop" => "Цикл"`, `"return" => "Возврат"`, используемые только при пустой метке.
2. В `llm::apply_labels` (или его fallback): не затирать `summary`, если LLM не вернул валидный JSON.
3. В `render.html`:
   - Убрать `polygon` для `decision`/`loop`. Вместо этого использовать `<rect>`/`<rect rx>`, как для остальных нод.
   - `nsummary` показывать всегда (для decision/loop/call), позиционировать под label.
   - Иконка `GitBranch` для `decision` и `RefreshCw` для `loop` остаются.
4. Добавить Rust-юнит-тест, который строит `semantic` для функции с `if` и проверяет, что `label.contains("if ") && label.contains(entity.condition)`.
5. Прогнать Graphloom live на `abyss`, проверить HTML — условия читаемы.

## Критерии приёмки

- [ ] Условие на схеме отображается прямоугольником с текстом, включающим выражение `if …`.
- [ ] Цикл отображается прямоугольником с осмысленным label (`for`/`range` + тело).
- [ ] Даже если LLM недоступен, summary с реальным выражением присутствует в HTML.
- [ ] Проходят `cargo test`, `cargo clippy --all-targets -- -D warnings`, `pnpm typecheck`, `pnpm build`, `./scripts/install-mac.sh`.
