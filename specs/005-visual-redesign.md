# Graphloom — визуальный редизайн интерфейса

## Контекст

Текущий экран — технический scaffold: плоская навигация, пустая рамка просмотрщика,
нет onboarding, dashboard-метрик, визуальной иерархии и выразительной карты архитектуры.
Продукт должен сразу объяснять пользователю, что делать, а после анализа — показывать
полноценную визуальную историю системы.

## Цель

Создать цельный dark editorial интерфейс Graphloom с выразительным onboarding,
dashboard после анализа и интерактивными semantic-картами в HTML-отчётах.

## Что изменится

1. `src/App.tsx` — shell с sidebar, проектным контекстом, навигацией и статусом endpoint.
2. `src/screens/Reports.tsx` — onboarding empty state, toolbar, dashboard-метрики,
   список отчётов, выбор функции и панель запуска тестов.
3. `src/screens/Settings.tsx` — визуально оформленная форма подключения.
4. `src/screens/PaletteBuilder.tsx` — визуальная палитра типов с preview-карточками.
5. `src/index.css` — дизайн-токены, фон, типографика, декоративные grid/noise эффекты.
6. `src-tauri/src/render.rs` — отчёт как полноценная architecture canvas:
   цветные semantic cards, группы, легенда, zoom/pan, метрики, source inspector,
   фильтры слоёв и inferred/verified legend.
7. `src-tauri/src/pipeline.rs` — компактные контексты UCM для устранения переполнения LLM.

## Визуальное направление

- Тёмная editorial-палитра: `#09090b`, graphite panels, electric blue/lilac/green accents.
- Sidebar 240px, content max-width, крупные заголовки, моноширинные metadata labels.
- Не использовать пустые рамки как главный empty state.
- Карточки имеют смысловую цветовую полосу, иконку, summary и показатели связей/тестов.
- Кнопки содержат понятный глагол и визуальный приоритет.

## Критерии приёмки

- [ ] Первый экран без проекта содержит onboarding-карточку с объяснением pipeline и CTA.
- [ ] После выбора проекта видны project header, действия, метрики и список отчётов.
- [ ] Навигация Sidebar визуально различает Отчёты / Конструктор / Настройки.
- [ ] Конструктор показывает live preview созданного типа элемента.
- [ ] HTML-отчёт содержит dashboard header, legend, semantic cards, zoom/pan и inspector.
- [ ] `cargo test`, `cargo clippy --all-targets -- -D warnings`, `pnpm typecheck`, `pnpm build` проходят.
- [ ] Компактный UCM-контекст не отправляет полный UCM большого проекта в LLM.
