import { invoke } from "@tauri-apps/api/core";

export interface Endpoint {
  base_url: string;
  model: string;
  api_key: string;
}

export interface ElementType {
  type: string;
  label: string;
  color: string;
  icon: string;
  description: string;
}

export interface Settings {
  endpoint: Endpoint;
  palette: ElementType[];
}

export interface AnalyzeResult {
  reportPath: string;
  nodes: number;
  edges: number;
}

export interface SymbolInfo {
  id: string;
  name: string;
  package: string;
  file: string;
}

export interface FileInfo {
  path: string;
  name: string;
  language: string;
}

export type FileUpdateStatus = "ready" | "pending" | "analyzing" | "error";

export interface UpdateFile {
  path: string;
  hash: string;
  status: FileUpdateStatus;
  reportPath: string | null;
  error: string | null;
}

export interface UpdatePlan {
  total: number;
  pending: number;
  cached: number;
  files: UpdateFile[];
}

export interface ReportEntry {
  name: string;
  path: string;
  createdAt: string;
}

export function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export function saveSettings(settings: Settings): Promise<void> {
  return invoke("save_settings", { settings });
}

export function checkConnection(): Promise<string[]> {
  return invoke("check_connection");
}

export function getPalette(): Promise<ElementType[]> {
  return invoke("get_palette");
}

export function savePalette(palette: ElementType[]): Promise<void> {
  return invoke("save_palette", { palette });
}

export function analyzeProject(path: string): Promise<AnalyzeResult> {
  return invoke("analyze_project", { path });
}

export function analyzeFunction(path: string, symbolId: string): Promise<AnalyzeResult> {
  return invoke("analyze_function", { path, symbolId });
}

export function analyzeFile(path: string, file: string): Promise<AnalyzeResult> {
  return invoke("analyze_file", { path, file });
}

export function getFileTree(path: string): Promise<FileInfo[]> {
  return invoke("get_file_tree", { path });
}

export function getUpdatePlan(path: string): Promise<UpdatePlan> {
  return invoke("get_update_plan", { path });
}

export function updateFile(path: string, file: string, expectedHash: string): Promise<AnalyzeResult> {
  return invoke("update_file", { path, file, expectedHash });
}

export function rerenderReport(path: string, file: string): Promise<string> {
  return invoke("rerender_report", { path, file });
}

export function getSymbols(path: string): Promise<SymbolInfo[]> {
  return invoke("get_symbols", { path });
}

export function rebuildModel(path: string): Promise<number> {
  return invoke("rebuild_model", { path });
}

export function listReports(path: string): Promise<ReportEntry[]> {
  return invoke("list_reports", { path });
}

export type TestKind = "unit" | "integration" | "e2e";

export type TestCommands = Record<TestKind, string | null>;

export interface TestRunResult {
  success: boolean;
  output: string;
  command: string;
}

export function detectTestCommands(path: string): Promise<TestCommands> {
  return invoke("detect_test_commands", { path });
}

export function runTest(path: string, kind: TestKind): Promise<TestRunResult> {
  return invoke("run_test", { path, kind });
}

export function readReport(path: string): Promise<string> {
  return invoke("read_report", { path });
}
