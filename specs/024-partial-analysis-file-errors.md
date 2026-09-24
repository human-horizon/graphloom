# Частичный анализ проекта с ошибками компиляции по файлам

## Контекст

Go analyzer сейчас завершает работу с ошибкой при первой проблеме `go/packages`. Graphloom получает ошибку всего analyzer и помечает весь проект как не построенный, хотя часть файлов может быть разобрана. Из-за этого ошибка одного файла блокирует обработку остальных.

## Цель

Расширить UCM контракт ошибками компиляции с привязкой к файлу, возвращать частичную модель валидных пакетов и продолжать file-level обработку. Для ошибочного файла Graphloom должен сохранять DSL/HTML-схему с узлом ошибки компиляции, а остальные файлы — обрабатывать независимо.

## Что изменится

1. `analyzers/go/internal/analyzer/analyzer.go` — собирать частичную модель и ошибки по позициям файлов, не прерывая анализ валидных пакетов.
2. `analyzers/go/main.go` — сохранять частичную модель с полем `errors`.
3. `analyzers/ts/src/analyzer.ts` и `analyzers/ts/src/entities.ts` — добавить совместимое поле ошибок TypeScript-диагностики.
4. `src-tauri/src/ucm.rs` — добавить общий тип ошибки анализа и `errors` в UCM.
5. `src-tauri/src/pipeline.rs` — учитывать error-файлы в update plan, создавать error-схему и не блокировать валидные файлы.
6. `src/screens/Reports.tsx` — показывать report ошибочного файла, не запускать бесконечные повторные попытки и продолжать очередь валидных файлов.
7. `src-tauri/tests/e2e.rs` и analyzer tests — проверить частичный UCM и file error report.
8. `specs/024-partial-analysis-file-errors.md` — критерии и результаты.

## Детали реализации

1. Общий `AnalysisError` содержит `file`, `message`, `start_line`, `end_line`; поле `errors` имеет default-пустой массив для обратной совместимости старого UCM.
2. Go analyzer добавляет `packageModel` для проектного пакета даже при `pkg.Errors`, записывает диагностические позиции в `errors`, а type-dependent collection выполняет только для пакетов без ошибок. Фатальными остаются только отсутствие проекта, отсутствие пакетов и невозможность загрузить результат.
3. TypeScript analyzer добавляет ошибки `project.getPreEmitDiagnostics()` с source range; анализ доступных SourceFile продолжается.
4. `get_update_plan` объединяет файлы из packages и errors. Файл с актуальной ошибкой получает статус `error`; файл без ошибки остаётся `pending`/`ready` независимо от соседних ошибок.
5. `analyze_file` для файла с актуальными errors создаёт валидируемую file-level Visualization с `NodeKind::Error`, русским label «Ошибка компиляции» и summary с текстом compiler diagnostics, затем сохраняет обычные `.dsl.json` и `.html` через общий finish pipeline без LLM.
6. `update_file` сохраняет `FileState.error` и report path для error-схемы. UI показывает этот report при выборе файла; error-файлы с уже созданным report не повторяет автоматически, а после изменения исходника получает новый шанс анализа.
7. `updateAll` продолжает обрабатывать pending-файлы, даже когда часть error-файлов уже отображена; ошибка одного `update_file` не прерывает остальные.
8. Project/function reports используют частичный UCM и не должны падать из-за диагностик файла, который в них не участвует.

## Критерии приёмки

- [x] Go analyzer возвращает частичный JSON UCM с `errors`, а не завершается из-за ошибок одного пакета.
- [x] Валидные Go-файлы продолжают попадать в packages/symbols/entities.
- [x] Ошибка содержит конкретный файл, диапазон строк и исходный текст диагностики.
- [x] Graphloom file-level report ошибочного файла содержит error node и сохраняется в `.dsl.json`/`.html`.
- [x] В update plan ошибочный файл имеет статус `error`, остальные файлы обрабатываются независимо.
- [x] UI не повторяет бесконечно анализ актуально ошибочного файла и показывает его error report.
- [x] TypeScript diagnostics не блокируют доступные файлы.
- [x] `go build`, `go test`, `go vet`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, `CI=true pnpm typecheck` и `CI=true pnpm build` проходят.
- [x] На Abyss Graphloom отображает ошибки `internal/js`, но продолжает обработку остальных доступных файлов.
- [x] Graphloom переустановлен; коммиты не создаются.
