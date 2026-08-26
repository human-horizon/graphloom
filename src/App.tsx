import { For, Show, createSignal } from "solid-js";
import { Dynamic } from "solid-js/web";
import { LayoutDashboard, Menu, Palette, Settings2, X } from "lucide-solid";
import Settings from "./screens/Settings";
import PaletteBuilder from "./screens/PaletteBuilder";
import Reports from "./screens/Reports";

type Screen = "reports" | "palette" | "settings";

const NAV_ITEMS: { id: Screen; label: string; icon: typeof LayoutDashboard; description: string }[] = [
  { id: "reports", label: "Обзор", icon: LayoutDashboard, description: "Карты и flow" },
  { id: "palette", label: "Язык карты", icon: Palette, description: "Типы элементов" },
  { id: "settings", label: "Подключение", icon: Settings2, description: "LLM endpoint" },
];

export default function App() {
  const [screen, setScreen] = createSignal<Screen>("reports");
  const [menuOpen, setMenuOpen] = createSignal(false);

  const selectScreen = (next: Screen) => {
    setScreen(next);
    setMenuOpen(false);
  };

  return (
    <div class="app-shell app-shell-flat">
      <section class="app-main">
        <button class="workspace-menu-toggle" aria-label="Открыть меню" onClick={() => setMenuOpen(!menuOpen())}>
          <Show when={menuOpen()} fallback={<Menu size={18} />}><X size={18} /></Show>
        </button>
        <Show when={menuOpen()}>
          <nav class="workspace-menu">
            <div class="workspace-menu-brand"><span class="brand-mark"><span>g</span></span><strong>Graphloom</strong></div>
            <For each={NAV_ITEMS}>
              {(item) => <button class={`workspace-menu-item ${screen() === item.id ? "is-active" : ""}`} onClick={() => selectScreen(item.id)}><Dynamic component={item.icon} size={16} /><span><strong>{item.label}</strong><small>{item.description}</small></span></button>}
            </For>
            <div class="workspace-menu-foot"><span class="status-dot" /> Local-first AI</div>
          </nav>
        </Show>
        <main class="app-content">
          <Show when={screen() === "reports"}><Reports /></Show>
          <Show when={screen() === "palette"}><PaletteBuilder /></Show>
          <Show when={screen() === "settings"}><Settings /></Show>
        </main>
      </section>
    </div>
  );
}
