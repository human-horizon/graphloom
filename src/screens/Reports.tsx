import { For, Show, createEffect, createMemo, createSignal, onMount } from "solid-js";
import { Dynamic } from "solid-js/web";
import { Check, ChevronDown, ChevronRight, FileCode2, FileJson, FileType, Folder, FolderOpen, Map, Network, Play, RefreshCw, X } from "lucide-solid";
import { open } from "@tauri-apps/plugin-dialog";
import {
  analyzeFile,
  analyzeFunction,
  analyzeProject,
  detectTestCommands,
  getFileTree,
  getUpdatePlan,
  getSymbols,
  readReport,
  rebuildModel,
  rerenderReport,
  runTest,
  updateFile,
  type AnalyzeResult,
  type FileInfo,
  type UpdateFile,
  type SymbolInfo,
  type TestCommands,
  type TestKind,
  type TestRunResult,
} from "../lib/ipc";

const KINDS: { kind: TestKind; label: string }[] = [
  { kind: "unit", label: "Unit" },
  { kind: "integration", label: "Integration" },
  { kind: "e2e", label: "E2E" },
];

type FileTreeNode = { name: string; path: string; directory: boolean; language?: string; children: FileTreeNode[] };

function buildFileTree(files: FileInfo[]): FileTreeNode[] {
  const root: FileTreeNode[] = [];
  for (const file of files) {
    const parts = file.path.split("/");
    let level = root;
    let currentPath = "";
    parts.forEach((name, index) => {
      currentPath = currentPath ? `${currentPath}/${name}` : name;
      let node = level.find((item) => item.name === name);
      if (!node) {
        node = { name, path: currentPath, directory: index < parts.length - 1, language: index === parts.length - 1 ? file.language : undefined, children: [] };
        level.push(node);
        level.sort((a, b) => Number(b.directory) - Number(a.directory) || a.name.localeCompare(b.name));
      }
      level = node.children;
    });
  }
  return root;
}

function iconForFile(node: FileTreeNode) {
  if (node.directory) return node.children.length ? FolderOpen : Folder;
  if (node.language === "go") return FileCode2;
  if (node.language === "typescript" || node.language === "tsx") return FileType;
  return FileJson;
}

function FileTreeBranch(props: { nodes: FileTreeNode[]; expanded: Set<string>; selected: string | null; statuses: Record<string, UpdateFile>; toggle: (path: string) => void; select: (node: FileTreeNode) => void }) {
  return <For each={props.nodes}>{(node) => {
    const Icon = iconForFile(node);
    const isExpanded = () => props.expanded.has(node.path);
    return <div class="file-tree-branch">
      <button class={`file-tree-row ${props.selected === node.path ? "is-selected" : ""}`} onClick={() => node.directory ? props.toggle(node.path) : props.select(node)}>
        <span class="file-tree-chevron">{node.directory ? (isExpanded() ? <ChevronDown size={13} /> : <ChevronRight size={13} />) : null}</span>
        <Dynamic component={Icon} size={15} strokeWidth={1.7} />
        <span>{node.name}</span>
        <Show when={!node.directory && props.statuses[node.path]}>{(item) => <span class={`file-tree-status status-${item().status}`} title={item().error ?? item().status} />}</Show>
      </button>
      <Show when={node.directory && isExpanded()}><div class="file-tree-children"><FileTreeBranch nodes={node.children} expanded={props.expanded} selected={props.selected} statuses={props.statuses} toggle={props.toggle} select={props.select} /></div></Show>
    </div>;
  }}</For>;
}

