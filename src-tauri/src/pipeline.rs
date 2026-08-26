use crate::analyzer;
use crate::dsl::Visualization;
use crate::llm::{self};
use crate::render;
use crate::semantic;
use crate::settings::Settings;
use crate::state;
use crate::ucm::{SymbolKind, UnifiedCodeModel};
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
            if let Ok(model) = serde_json::from_str(&raw) {
                return Ok(model);
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
    let model = ucm(root).await?;
    let mut paths = BTreeSet::new();
    for package in &model.packages {
        for file in &package.files {
            paths.insert(file.clone());
        }
    }
    let mut files = paths
        .into_iter()
        .map(|path| {
            let language = match Path::new(&path).extension().and_then(|item| item.to_str()) {
                Some("go") => "go",
                Some("tsx") => "tsx",
                Some("ts") => "typescript",
                _ => "code",
            };
            let name = Path::new(&path)
                .file_name()
                .map(|item| item.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            FileInfo {
                path,
                name,
                language: language.to_string(),
            }
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

pub async fn get_update_plan(root: &Path, settings: &Settings) -> Result<UpdatePlan> {
    let model = ucm(root).await?;
    let paths = model
        .packages
        .iter()
        .flat_map(|package| package.files.iter().cloned())
        .collect::<BTreeSet<_>>();
    let cache_key = state::cache_key(settings);
    let mut project_state = state::load(root);
    project_state.files.retain(|path, _| paths.contains(path));
    let mut files = Vec::new();
    let mut pending = 0;
    let mut cached = 0;
    for path in paths {
        let hash = state::file_hash(root, &path)?;
        let cached_state = project_state.files.get(&path);
        let report_path = cached_state.map(|item| item.report_path.clone());
        let is_cached = cached_state.is_some_and(|item| {
            item.hash == hash
                && item.cache_key == cache_key
                && Path::new(&item.report_path).exists()
        });
        if is_cached {
            cached += 1;
        } else {
            pending += 1;
        }
        files.push(UpdateFile {
            path,
            hash,
            status: if is_cached { "ready" } else { "pending" }.to_string(),
            report_path: if is_cached { report_path } else { None },
            error: None,
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
    let output = analyze_file(root, settings, file).await?;
    let mut project_state = state::load(root);
    project_state.files.insert(
        file.to_string(),
        state::FileState {
            hash: actual_hash,
            cache_key: state::cache_key(settings),
            report_path: output.report_path.to_string_lossy().into_owned(),
            dsl_path: output.dsl_path.to_string_lossy().into_owned(),
        },
    );
    state::save(root, &project_state)?;
    Ok(output)
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
            });
        }
    }

    let mut viz = semantic::from_project(&model);
    if !viz.nodes.is_empty() {
        let tree_json = serde_json::to_string_pretty(&viz)?;
        let system = llm::entity_labels_prompt(&settings.palette, &tree_json);
        let user = "Fill human-readable Russian labels and short summaries for every node. Respond with strict JSON only.".to_string();
        let mut chat = llm::Chat::new(&settings.endpoint, system, user, 8192);
        match chat.complete().await {
            Ok(labels_json) => {
                if let Err(error) = semantic::apply_labels(&mut viz, &labels_json) {
                    eprintln!("project labels warning: {error}");
                }
            }
            Err(error) => eprintln!("project labels request failed: {error}"),
        }
    }
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
    let mut viz = semantic::from_entities(file, &source, &model);
    if !viz.nodes.is_empty() {
        let tree_json = serde_json::to_string_pretty(&viz)?;
        let system = llm::entity_labels_prompt(&settings.palette, &tree_json);
        let user = "Fill human-readable Russian labels and short summaries for every node. Respond with strict JSON only.".to_string();
        let mut chat = llm::Chat::new(&settings.endpoint, system, user, 8192);
        match chat.complete().await {
            Ok(labels_json) => {
                if let Err(error) = semantic::apply_labels(&mut viz, &labels_json) {
                    eprintln!("file labels warning: {error}");
                }
            }
            Err(error) => eprintln!("file labels request failed: {error}"),
        }
    }
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
            });
        }
    }

    let source = fs::read_to_string(root.join(&symbol.source.file))
        .with_context(|| format!("cannot read {}", symbol.source.file))?;

    let mut viz = semantic::from_function(symbol_id, &source, &model)?;
    if !viz.nodes.is_empty() {
        let tree_json = serde_json::to_string_pretty(&viz)?;
        let system = llm::entity_labels_prompt(&settings.palette, &tree_json);
        let user = "Fill human-readable Russian labels and short summaries for every node. Respond with strict JSON only.".to_string();
        let mut chat = llm::Chat::new(&settings.endpoint, system, user, 8192);
        match chat.complete().await {
            Ok(labels_json) => {
                if let Err(error) = semantic::apply_labels(&mut viz, &labels_json) {
                    eprintln!("function labels warning: {error}");
                }
            }
            Err(error) => eprintln!("function labels request failed: {error}"),
        }
    }
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

/// Renders the visualization to a self-contained HTML report in `.graphloom/`.
fn finish(
    root: &Path,
    settings: &Settings,
    viz: Visualization,
    level: &str,
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
