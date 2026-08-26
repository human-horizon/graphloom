import { createSignal, onMount, Show } from "solid-js";
import { Check, PlugZap, Save, X } from "lucide-solid";
import { checkConnection, getSettings, saveSettings, type Settings as SettingsData } from "../lib/ipc";

export default function Settings() {
  const [settings, setSettings] = createSignal<SettingsData | null>(null);
  const [status, setStatus] = createSignal<{ ok: boolean; text: string } | null>(null);
  const [saving, setSaving] = createSignal(false);
  const [checking, setChecking] = createSignal(false);

  onMount(async () => setSettings(await getSettings()));

  const update = (patch: Partial<SettingsData["endpoint"]>) => {
    const current = settings();
    if (current) setSettings({ ...current, endpoint: { ...current.endpoint, ...patch } });
  };

  const save = async () => {
    const current = settings();
    if (!current) return;
    setSaving(true);
    try {
      await saveSettings(current);
      setStatus({ ok: true, text: "Настройки сохранены" });
    } catch (error) {
      setStatus({ ok: false, text: String(error) });
    } finally {
      setSaving(false);
    }
  };

  const check = async () => {
    await save();
    setChecking(true);
    try {
      const models = await checkConnection();
      setStatus({ ok: true, text: `Соединение установлено · ${models.join(", ") || "модель доступна"}` });
    } catch (error) {
      setStatus({ ok: false, text: `Endpoint недоступен · ${String(error)}` });
    } finally {
      setChecking(false);
    }
  };

  return (
    <div class="settings-page">
      <section class="page-hero">
        <div><div class="eyebrow">Workspace / 03</div><h1 class="page-title">Подключение <em>мозга.</em></h1><p class="page-subtitle">Graphloom работает с любым OpenAI-совместимым endpoint. Локальный llama.cpp, Ollama или ваш private server.</p></div>
        <div class="hero-orbit" aria-hidden="true"><div class="orbit-core" /><span class="orbit-dot one" /><span class="orbit-dot two" /><span class="orbit-dot three" /></div>
      </section>
      <Show when={settings()} fallback={<div class="settings-card">Загружаю настройки…</div>}>
        {(current) => <div class="settings-card">
          <div class="settings-grid">
            <div class="field full"><label>Base URL</label><input value={current().endpoint.base_url} placeholder="http://localhost:8080/v1" onInput={(event) => update({ base_url: event.currentTarget.value })} /></div>
            <div class="field"><label>Model identifier</label><input value={current().endpoint.model} placeholder="qwen35-9b-q4_k_m.gguf" onInput={(event) => update({ model: event.currentTarget.value })} /></div>
            <div class="field"><label>API key</label><input type="password" value={current().endpoint.api_key} placeholder="Опционально для локальной модели" onInput={(event) => update({ api_key: event.currentTarget.value })} /></div>
          </div>
          <div class="form-actions"><button class="action-btn" disabled={saving()} onClick={save}>{saving() ? "Сохраняю…" : <><Save size={15} /> Сохранить настройки</>}</button><button class="action-btn secondary" disabled={checking()} onClick={check}>{checking() ? "Проверяю…" : <><PlugZap size={15} /> Проверить соединение</>}</button></div>
          <Show when={status()}>{(item) => <div class={`status-message ${item().ok ? "" : "error"}`}>{item().ok ? <Check size={14} /> : <X size={14} />}{item().text}</div>}</Show>
        </div>}
      </Show>
    </div>
  );
}
