# Профессиональные иконки

## Контекст

В интерфейсе и HTML-отчётах используются emoji и текстовые символы (`⚙️`, `📦`, `▶`, `✕`).
Они зависят от системного шрифта и выглядят непоследовательно.

## Цель

Заменить emoji на единый набор бесплатных профессиональных иконок Lucide (MIT), включая
навигацию, действия, палитру и узлы отчёта.

## Что изменится

1. `package.json` / lockfile — `lucide-solid` для SolidJS.
2. `src/App.tsx`, `src/screens/Reports.tsx`, `src/screens/PaletteBuilder.tsx` — Lucide-компоненты.
3. `src-tauri/src/settings.rs` — идентификаторы Lucide вместо emoji в дефолтной палитре.
4. `src-tauri/src/render.rs` — встроенный набор SVG path для offline HTML без CDN.
5. `src-tauri/src/dsl.rs` / типы палитры — сохраняем имя иконки как строковый идентификатор.

## Критерии приёмки

- [x] В основном UI нет emoji-иконок — используются компоненты `lucide-solid`.
- [x] В дефолтной палитре нет emoji; старые emoji в settings.json мигрируют в Lucide IDs.
- [x] HTML-отчёт офлайн отображает встроенные Lucide-style SVG paths без CDN.
- [x] `pnpm typecheck`, `pnpm build`, `cargo test`, `cargo clippy --all-targets -- -D warnings` проходят.
