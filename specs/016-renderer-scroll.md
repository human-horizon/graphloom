# Renderer: вместо зума — скролл

## Контекст

В текущем renderer используется зум + панорамирование через SVG-трансформацию. Управление прячется за несколькими комбинациями (Space+drag, колёсико, кнопки, клавиши 0/1). Это лишняя сложность, которая усложняет скроллинг больших деревьев и поведение sidebar.

## Цель

Заменить зум и панорамирование на нативный скролл страницы. Сцена рисуется в её естественных координатах, окно `<svg>`/`<div>` делает общий overflow-auto, а пользователь использует колёсико, трекпад и полосы прокрутки для перемещения. Sidebar, клики по нодам и `⌘ + click` продолжают работать без изменений.

## Что изменится

1. `src-tauri/src/render.html` — убрать viewport-инструменты, `transform`, pan/zoom события; перейти на скролл-контейнер.
2. `src-tauri/tests/e2e.rs` — обновить проверки renderer-шаблона: нет упоминаний zoom, фитится скролл.
3. `specs/015-renderer-navigation-and-visuals.md` — пометить как устаревшую; добавить эту спеку.
4. `PROJECT_CONTEXT.md` — записать причину перехода и ссылку на новую спеку.

## Детали реализации

1. В `<div id="stage">` убрать `cursor: grab; touch-action: none` и класс `panning`.
2. Разрешить `overflow: auto` на `<div id="stage">`, убрать `transform` на внутренней SVG-группе `#scene`. SVG получает естественные размеры контента (`width="0" height="0"`, `viewBox` по контенту); полосы прокрутки появляются у `<div id="stage">`.
3. Удалить переменные `viewportScale`, `viewportX`, `viewportY`, `panState`, `suppressClickUntil`, `spacePressed`, и функции `applyViewport`, `fitViewport`, `zoomAt`, `zoomAroundCenter`, `resetZoom`, `refit`.
4. Убрать viewport-кнопки из панели (`#viewport-tools`, `#zoom-out`, `#zoom-in`, `#zoom-reset`, `#zoom-fit`).
5. Убрать wheel/pointerdown/pointermove/pointerup/keydown/keyup обработчики, связанные с зумом/паном. Оставить keydown для `Backspace`/`Esc`/других действий, не относящихся к viewport.
6. Не трогать `⌘ + click`/sidebar/навигацию/поиск — они продолжают работать через DOM-события вне SVG-трансформации.
7. Заменить проверки e2e (renderer-шаблон) на отсутствие `transform`, `viewportX`, `applyViewport` и наличие `overflow: auto` у `#stage`.

## Критерии приёмки

- [x] В `<div id="stage">` есть `overflow: auto` и нет `cursor: grab`/`touch-action: none`.
- [x] `<svg>` не использует `transform`/scale.
- [x] В шаблоне нет viewport-кнопок, `applyViewport`, `zoomAt`, `refit`, `panState`.
- [x] Колесо мыши и трекпад скроллят схему без перехвата в JavaScript.
- [x] Обычный клик по ноде открывает sidebar, `⌘ + click` выполняет переход — без изменений.
- [x] Проходят `cargo test`, `cargo clippy --all-targets -- -D warnings`, `pnpm typecheck`, `pnpm build`, `./scripts/install-mac.sh`.
