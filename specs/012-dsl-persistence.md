# Сохранение DSL рядом с HTML

## Контекст

LLM возвращает только валидированный DSL JSON. HTML — это проекция этого JSON, которую
строит deterministic renderer. Сейчас хранится только HTML; смена renderer не может
перерисовать старые схемы без повторного LLM-запроса.

## Цель

Хранить DSL JSON вместе с HTML, чтобы любой renderer мог восстановить схему без LLM.
Инвалидация кэша по версии renderer заменяется автоматическим перерендером из сохранённого
DSL.

## Поведение

1. `finish()` после успешной генерации записывает HTML и `.dsl.json` рядом в `.graphloom/`.
2. При открытии проекта:
   - читаются все сохранённые `.dsl.json`;
   - для каждого файла, если есть и HTML и DSL, и DSL валиден, рендер пересоздаёт HTML;
   - LLM не вызывается.
3. Если `.dsl.json` отсутствует, файл помечается `pending` и требует Update.
4. Изменение содержимого файла или модели инвалидирует только DSL без DSL → требует LLM.
5. Старые HTML без DSL безопасно удаляются после первого успешного Update.

## Изменения

- `src-tauri/src/pipeline.rs` — `finish()` пишет и HTML и DSL; добавить `rerender_all()`.
- `src-tauri/src/commands.rs` — команды `get_project_reports`, `rerender_project`.
- `src/lib/ipc.ts` — соответствующие IPC-функции.
- `src/screens/Reports.tsx` — при открытии проекта перерендерить готовые отчёты;
  кнопка Update обращается к LLM только для pending файлов.
- `src-tauri/src/state.rs` — убрать renderer version из cache key (re-render сам
  обновит HTML).
- `src-tauri/tests/e2e.rs` — проверка, что rerender не вызывает LLM и читает сохранённый DSL.

## Критерии приёмки

- [x] `.dsl.json` сохраняется рядом с HTML после генерации.
- [x] При открытии проекта все готовые отчёты пересоздаются без обращения к LLM.
- [x] Изменение содержимого файла по-прежнему помечает запись как pending.
- [x] Mock E2E подтверждает, что rerender не вызывает LLM.
- [x] `cargo test`, `clippy`, `pnpm typecheck`, `pnpm build` проходят.
- [x] Graphloom.app переустановлен в `/Applications`.
