use crate::pipeline::{self, FileInfo, SymbolInfo, UpdatePlan};
use crate::settings::{ElementType, Settings};
use crate::testrun::{TestCommands, TestKind, TestRunResult};
use crate::{llm, settings, testrun};
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResultPayload {
    report_path: String,
    nodes: usize,
    edges: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportEntry {
    name: String,
    path: String,
    created_at: String,
}

#[tauri::command]
pub fn get_settings() -> Settings {
    settings::load()
}

#[tauri::command]
pub fn save_settings(settings: Settings) -> Result<(), String> {
    settings::save(&settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn check_connection() -> Result<Vec<String>, String> {
    let s = settings::load();
    llm::check_connection(&s.endpoint)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_palette() -> Vec<ElementType> {
    settings::load().palette
}

#[tauri::command]
pub fn save_palette(palette: Vec<ElementType>) -> Result<(), String> {
    let mut s = settings::load();
    s.palette = palette;
    settings::save(&s).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn analyze_project(path: String) -> Result<AnalyzeResultPayload, String> {
    let root = PathBuf::from(&path);
    let s = settings::load();
    let out = pipeline::analyze_project(&root, &s)
        .await
        .map_err(|e| e.to_string())?;
    Ok(AnalyzeResultPayload {
        report_path: out.report_path.to_string_lossy().into_owned(),
        nodes: out.nodes,
        edges: out.edges,
    })
}

#[tauri::command]
pub async fn analyze_function(
    path: String,
    symbol_id: String,
) -> Result<AnalyzeResultPayload, String> {
    let root = PathBuf::from(&path);
    let s = settings::load();
    let out = pipeline::analyze_function(&root, &s, &symbol_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(AnalyzeResultPayload {
        report_path: out.report_path.to_string_lossy().into_owned(),
        nodes: out.nodes,
        edges: out.edges,
    })
}

#[tauri::command]
pub async fn get_update_plan(path: String) -> Result<UpdatePlan, String> {
    let root = PathBuf::from(&path);
    let s = settings::load();
    pipeline::get_update_plan(&root, &s)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_file(
    path: String,
    file: String,
    expected_hash: String,
) -> Result<AnalyzeResultPayload, String> {
    let root = PathBuf::from(&path);
    let s = settings::load();
    let out = pipeline::update_file(&root, &s, &file, &expected_hash)
        .await
        .map_err(|e| e.to_string())?;
    Ok(AnalyzeResultPayload {
        report_path: out.report_path.to_string_lossy().into_owned(),
        nodes: out.nodes,
        edges: out.edges,
    })
}

#[tauri::command]
pub fn rerender_report(path: String, file: String) -> Result<String, String> {
    let root = PathBuf::from(&path);
    let s = settings::load();
    pipeline::render_report_from_dsl(&root, &s, &file)
        .map(|report| report.to_string_lossy().into_owned())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_file_tree(path: String) -> Result<Vec<FileInfo>, String> {
    pipeline::get_file_tree(Path::new(&path))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn analyze_file(path: String, file: String) -> Result<AnalyzeResultPayload, String> {
    let root = PathBuf::from(&path);
    let s = settings::load();
    let out = pipeline::analyze_file(&root, &s, &file)
        .await
        .map_err(|e| e.to_string())?;
    Ok(AnalyzeResultPayload {
        report_path: out.report_path.to_string_lossy().into_owned(),
        nodes: out.nodes,
        edges: out.edges,
    })
}

#[tauri::command]
pub async fn get_symbols(path: String) -> Result<Vec<SymbolInfo>, String> {
    pipeline::get_symbols(Path::new(&path))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn rebuild_model(path: String) -> Result<usize, String> {
    let model = pipeline::rebuild_ucm(Path::new(&path))
        .await
        .map_err(|e| e.to_string())?;
    Ok(model.symbols.len())
}

#[tauri::command]
pub fn list_reports(path: String) -> Result<Vec<ReportEntry>, String> {
    let dir = PathBuf::from(&path).join(".graphloom");
    let mut entries = Vec::new();
    let Ok(read_dir) = fs::read_dir(&dir) else {
        return Ok(entries);
    };
    for entry in read_dir.filter_map(std::result::Result::ok) {
        let p = entry.path();
        if p.extension().and_then(|e| e.to_str()) != Some("html") {
            continue;
        }
        let created = entry
            .metadata()
            .and_then(|m| m.modified())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Local> = t.into();
                dt.format("%Y-%m-%d %H:%M:%S").to_string()
            })
            .unwrap_or_default();
        entries.push(ReportEntry {
            name: p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            path: p.to_string_lossy().into_owned(),
            created_at: created,
        });
    }
    entries.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(entries)
}

#[tauri::command]
pub fn read_report(path: String) -> Result<String, String> {
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn detect_test_commands(path: String) -> TestCommands {
    testrun::detect(&PathBuf::from(path))
}

#[tauri::command]
pub async fn run_test(path: String, kind: TestKind) -> Result<TestRunResult, String> {
    testrun::run(&PathBuf::from(path), kind)
        .await
        .map_err(|e| e.to_string())
}
