use crate::ucm::UnifiedCodeModel;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use tokio::process::Command;

const ANALYZERS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../analyzers");

/// Runs language sidecar analyzers and merges their output into one model.
pub async fn build_ucm(root: &Path) -> Result<UnifiedCodeModel> {
    let mut models = Vec::new();
    if root.join("go.mod").exists() {
        models.push(run_sidecar(&go_analyzer_command(root)).await?);
    }
    if root.join("package.json").exists() || root.join("tsconfig.json").exists() {
        models.push(run_sidecar(&ts_analyzer_command(root)).await?);
    }
    match models.len() {
        0 => bail!("no Go or TypeScript project detected (need go.mod or package.json)"),
        1 => Ok(models.remove(0)),
        _ => Ok(merge(models)),
    }
}

fn go_analyzer_command(root: &Path) -> (PathBuf, Vec<String>, PathBuf) {
    let dir = PathBuf::from(ANALYZERS_DIR).join("go");
    let binary = dir.join("graphloom-analyze");
    if binary.exists() {
        (binary, vec![format!("-dir={}", root.display())], dir)
    } else {
        // Fallback: `go run .` works without a prebuilt binary.
        (
            PathBuf::from("go"),
            vec![
                "run".to_string(),
                ".".to_string(),
                format!("-dir={}", root.display()),
            ],
            dir,
        )
    }
}

fn ts_analyzer_command(root: &Path) -> (PathBuf, Vec<String>, PathBuf) {
    let dir = PathBuf::from(ANALYZERS_DIR).join("ts");
    (
        PathBuf::from("node"),
        vec![
            dir.join("dist/analyze.js").to_string_lossy().into_owned(),
            root.to_string_lossy().into_owned(),
        ],
        dir,
    )
}

async fn run_sidecar(
    (program, args, cwd): &(PathBuf, Vec<String>, PathBuf),
) -> Result<UnifiedCodeModel> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .with_context(|| format!("failed to spawn analyzer: {}", program.display()))?;
    if !output.status.success() {
        bail!(
            "analyzer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let model = serde_json::from_slice(&output.stdout).context("invalid UCM JSON from analyzer")?;
    Ok(model)
}

fn merge(models: Vec<UnifiedCodeModel>) -> UnifiedCodeModel {
    let mut merged = UnifiedCodeModel {
        language: "mixed".to_string(),
        packages: Vec::new(),
        symbols: Vec::new(),
        calls: Vec::new(),
        effects: Vec::new(),
        entities: Vec::new(),
    };
    for model in models {
        merged.packages.extend(model.packages);
        merged.symbols.extend(model.symbols);
        merged.calls.extend(model.calls);
        merged.effects.extend(model.effects);
        merged.entities.extend(model.entities);
    }
    merged
}