export default function Reports() {
  const [projectPath, setProjectPath] = createSignal<string | null>(null);
  const [files, setFiles] = createSignal<FileInfo[]>([]);
  const [selectedFile, setSelectedFile] = createSignal<string | null>(null);
  const [expandedDirs, setExpandedDirs] = createSignal<Set<string>>(new Set());
  const [fileStatuses, setFileStatuses] = createSignal<Record<string, UpdateFile>>({});
  const [updateProgress, setUpdateProgress] = createSignal<{ completed: number; total: number; current: string | null } | null>(null);

  const [currentHtml, setCurrentHtml] = createSignal<string | null>(null);
  const [analyzing, setAnalyzing] = createSignal(false);
  const [message, setMessage] = createSignal<{ ok: boolean; text: string } | null>(null);
  const [testCommands, setTestCommands] = createSignal<TestCommands | null>(null);
  const [running, setRunning] = createSignal<TestKind | null>(null);
  const [testResults, setTestResults] = createSignal<Partial<Record<TestKind, TestRunResult>>>({});
  const [logOpen, setLogOpen] = createSignal<TestKind | null>(null);
  const [symbols, setSymbols] = createSignal<SymbolInfo[]>([]);
  const [selectedSymbol, setSelectedSymbol] = createSignal("");
  const tree = createMemo(() => buildFileTree(files()));
  const visibleSymbols = createMemo(() => symbols().filter((symbol) => !selectedFile() || symbol.file === selectedFile()));

  const refresh = async (path: string) => {
    setFiles(await getFileTree(path));
    const plan = await getUpdatePlan(path);
    setFileStatuses(Object.fromEntries(plan.files.map((file) => [file.path, file])));
    for (const file of plan.files) {
      if (file.status === "ready") {
        try {
          await rerenderReport(path, file.path);
        } catch (error) {
          console.warn(`rerender ${file.path} failed`, error);
        }
      }
    }
    setTestCommands(await detectTestCommands(path));
    try {
      const list = await getSymbols(path);
      setSymbols(list);
      const first = list.find((symbol) => symbol.file === selectedFile()) ?? list[0];
      setSelectedSymbol(first?.id ?? "");
    } catch {
      setSymbols([]);
      setSelectedSymbol("");
    }
  };

  const pickFolder = async () => {
    const selected = await open({ directory: true });
    if (typeof selected !== "string") return;
    setProjectPath(selected);
    setSelectedFile(null);
    setCurrentHtml(null);
    setMessage(null);
    setTestResults({});
    await refresh(selected);
  };

  const updateAll = async () => {
    const path = projectPath();
    if (!path) return;
    setAnalyzing(true);
    setMessage(null);
    try {
      const plan = await getUpdatePlan(path);
      const pending = plan.files.filter((file) => file.status === "pending");
      if (!pending.length) {
        setMessage({ ok: true, text: `Всё актуально · ${plan.cached} файлов из кэша` });
        return;
      }
      let completed = 0;
      setUpdateProgress({ completed, total: pending.length, current: pending[0].path });
      for (const file of pending) {
        setFileStatuses((current) => ({ ...current, [file.path]: { ...file, status: "analyzing", error: null } }));
        setUpdateProgress({ completed, total: pending.length, current: file.path });
        try {
          const result = await updateFile(path, file.path, file.hash);
          const ready = { ...file, status: "ready" as const, reportPath: result.reportPath, error: null };
          setFileStatuses((current) => ({ ...current, [file.path]: ready }));
          if (!currentHtml() || selectedFile() === file.path) {
            setSelectedFile(file.path);
            setCurrentHtml(await readReport(result.reportPath));
          }
        } catch (error) {
          setFileStatuses((current) => ({ ...current, [file.path]: { ...file, status: "error", error: String(error) } }));
        }
        completed += 1;
        setUpdateProgress({ completed, total: pending.length, current: completed < pending.length ? pending[completed].path : null });
      }
      const failed = Object.values(fileStatuses()).filter((file) => file.status === "error").length;
      setMessage({ ok: failed === 0, text: failed ? `Готово · ошибок: ${failed}` : `Готово · обновлено файлов: ${pending.length}` });
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setAnalyzing(false);
      setUpdateProgress(null);
      await refresh(path);
    }
  };

  onMount(() => {
    const handler = async (event: MessageEvent) => {
      const data = event.data;
      if (data?.type === "graphloom:navigate" && typeof data.symbolId === "string") {
        const symbolId = data.symbolId;
        const symbol = symbols().find((s) => s.id === symbolId);
        const filePath = typeof data.file === "string" ? data.file : symbol?.file;
        if (filePath) {
          await selectFile({ name: filePath.split("/").pop() ?? filePath, path: filePath, directory: false, children: [] });
          if (symbol) {
            setSelectedSymbol(symbol.id);
            try {
              const result = await analyzeFunction(projectPath()!, symbol.id);
              setMessage({ ok: true, text: `Flow · ${result.nodes} узлов · ${result.edges} связей` });
              await refresh(projectPath()!);
              setCurrentHtml(await readReport(result.reportPath));
            } catch (error) {
              console.warn(`navigate analyzeFunction failed`, error);
            }
          }
        } else {
          setMessage({ ok: false, text: `Символ ${symbolId} не найден в модели` });
        }
      }
    };
    window.addEventListener("message", handler);
    return () => window.removeEventListener("message", handler);
  });

  const selectFile = async (node: FileTreeNode) => {
    setSelectedFile(node.path);
    const first = symbols().find((symbol) => symbol.file === node.path);
    setSelectedSymbol(first?.id ?? "");
    const cached = fileStatuses()[node.path];
    if (cached?.status === "ready" && cached.reportPath) {
      setCurrentHtml(await readReport(cached.reportPath));
      setMessage(null);
    } else {
      setMessage({ ok: false, text: "Файл ожидает генерации. Нажмите Update." });
    }
  };

  createEffect(() => {
    const file = selectedFile();
    if (!file) return;
    const parts = file.split("/");
    setExpandedDirs((current) => {
      const next = new Set(current);
      let path = "";
      for (const part of parts.slice(0, -1)) {
        path = path ? `${path}/${part}` : part;
        next.add(path);
      }
      return next;
    });
  });
  const toggleDirectory = (path: string) => {
    setExpandedDirs((current) => {
      const next = new Set(current);
      next.has(path) ? next.delete(path) : next.add(path);
      return next;
    });
  };

  const analyzeProjectMap = async () => {
    const path = projectPath();
    if (!path) return;
    setAnalyzing(true);
    setMessage(null);
    try {
      const result = await analyzeProject(path);
      setMessage({ ok: true, text: `Project map · ${result.nodes} узлов · ${result.edges} связей` });
      await refresh(path);
      setSelectedFile(null);
      setCurrentHtml(await readReport(result.reportPath));
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setAnalyzing(false);
    }
  };

  const analyzeFn = async () => {
    const path = projectPath();
    const symbolId = selectedSymbol();
    if (!path || !symbolId) return;
    setAnalyzing(true);
    setMessage(null);
    try {
      const result = await analyzeFunction(path, symbolId);
      setMessage({ ok: true, text: `Flow · ${result.nodes} узлов · ${result.edges} связей` });
      await refresh(path);
      setCurrentHtml(await readReport(result.reportPath));
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setAnalyzing(false);
    }
  };

  const rebuild = async () => {
    const path = projectPath();
    if (!path) return;
    setAnalyzing(true);
    try {
      const count = await rebuildModel(path);
      setMessage({ ok: true, text: `Модель обновлена · ${count} символов` });
      await refresh(path);
    } catch (error) {
      setMessage({ ok: false, text: String(error) });
    } finally {
      setAnalyzing(false);
    }
  };

  const run = async (kind: TestKind) => {
    const path = projectPath();
    if (!path) return;
    setRunning(kind);
    try {
      const result = await runTest(path, kind);
      setTestResults((results) => ({ ...results, [kind]: result }));
    } catch (error) {
      setTestResults((results) => ({
        ...results,
        [kind]: { success: false, output: String(error), command: "" },
      }));
    } finally {
      setRunning(null);
    }
  };

  return (
    <div>
      <Show when={!projectPath()}>
        <section class="page-hero">
          <div>
            <div class="eyebrow">Architecture explorer / 01</div>
            <h1 class="page-title">Увидеть код как <em>систему.</em></h1>
            <p class="page-subtitle">
              Graphloom превращает структуру проекта в карту смыслов: от пакетов и сервисов до
              конкретных решений внутри функции.
            </p>
          </div>
        </section>
      </Show>

      <Show when={projectPath()} fallback={
        <section class="onboarding">
          <div class="onboarding-copy">
            <div class="eyebrow">Start with a repository</div>
            <h2>Сложное становится <span>видимым.</span></h2>
            <p>
              Выберите локальную папку. Статические анализаторы соберут объективную модель,
              а ваш AI объяснит её человеческим языком — без загрузки кода в облако.
            </p>
            <button class="action-btn" onClick={pickFolder}><FolderOpen size={16} /> Выбрать проект</button>
            <div class="steps-row">
              <div class="step"><span class="step-number">01</span> Репозиторий</div>
              <div class="step"><span class="step-number">02</span> AI-смысл</div>
              <div class="step"><span class="step-number">03</span> Живая карта</div>
            </div>
          </div>
          <div class="onboarding-art" aria-hidden="true">
            <div class="art-connector" /><div class="art-connector two" />
            <div class="art-card art-a"><strong>HTTP Gateway</strong><small>entrypoint · 12 calls</small><span class="art-line" /></div>
            <div class="art-card art-b"><strong>User service</strong><small>module · 84 symbols</small><span class="art-line" /></div>
            <div class="art-card art-c"><strong>PostgreSQL</strong><small>storage · verified</small><span class="art-line" /></div>
          </div>
        </section>
      }>
        {(path) => (
          <>
            <div class="project-toolbar">
              <div class="project-path" title={path()}>{path()}</div>
              <button class="action-btn" disabled={analyzing()} onClick={updateAll}>{analyzing() ? "Обновляю…" : <><RefreshCw size={15} /> Update</>}</button>
              <button class="action-btn secondary" disabled={analyzing()} onClick={analyzeProjectMap}><Map size={15} /> Project map</button>
              <select class="symbol-select" value={selectedSymbol()} onChange={(event) => setSelectedSymbol(event.currentTarget.value)}>
                <For each={visibleSymbols()}>{(symbol) => <option value={symbol.id}>{symbol.name}</option>}</For>
              </select>
              <button class="action-btn secondary" disabled={analyzing() || !selectedSymbol()} onClick={analyzeFn}>Flow функции</button>
              <button class="action-btn secondary" disabled={analyzing()} onClick={rebuild}>Rescan</button>
            </div>

            <Show when={message()}>{(status) => <div class={`status-message ${status().ok ? "" : "error"}`}>{status().ok ? "● Готово · " : "× "}{status().text}</div>}</Show>
            <Show when={updateProgress()}>{(progress) => <div class="update-progress"><div class="update-progress-top"><strong>Update</strong><span>{progress().completed} / {progress().total}</span></div><div class="update-progress-track"><span style={{ width: `${progress().total ? (progress().completed / progress().total) * 100 : 0}%` }} /></div><small>{progress().current ? `Анализируется · ${progress().current}` : "Завершение очереди…"}</small></div>}</Show>

            <div class="dashboard-header"><h2>{path().split("/").pop() || "Project"}</h2><span>{symbols().length ? `${symbols().length} доступных функций` : "Модель ещё не собрана"}</span></div>
            <div class="report-layout">
              <aside class="file-tree-panel">
                <div class="file-tree-heading"><span>Project files</span><small>{files().length} files</small></div>
                <Show when={files().length} fallback={<div class="file-tree-empty">Выберите папку проекта</div>}>
                  <div class="file-tree"><FileTreeBranch nodes={tree()} expanded={expandedDirs()} selected={selectedFile()} statuses={fileStatuses()} toggle={toggleDirectory} select={selectFile} /></div>
                </Show>
              </aside>
              <section class="report-view">
                <Show when={testCommands()}>
                  {(commands) => <div class="test-panel"><span class="test-panel-label">Run checks</span><For each={KINDS}>{({ kind, label }) => {
                    const available = () => Boolean(commands()[kind]);
                    const result = () => testResults()[kind];
                    return <><button class="action-btn secondary" disabled={!available() || running() !== null} title={commands()[kind] ?? "Команда не найдена"} onClick={() => run(kind)}>{running() === kind ? "…" : <Play size={13} />} {label}</button><Show when={result()}>{(item) => <button class="test-result" onClick={() => setLogOpen(logOpen() === kind ? null : kind)}>{item().success ? <Check size={17} /> : <X size={17} />}</button>}</Show></>;
                  }}</For><Show when={logOpen()}>{(kind) => <pre class="test-log">{testResults()[kind()]?.output ?? ""}</pre>}</Show></div>}
                </Show>
                <div class="report-frame">
                  <Show when={currentHtml()} fallback={<div class="report-empty"><div><div class="empty-symbol"><Network size={30} strokeWidth={1.4} /></div><strong>Ваша архитектурная карта появится здесь</strong><small>Нажмите Update, чтобы построить диаграммы файлов</small></div></div>}>
                    {(html) => <iframe title="Graphloom architecture report" srcdoc={html()} />}
                  </Show>
                </div>
              </section>
            </div>
          </>
        )}
      </Show>
    </div>
  );
}
