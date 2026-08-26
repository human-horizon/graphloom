use crate::settings::Settings;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const PROMPT_VERSION: &str = "file-map-v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub hash: String,
    pub cache_key: String,
    pub report_path: String,
    pub dsl_path: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProjectState {
    pub files: BTreeMap<String, FileState>,
}

pub fn state_path(root: &Path) -> PathBuf {
    root.join(".graphloom").join("state.json")
}

pub fn load(root: &Path) -> ProjectState {
    let path = state_path(root);
    let Ok(raw) = fs::read_to_string(path) else {
        return ProjectState::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn save(root: &Path, state: &ProjectState) -> Result<()> {
    let path = state_path(root);
    fs::create_dir_all(path.parent().context("state directory is missing")?)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, serde_json::to_vec_pretty(state)?)?;
    fs::rename(temporary, path)?;
    Ok(())
}

pub fn file_hash(root: &Path, relative_path: &str) -> Result<String> {
    let content = fs::read(root.join(relative_path))
        .with_context(|| format!("cannot read {relative_path}"))?;
    Ok(hash_bytes(&content))
}

pub fn source_fingerprint(root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut files = WalkDir::new(root)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| {
            let path = entry.path().strip_prefix(root).ok()?.to_path_buf();
            if path.components().any(|part| part.as_os_str() == ".graphloom") {
                return None;
            }
            let supported = matches!(path.extension().and_then(|item| item.to_str()), Some("go" | "ts" | "tsx"));
            supported.then_some(path)
        })
        .collect::<Vec<_>>();
    files.sort();
    for path in files {
        hasher.update(path.to_string_lossy().as_bytes());
        hasher.update([0]);
        hasher.update(fs::read(root.join(&path))?);
        hasher.update([0]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn cache_key(settings: &Settings) -> String {
    let payload = serde_json::json!({
        "prompt_version": PROMPT_VERSION,
        "model": settings.endpoint.model,
        "base_url": settings.endpoint.base_url,
        "palette": settings.palette,
    });
    hash_bytes(payload.to_string().as_bytes())
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
