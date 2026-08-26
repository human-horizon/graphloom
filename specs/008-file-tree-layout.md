# Дерево файлов и layout без глобального sidebar

## Контекст

Глобальная боковая навигация занимает место и не помогает исследовать код. Пользователю
нужно видеть структуру проекта и получать диаграмму конкретного файла по клику.

## Цель

Сделать основной экран Graphloom трёхзонным:

- компактное меню приложения — только по запросу;
- дерево файлов проекта слева;
- диаграмма файла/функции в центре;
- inspector отчёта открывается только при выборе узла.

## Что изменится

1. `src/App.tsx` — убрать постоянный sidebar и topbar, добавить компактное меню workspace.
2. `src/screens/Reports.tsx` — дерево файлов вместо списка отчётов; file click запускает
   file diagram; сохранённые отчёты доступны внутри дерева/контекстного списка.
3. `src/lib/ipc.ts` — `getFileTree`, `analyzeFile`, тип `FileEntry`.
4. `src-tauri/src/pipeline.rs` — `get_file_tree`, `analyze_file` и компактный file context.
5. `src-tauri/src/commands.rs` — Tauri-команды.
6. `src-tauri/src/llm.rs`, `src-tauri/prompts/file_map.md` — промпт диаграммы файла.
7. `src/index.css` — layout file tree и compact workspace menu.

## Критерии приёмки

- [x] На основном экране нет постоянного sidebar и верхней строки.
- [x] После выбора проекта показывается дерево исходных файлов с Lucide-иконками языка.
- [x] Клик по файлу запускает file diagram и открывает HTML в центре (mock E2E).
- [x] Выбор функции остаётся доступен внутри выбранного файла.
- [x] Settings и Palette открываются из компактного меню.
- [x] `cargo test`, `cargo clippy --all-targets -- -D warnings`, `pnpm typecheck`, `pnpm build` проходят.
