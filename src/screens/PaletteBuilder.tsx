import { For, Show, createSignal, onMount } from "solid-js";
import { Dynamic } from "solid-js/web";
import { Box, Database, Layers3, Library, Plus, Rocket, Save, Server, Settings2, Trash2 } from "lucide-solid";
import { getPalette, savePalette, type ElementType } from "../lib/ipc";

const ICONS = { Settings2, Database, Box, Library, Rocket, Server, Layers3 };
type IconName = keyof typeof ICONS;
const iconFor = (name: string) => ICONS[name as IconName] ?? Box;
const emptyType = (): ElementType => ({ type: "", label: "", color: "#6572e8", icon: "Box", description: "" });

export default function PaletteBuilder() {
  const [palette, setPalette] = createSignal<ElementType[]>([]);
  const [status, setStatus] = createSignal<string | null>(null);

  onMount(async () => setPalette(await getPalette()));
  const updateAt = (index: number, patch: Partial<ElementType>) => setPalette((items) => items.map((item, current) => current === index ? { ...item, ...patch } : item));
  const removeAt = (index: number) => setPalette((items) => items.filter((_, current) => current !== index));
  const save = async () => {
    try { await savePalette(palette()); setStatus("Палитра сохранена"); } catch (error) { setStatus(String(error)); }
  };

  return (
    <div class="palette-page">
      <section class="page-hero">
        <div><div class="eyebrow">Workspace / 02</div><h1 class="page-title">Ваш язык <em>архитектуры.</em></h1><p class="page-subtitle">Опишите собственные смысловые типы. AI будет использовать их, чтобы превратить техническую структуру в понятную карту.</p></div>
        <div class="hero-orbit" aria-hidden="true"><div class="orbit-core" /><span class="orbit-dot one" /><span class="orbit-dot two" /><span class="orbit-dot three" /></div>
      </section>
      <div class="palette-card"><div class="dashboard-header"><h2>Semantic palette</h2><span>{palette().length} element types</span></div>
        <div class="palette-grid">
          <div class="palette-items">
            <For each={palette()} fallback={<div class="preview-empty">Палитра пуста. Добавьте первый тип, чтобы начать.</div>}>
              {(item, index) => <div class="palette-item"><div class="palette-item-row"><select class="icon-select" value={item.icon} title="Иконка" onChange={(event) => updateAt(index(), { icon: event.currentTarget.value })}><For each={Object.keys(ICONS)}>{(name) => <option value={name}>{name}</option>}</For></select><input type="text" placeholder="type · service" value={item.type} onInput={(event) => updateAt(index(), { type: event.currentTarget.value })} /><input type="text" placeholder="Название · Сервис" value={item.label} onInput={(event) => updateAt(index(), { label: event.currentTarget.value })} /><input type="color" value={item.color} onInput={(event) => updateAt(index(), { color: event.currentTarget.value })} /><button class="action-btn ghost" title="Удалить тип" onClick={() => removeAt(index())}><Trash2 size={15} /></button></div><input class="palette-description" placeholder="Что относится к этому типу? Подсказка для AI" value={item.description} onInput={(event) => updateAt(index(), { description: event.currentTarget.value })} /></div>}
            </For>
            <div class="form-actions"><button class="action-btn secondary" onClick={() => setPalette((items) => [...items, emptyType()])}><Plus size={15} /> Добавить тип</button><button class="action-btn" onClick={save}><Save size={15} /> Сохранить палитру</button></div>
            <Show when={status()}>{(item) => <div class="status-message">● {item()}</div>}</Show>
          </div>
          <aside class="palette-preview"><h3>Live preview</h3><Show when={palette()[0]} fallback={<div class="preview-empty">Первый элемент палитры появится здесь в виде карточки.</div>}>
            {(item) => <div class="preview-node" style={{ "border-color": `${item().color}66` }}><div class="preview-icon" style={{ background: item().color }}><Dynamic component={iconFor(item().icon)} size={18} /></div><div><strong>{item().label || "Новый элемент"}</strong><small>{item().description || "Описание типа для AI"}</small></div></div>}
          </Show><div style={{ "margin-top": "17px" }} class="preview-empty">Эти цвета появятся на карте проекта и в легенде отчёта.</div></aside>
        </div>
      </div>
    </div>
  );
}
