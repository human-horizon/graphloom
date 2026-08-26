# Graphloom MVP — каркас приложения

## Контекст

Начинаем новый проект **Graphloom** — инструмент, который строит понятную архитектуру кода
в виде инфографики и диаграмм. Целевые языки анализа: Go, Rust, TypeScript.

Приложение — десктоп на **Tauri**:
- **Backend (Rust):** сбор исходников, вызов LLM через OpenAI-совместимый endpoint
  (локальный llama.cpp, Ollama и т.п.), генерация HTML-файлов на диск.
- **Frontend (SolidJS + Tailwind v4 + tailwind-variants):** экран настроек,
  конструктор типов элементов, отображение сгенерированных HTML-файлов с диска.

**Ключевое решение:** никакого парсинга (ни tree-sitter, ни regex). Анализ архитектуры
делает LLM. Пользователь определяет палитру типов элементов (конструктор), AI раскладывает
код по этим элементам.

## Цель

Поднять работающий каркас Tauri-приложения, в котором:
1. Пользователь настраивает LLM-endpoint (URL, model, api key) на экране настроек.
2. Пользователь создаёт типы элементов в конструкторе (название, цвет, иконка, описание).
3. Пользователь выбирает папку репозитория → Rust собирает исходники → LLM анализирует
   и возвращает JSON-граф → Rust рендерит self-contained HTML в `.graphloom/`.
4. Фронтенд показывает список сгенерированных HTML-файлов и отображает выбранный.

## Что изменится

Всё создаётся с нуля в `/Users/a/Space/HumanHorizon/graphloom`:

1. Конфиги фронта: `package.json`, Vite + SolidJS + Tailwind v4 (`@tailwindcss/vite`) + tailwind-variants
2. `src/` — фронтенд:
   - `App.tsx` — навигация: экраны «Настройки», «Конструктор», «Отчёты»
   - `screens/Settings.tsx` — форма endpoint'а (URL, model, api key) + кнопка «Проверить соединение»
   - `screens/PaletteBuilder.tsx` — конструктор типов элементов (CRUD: имя, цвет, иконка, описание)
   - `screens/Reports.tsx` — выбор папки, запуск анализа, список отчётов, просмотрщик (iframe `srcdoc`), панель запуска тестов
   - `lib/ipc.ts` — типизированные обёртки над Tauri `invoke`
3. `src-tauri/` — Rust-ядро:
   - `Cargo.toml`, `tauri.conf.json`, `build.rs`
   - `src/main.rs`, `src/lib.rs`
   - `src/settings.rs` — хранение настроек и палитры в JSON в config-директории приложения
   - `src/collect.rs` — обход директории, сбор файлов `.go`, `.rs`, `.ts`, `.tsx`
     (игнор: `.git`, `node_modules`, `target`, `.graphloom`), усечение по лимиту токенов
   - `src/llm.rs` — клиент OpenAI-совместимого API: `chat/completions`, проверка соединения (`/models`),
     запрос анализа со строгим JSON-ответом, ретрай при невалидном JSON (1 повтор)
   - `src/graph.rs` — модель результата LLM:
     ```rust
     pub struct AnalysisResult { pub elements: Vec<Element>, pub edges: Vec<Edge> }
     pub struct Element { pub id: String, pub element_type: String, pub label: String,
                          pub files: Vec<String>, pub summary: String }
     pub struct Edge { pub from: String, pub to: String, pub label: Option<String> }
     ```
   - `src/render.rs` — генерация self-contained HTML (inline CSS/JS, диаграмма без внешних CDN),
     стили элементов подставляются из палитры пользователя
   - `src/commands.rs` — Tauri-команды:
     `get_settings`, `save_settings`, `check_connection`,
     `get_palette`, `save_palette`,
     `analyze_project(path) -> AnalyzeResult`, `list_reports(path)`, `read_report(path)`
4. `specs/` — спецификации (этот файл)
5. `.gitignore`, `README.md`

## Детали реализации

### Настройки и палитра

- Хранятся в `<config_dir>/graphloom/settings.json`:
  ```json
  { "endpoint": { "base_url": "http://localhost:8080/v1", "model": "...", "api_key": "" },
    "palette": [ { "type": "service", "label": "Сервис", "color": "#3b82f6", "icon": "⚙️", "description": "..." } ] }
  ```
- Палитра по умолчанию (если файла нет): `service`, `database`, `module`, `library`, `entrypoint`.

### Сбор кода (`collect.rs`)

- Рекурсивный обход через `walkdir`, фильтр по расширениям.
- Для каждого файла: относительный путь + содержимое (усечение файла до 400 строк).
- Общий лимит промпта: ~100k символов, при превышении — приоритет по размеру, остальные файлы
  перечисляются только путями (список).

### LLM-анализ (`llm.rs`)

- HTTP-клиент `reqwest`, endpoint `{base_url}/chat/completions`.
- System-промпт содержит палитру пользователя и требование ответить **строго JSON**
  схемы `AnalysisResult` (elements/edges). `response_format: {"type": "json_object"}` если поддерживается.
- Ответ валидируется через `serde`; при ошибке парсинга — 1 ретрай с уточняющим сообщением,
  затем понятная ошибка пользователю.
- `check_connection` — GET `{base_url}/models`, возвращает список моделей или ошибку.

### Рендер (`render.rs`)

- HTML: self-contained, inline CSS/JS, простой force-layout на canvas.
- Цвет/иконка каждого элемента — из палитры по `element_type`; неизвестный тип — нейтральный серый.
- В отчёте: диаграмма + боковая панель с `summary` элемента при клике.
- Файл сохраняется в `path/.graphloom/report-<timestamp>.html`.
- `analyze_project` возвращает: `{ reportPath, elements, edges }`.

### Фронтенд

- Три экрана, простая навигация вкладками в `App.tsx`.
- Стили — Tailwind v4, варианты кнопок/карточек через `tailwind-variants`.
- `lib/ipc.ts` — строгие типы для всех команд (Settings, ElementType, AnalysisResult, ReportEntry).
- Просмотр отчёта — iframe с `srcdoc` (файл читается через `read_report`).

### Зависимости (ключевые)

- Rust: `tauri`, `serde`, `serde_json`, `walkdir`, `reqwest`, `tokio`, `chrono`, `anyhow`, `dirs`
- Frontend: `solid-js`, `@tauri-apps/api`, `@tauri-apps/plugin-dialog`, `tailwindcss@4`,
  `@tailwindcss/vite`, `tailwind-variants`

## Критерии приёмки

- [x] `pnpm install` проходит, `pnpm tauri dev` поднимает окно приложения
- [ ] Экран настроек: сохранение endpoint'а, «Проверить соединение» (код готов; проверить при запущенном LLM)
- [x] Конструктор: создание/редактирование/удаление типов элементов, сохранение между запусками
- [ ] Выбор папки → «Анализ» → в `.graphloom/` появляется HTML-файл (требует запущенный локальный LLM)
- [ ] HTML открывается автономно (без сети), элементы раскрашены по палитре пользователя (проверить после первого анализа)
- [x] Список отчётов в приложении, клик → отчёт виден в просмотрщике
- [ ] LLM недоступен / вернул невалидный JSON → понятная ошибка в UI, без паники (проверить живьём)
- [x] `cargo clippy` без ошибок, `pnpm tsc --noEmit` без ошибок
