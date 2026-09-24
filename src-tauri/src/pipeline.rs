use crate::analyzer;
use crate::dsl::{Layer, NodeKind, SourceRef, Visualization, VizNode};
use crate::llm::{self};
use crate::render;
use crate::semantic;
use crate::settings::Settings;
use crate::state;
use crate::ucm::{AnalysisError, SymbolKind, UnifiedCodeModel};
use crate::validate;
use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct SymbolInfo {
    pub id: String,
    pub name: String,
    pub package: String,
    pub file: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFile {
    pub path: String,
    pub hash: String,
    pub status: String,
    pub report_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePlan {
    pub total: usize,
    pub pending: usize,
    pub cached: usize,
    pub files: Vec<UpdateFile>,
}

pub struct PipelineOutput {
    pub report_path: PathBuf,
    pub dsl_path: PathBuf,
    pub nodes: usize,
    pub edges: usize,
    pub error: Option<String>,
}

/// Returns cached UCM or rebuilds it via sidecar analyzers.
pub async fn ucm(root: &Path) -> Result<UnifiedCodeModel> {
    let cache = root.join(".graphloom").join("ucm.json");
    let fingerprint_path = root.join(".graphloom").join("ucm.hash");
    let fingerprint = state::source_fingerprint(root)?;
    if let (Ok(raw), Ok(cached_fingerprint)) = (
        fs::read_to_string(&cache),
        fs::read_to_string(&fingerprint_path),
    ) {
        if cached_fingerprint == fingerprint {
            if let Ok(model) = serde_json::from_str::<UnifiedCodeModel>(&raw) {
                if model.entities.iter().any(|entity| entity.coverage) || !model.errors.is_empty() {
                    return Ok(model);
                }
            }
        }
    }
    let model = analyzer::build_ucm(root).await?;
    fs::create_dir_all(cache.parent().unwrap())?;
    fs::write(&cache, serde_json::to_string_pretty(&model)?)?;
    fs::write(&fingerprint_path, fingerprint)?;
    Ok(model)
}

pub async fn rebuild_ucm(root: &Path) -> Result<UnifiedCodeModel> {
    let cache = root.join(".graphloom").join("ucm.json");
    let fingerprint = root.join(".graphloom").join("ucm.hash");
    let _ = fs::remove_file(&cache);
    let _ = fs::remove_file(&fingerprint);
    ucm(root).await
}

pub async fn get_file_tree(root: &Path) -> Result<Vec<FileInfo>> {
    if !root.is_dir() {
        bail!("project path is not a directory: {}", root.display());
    }

    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().components().any(|component| {
            matches!(
                component.as_os_str().to_string_lossy().as_ref(),
                ".git" | ".graphloom" | "node_modules" | "target" | "dist"
            )
        }) {
            continue;
        }

        let Some(extension) = entry.path().extension().and_then(|item| item.to_str()) else {
            continue;
        };
        let language = match extension {
            "go" => "go",
            "tsx" => "tsx",
            "ts" => "typescript",
            _ => continue,
        };
        let path = entry
            .path()
            .strip_prefix(root)
            .context("failed to make project file path relative")?
            .to_string_lossy()
            .replace('\\', "/");
        let name = entry
            .path()
            .file_name()
            .map(|item| item.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.clone());
        files.push(FileInfo {
            path,
            name,
            language: language.to_string(),
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

pub async fn get_update_plan(root: &Path, settings: &Settings) -> Result<UpdatePlan> {
    let model = ucm(root).await?;
    let mut paths = model
        .packages
        .iter()
        .flat_map(|package| package.files.iter().cloned())
        .collect::<BTreeSet<_>>();
    paths.extend(
        model
            .errors
            .iter()
            .filter(|error| !error.file.is_empty())
            .map(|error| error.file.clone()),
    );
    let cache_key = state::cache_key(settings);
    let mut project_state = state::load(root);
    project_state.files.retain(|path, _| paths.contains(path));
    let mut files = Vec::new();
    let mut pending = 0;
    let mut cached = 0;
    for path in paths {
        let hash = state::file_hash(root, &path)?;
        let cached_state = project_state.files.get(&path);
        let state_matches =
            cached_state.is_some_and(|item| item.hash == hash && item.cache_key == cache_key);
        let report_path = cached_state.as_ref().and_then(|item| {
            (state_matches && !item.report_path.is_empty() && Path::new(&item.report_path).exists())
                .then(|| item.report_path.clone())
        });
        let compile_errors = model
            .errors
            .iter()
            .filter(|error| error.file == path)
            .map(format_analysis_error)
            .collect::<Vec<_>>();
        let error = if compile_errors.is_empty() {
            cached_state
                .as_ref()
                .and_then(|item| state_matches.then(|| item.error.clone()).flatten())
        } else {
            Some(compile_errors.join("; "))
        };
        let is_cached = error.is_none() && report_path.is_some();
        if is_cached {
            cached += 1;
        } else if error.is_none() {
            pending += 1;
        }
        files.push(UpdateFile {
            path,
            hash,
            status: if is_cached {
                "ready".to_string()
            } else if error.is_some() {
                "error".to_string()
            } else {
                "pending".to_string()
            },
            report_path,
            error,
        });
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    state::save(root, &project_state)?;
    Ok(UpdatePlan {
        total: files.len(),
        pending,
        cached,
        files,
    })
}

fn format_analysis_error(error: &AnalysisError) -> String {
    if error.start_line > 0 {
        format!("{}:{}: {}", error.file, error.start_line, error.message)
    } else {
        format!("{}: {}", error.file, error.message)
    }
}

fn errors_for_file<'a>(model: &'a UnifiedCodeModel, file: &str) -> Vec<&'a AnalysisError> {
    model
        .errors
        .iter()
        .filter(|error| error.file == file)
        .collect()
}

pub async fn update_file(
    root: &Path,
    settings: &Settings,
    file: &str,
    expected_hash: &str,
) -> Result<PipelineOutput> {
    let actual_hash = state::file_hash(root, file)?;
    if actual_hash != expected_hash {
        bail!("file '{file}' changed while waiting for generation");
    }
    let mut project_state = state::load(root);
    let cache_key = state::cache_key(settings);
    let output = analyze_file(root, settings, file).await;
    match output {
        Ok(out) => {
            project_state.files.insert(
                file.to_string(),
                state::FileState {
                    hash: actual_hash,
                    cache_key,
                    report_path: out.report_path.to_string_lossy().into_owned(),
                    dsl_path: out.dsl_path.to_string_lossy().into_owned(),
                    error: out.error.clone(),
                },
            );
            state::save(root, &project_state)?;
            Ok(out)
        }
        Err(e) => {
            project_state.files.insert(
                file.to_string(),
                state::FileState {
                    hash: actual_hash,
                    cache_key,
                    report_path: String::new(),
                    dsl_path: String::new(),
                    error: Some(e.to_string()),
                },
            );
            state::save(root, &project_state)?;
            Err(e)
        }
    }
}

pub fn render_report_from_dsl(root: &Path, settings: &Settings, file: &str) -> Result<PathBuf> {
    let mut project_state = state::load(root);
    let Some(entry) = project_state.files.get(file) else {
        bail!("file '{file}' has no cached report");
    };
    if !Path::new(&entry.dsl_path).exists() {
        bail!("DSL for '{file}' is missing; run Update");
    }
    let raw = fs::read_to_string(&entry.dsl_path)?;
    let mut viz: Visualization = serde_json::from_str(&raw)?;
    let model = ucm_sync(root)?;
    validate::validate(&mut viz, &model, &settings.palette)
        .map_err(|errors| anyhow::anyhow!(errors.join("; ")))?;
    let mut referenced = BTreeSet::new();
    collect_files(&viz.nodes, &mut referenced);
    let mut sources = BTreeMap::new();
    for source in referenced {
        if let Ok(content) = fs::read_to_string(root.join(&source)) {
            sources.insert(source, content);
        }
    }
    let html = render::render_html(&viz, &settings.palette, &sources)?;
    let report_path = entry.report_path.clone();
    fs::write(&report_path, html)?;
    if let Some(cached) = project_state.files.get_mut(file) {
        cached.report_path = report_path.clone();
    }
    state::save(root, &project_state)?;
    Ok(PathBuf::from(report_path))
}

fn ucm_sync(root: &Path) -> Result<UnifiedCodeModel> {
    let cache = root.join(".graphloom").join("ucm.json");
    let raw = fs::read_to_string(&cache)?;
    Ok(serde_json::from_str(&raw)?)
}

pub async fn get_symbols(root: &Path) -> Result<Vec<SymbolInfo>> {
    let model = ucm(root).await?;
    let mut out: Vec<SymbolInfo> = model
        .symbols
        .iter()
        .filter(|s| matches!(s.kind, SymbolKind::Function | SymbolKind::Method))
        .map(|s| SymbolInfo {
            id: s.id.clone(),
            name: s.name.clone(),
            package: s.package.clone(),
            file: s.source.file.clone(),
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

pub async fn analyze_project(root: &Path, settings: &Settings) -> Result<PipelineOutput> {
    let model = ucm(root).await?;

    let ucm_raw = fs::read_to_string(root.join(".graphloom").join("ucm.json"))?;
    let project_cache_key = format!(
        "{}-{}",
        state::cache_key(settings),
        state::hash_bytes(ucm_raw.as_bytes())
    );
    let mut project_state = state::load(root);
    if let Some(project) = &project_state.project {
        if project.cache_key == project_cache_key
            && fs::metadata(&project.report_path).is_ok()
            && fs::metadata(&project.dsl_path).is_ok()
        {
            let dsl_raw = fs::read_to_string(&project.dsl_path)?;
            let dsl: crate::dsl::Visualization = serde_json::from_str(&dsl_raw)?;
            return Ok(PipelineOutput {
                report_path: PathBuf::from(&project.report_path),
                dsl_path: PathBuf::from(&project.dsl_path),
                nodes: count_nodes(&dsl.nodes),
                edges: dsl.edges.len(),
                error: None,
            });
        }
    }

    let mut viz = semantic::from_project(&model);
    label_visualization_strict(&mut viz, "", settings).await?;
    validate::validate(&mut viz, &model, &settings.palette)
        .map_err(|errors| anyhow::anyhow!(errors.join("; ")))?;
    let out = finish(root, settings, viz, "project")?;
    project_state.project = Some(state::ProjectReportState {
        cache_key: project_cache_key,
        report_path: out.report_path.to_string_lossy().to_string(),
        dsl_path: out.dsl_path.to_string_lossy().to_string(),
    });
    state::save(root, &project_state)?;
    Ok(out)
}

pub async fn analyze_file(root: &Path, settings: &Settings, file: &str) -> Result<PipelineOutput> {
    let model = ucm(root).await?;
    let source =
        fs::read_to_string(root.join(file)).with_context(|| format!("cannot read {file}"))?;
    let compile_errors = errors_for_file(&model, file);
    if !compile_errors.is_empty() {
        let message = compile_errors
            .iter()
            .map(|error| format_analysis_error(error))
            .collect::<Vec<_>>()
            .join("; ");
        let viz = compile_error_visualization(file, &source, &compile_errors);
        return finish_with_error(root, settings, viz, "file", Some(message));
    }
    let mut viz = semantic::from_entities(file, &source, &model);
    label_visualization_strict(&mut viz, &source, settings).await?;
    validate::validate(&mut viz, &model, &settings.palette)
        .map_err(|errors| anyhow::anyhow!(errors.join("; ")))?;
    finish(root, settings, viz, "file")
}

pub async fn analyze_function(
    root: &Path,
    settings: &Settings,
    symbol_id: &str,
) -> Result<PipelineOutput> {
    let model = ucm(root).await?;
    let symbol = model
        .symbols
        .iter()
        .find(|s| s.id == symbol_id)
        .with_context(|| format!("symbol '{symbol_id}' not found in UCM"))?;

    let file_hash = state::file_hash(root, &symbol.source.file)?;
    let function_cache_key = format!("{}-{}-{}", state::cache_key(settings), file_hash, symbol_id);
    let mut project_state = state::load(root);
    if let Some(function_state) = project_state.functions.get(symbol_id) {
        if function_state.cache_key == function_cache_key
            && fs::metadata(&function_state.report_path).is_ok()
            && fs::metadata(&function_state.dsl_path).is_ok()
        {
            let dsl_raw = fs::read_to_string(&function_state.dsl_path)?;
            let dsl: crate::dsl::Visualization = serde_json::from_str(&dsl_raw)?;
            return Ok(PipelineOutput {
                report_path: PathBuf::from(&function_state.report_path),
                dsl_path: PathBuf::from(&function_state.dsl_path),
                nodes: count_nodes(&dsl.nodes),
                edges: dsl.edges.len(),
                error: None,
            });
        }
    }

    let source = fs::read_to_string(root.join(&symbol.source.file))
        .with_context(|| format!("cannot read {}", symbol.source.file))?;

    let mut viz = semantic::from_function(symbol_id, &source, &model)?;
    label_visualization_strict(&mut viz, &source, settings).await?;
    validate::validate(&mut viz, &model, &settings.palette)
        .map_err(|errors| anyhow::anyhow!(errors.join("; ")))?;
    let out = finish(root, settings, viz, "function")?;
    project_state.functions.insert(
        symbol_id.to_string(),
        state::FunctionState {
            cache_key: function_cache_key,
            report_path: out.report_path.to_string_lossy().to_string(),
            dsl_path: out.dsl_path.to_string_lossy().to_string(),
        },
    );
    state::save(root, &project_state)?;
    Ok(out)
}

const LABEL_MAX_TOKENS: u32 = 8192;
const LABEL_RETRY_LIMIT: usize = 3;

#[derive(Clone)]
struct LabelNode {
    id: String,
    kind: String,
    source: Option<SourceRef>,
    coverage_ids: Vec<String>,
    children: Vec<String>,
}

async fn label_visualization_strict(
    viz: &mut Visualization,
    source: &str,
    settings: &Settings,
) -> Result<()> {
    let nodes = label_nodes(&viz.nodes);
    if nodes.is_empty() {
        return Ok(());
    }

    let line_budget = (LABEL_MAX_TOKENS as usize / 64).max(24);
    let mut cursor = 0;
    let mut done_ids = BTreeSet::new();
    while cursor < nodes.len() {
        let first_line = nodes[cursor]
            .source
            .as_ref()
            .map(|source| source.start_line)
            .unwrap_or(1);
        let last_allowed_line = first_line.saturating_add(line_budget as u32 - 1);
        let mut end = cursor + 1;
        while end < nodes.len() {
            let node_line = nodes[end]
                .source
                .as_ref()
                .map(|source| source.start_line)
                .unwrap_or(first_line);
            if node_line > last_allowed_line {
                break;
            }
            end += 1;
        }
        let window = &nodes[cursor..end];
        let expected_ids = window
            .iter()
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        let window_ids = expected_ids.join("\n");
        let done_text = if done_ids.is_empty() {
            "нет".to_string()
        } else {
            done_ids.iter().cloned().collect::<Vec<_>>().join("\n")
        };
        let tree_json = serde_json::to_string_pretty(
            &window
                .iter()
                .map(|node| {
                    serde_json::json!({
                        "id": node.id,
                        "kind": node.kind,
                        "source": node.source,
                        "coverage_ids": node.coverage_ids,
                        "children": node.children,
                        "code": source_for_node(source, node.source.as_ref())
                    })
                })
                .collect::<Vec<_>>(),
        )?;
        let source_window = source_window(source, first_line, last_allowed_line, &done_ids, &nodes);
        let system = llm::entity_labels_prompt(
            &settings.palette,
            &tree_json,
            &source_window,
            &done_text,
            &window_ids,
        );
        let user = format!(
            "Опиши на русском каждый ID текущего окна. Верни ровно {} записей и не пропускай строки.",
            expected_ids.len()
        );
        let mut chat = llm::Chat::new(&settings.endpoint, system, user, LABEL_MAX_TOKENS);
        let mut applied = false;
        let mut last_error = String::new();
        for _ in 0..LABEL_RETRY_LIMIT {
            match chat.complete().await {
                Ok(labels_json) => {
                    match semantic::apply_labels_strict(viz, &labels_json, &expected_ids) {
                        Ok(()) => {
                            applied = true;
                            break;
                        }
                        Err(error) => {
                            last_error = error.to_string();
                            chat.push_user(format!(
                            "Ответ отклонён валидатором: {}. Исправь только текущее окно и верни полный JSON.",
                            last_error
                        ));
                        }
                    }
                }
                Err(error) => {
                    last_error = error.to_string();
                    chat.push_user(format!(
                        "Запрос не принят: {}. Верни корректный JSON для текущего окна.",
                        last_error
                    ));
                }
            }
        }
        if !applied {
            anyhow::bail!(
                "strict labeling failed for lines {}-{}: {}",
                first_line,
                last_allowed_line,
                last_error
            );
        }
        done_ids.extend(expected_ids);
        cursor = end;
    }
    semantic::merge_grouped_nodes(viz);
    Ok(())
}

fn label_nodes(nodes: &[VizNode]) -> Vec<LabelNode> {
    let mut out = Vec::new();
    fn walk(nodes: &[VizNode], out: &mut Vec<LabelNode>) {
        for node in nodes {
            out.push(LabelNode {
                id: node.id.clone(),
                kind: format!("{:?}", node.kind).to_lowercase(),
                source: node.source.clone(),
                coverage_ids: node.coverage_ids.clone(),
                children: node.children.iter().map(|child| child.id.clone()).collect(),
            });
            walk(&node.children, out);
        }
    }
    walk(nodes, &mut out);
    out.sort_by_key(|node| {
        node.source
            .as_ref()
            .map(|source| (source.start_line, source.end_line, node.id.clone()))
    });
    out
}

fn source_for_node(source: &str, source_ref: Option<&SourceRef>) -> String {
    let Some(source_ref) = source_ref else {
        return String::new();
    };
    numbered_source(source, source_ref.start_line, source_ref.end_line)
}

fn source_window(
    source: &str,
    start_line: u32,
    end_line: u32,
    done_ids: &BTreeSet<String>,
    nodes: &[LabelNode],
) -> String {
    if source.is_empty() {
        return "(Исходный текст не приложен: используйте точные source-диапазоны.)".to_string();
    }
    let done_ranges = nodes
        .iter()
        .filter(|node| done_ids.contains(&node.id) && !node.coverage_ids.is_empty())
        .filter_map(|node| node.source.as_ref())
        .collect::<Vec<_>>();
    let mut lines = numbered_source(source, start_line, end_line);
    if !done_ranges.is_empty() {
        lines = lines
            .lines()
            .map(|line| {
                let line_number = line
                    .split('|')
                    .next()
                    .and_then(|number| number.trim().parse::<u32>().ok())
                    .unwrap_or(0);
                if done_ranges
                    .iter()
                    .any(|range| range.start_line <= line_number && line_number <= range.end_line)
                {
                    format!("[СДЕЛАНО] {line}")
                } else {
                    format!("[ДАЛЬШЕ] {line}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    lines
}

fn numbered_source(source: &str, start_line: u32, end_line: u32) -> String {
    source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index as u32 + 1;
            (line_number >= start_line && line_number <= end_line)
                .then(|| format!("{line_number:04} | {line}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn compile_error_visualization(
    file: &str,
    _source: &str,
    errors: &[&AnalysisError],
) -> Visualization {
    let first = errors.first();
    let summary = errors
        .iter()
        .map(|error| format_analysis_error(error))
        .collect::<Vec<_>>()
        .join("\n");
    Visualization {
        title: file.to_string(),
        level: crate::dsl::Level::File,
        nodes: vec![VizNode {
            id: format!("error:{file}"),
            kind: NodeKind::Error,
            label: "Ошибка компиляции".to_string(),
            layer: Layer::Flow,
            source: Some(SourceRef {
                file: file.to_string(),
                start_line: first.map_or(1, |error| error.start_line),
                end_line: first.map_or(1, |error| error.end_line),
            }),
            element_type: None,
            symbol: None,
            summary: Some(summary),
            coverage_ids: vec![],
            group_id: None,
            tests: None,
            confidence: None,
            children: vec![],
            branches: vec![],
            data_in: vec![],
            data_out: vec![],
            effects: vec![],
            cross_refs: vec![],
        }],
        edges: vec![],
    }
}

/// Renders the visualization to a self-contained HTML report in `.graphloom/`.
fn finish(
    root: &Path,
    settings: &Settings,
    viz: Visualization,
    level: &str,
) -> Result<PipelineOutput> {
    finish_with_error(root, settings, viz, level, None)
}

fn finish_with_error(
    root: &Path,
    settings: &Settings,
    viz: Visualization,
    level: &str,
    error: Option<String>,
) -> Result<PipelineOutput> {
    let mut referenced = BTreeSet::new();
    collect_files(&viz.nodes, &mut referenced);
    let mut sources = BTreeMap::new();
    for file in referenced {
        if let Ok(content) = fs::read_to_string(root.join(&file)) {
            sources.insert(file, content);
        }
    }
    let html = render::render_html(&viz, &settings.palette, &sources)?;

    let dir = root.join(".graphloom");
    fs::create_dir_all(&dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let base = format!("report-{level}-{stamp}");
    let report_path = dir.join(format!("{base}.html"));
    let dsl_path = dir.join(format!("{base}.dsl.json"));
    fs::write(&report_path, html)?;
    fs::write(&dsl_path, serde_json::to_vec_pretty(&viz)?)?;

    Ok(PipelineOutput {
        report_path,
        dsl_path,
        nodes: count_nodes(&viz.nodes),
        edges: viz.edges.len(),
        error,
    })
}

fn count_nodes(nodes: &[crate::dsl::VizNode]) -> usize {
    nodes.iter().map(|n| 1 + count_nodes(&n.children)).sum()
}

fn collect_files(nodes: &[crate::dsl::VizNode], out: &mut BTreeSet<String>) {
    for node in nodes {
        if let Some(source) = &node.source {
            out.insert(source.file.clone());
        }
        collect_files(&node.children, out);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        compile_error_visualization, format_analysis_error, get_file_tree, source_window, LabelNode,
    };
    use crate::dsl::{NodeKind, SourceRef};
    use crate::ucm::AnalysisError;
    use std::collections::BTreeSet;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn compile_error_visualization_contains_diagnostic() {
        let error = AnalysisError {
            file: "internal/js/dom.go".to_string(),
            message: "InitDOM redeclared".to_string(),
            start_line: 118,
            end_line: 118,
        };
        let viz = compile_error_visualization("internal/js/dom.go", "package js", &[&error]);

        assert_eq!(viz.nodes.len(), 1);
        assert_eq!(viz.nodes[0].kind, NodeKind::Error);
        assert_eq!(viz.nodes[0].label, "Ошибка компиляции");
        assert!(viz.nodes[0]
            .summary
            .as_deref()
            .is_some_and(|summary| summary.contains("InitDOM redeclared")));
        assert_eq!(
            format_analysis_error(&error),
            "internal/js/dom.go:118: InitDOM redeclared"
        );
    }

    #[test]
    fn source_window_marks_processed_lines() {
        let node = LabelNode {
            id: "statement:main.go:2".to_string(),
            kind: "statement".to_string(),
            source: Some(SourceRef {
                file: "main.go".to_string(),
                start_line: 2,
                end_line: 2,
            }),
            coverage_ids: vec!["statement:main.go:2".to_string()],
            children: vec![],
        };
        let mut done = BTreeSet::new();
        done.insert(node.id.clone());
        let marked = source_window("first\nsecond\nthird", 1, 3, &done, &[node]);

        assert!(marked.contains("[СДЕЛАНО] 0002 | second"));
        assert!(marked.contains("[ДАЛЬШЕ] 0001 | first"));
    }

    #[tokio::test]
    async fn get_file_tree_lists_sources_without_analyzer() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time must be after unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("graphloom-file-tree-{unique}"));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::create_dir_all(root.join(".graphloom")).unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("main.go"), "package main").unwrap();
        fs::write(root.join("nested/app.ts"), "export const app = true").unwrap();
        fs::write(root.join("README.md"), "ignored").unwrap();
        fs::write(root.join(".graphloom/ignored.go"), "package ignored").unwrap();
        fs::write(root.join("node_modules/pkg/ignored.ts"), "ignored").unwrap();

        let files = get_file_tree(&root).await.unwrap();
        let paths = files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["main.go", "nested/app.ts"]);

        fs::remove_dir_all(root).unwrap();
    }
}
