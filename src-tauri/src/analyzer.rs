use crate::ucm::UnifiedCodeModel;
use anyhow::{bail, Context, Result};
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command as StdCommand;
use tokio::process::Command;

const ANALYZERS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../analyzers");

/// Runs language sidecar analyzers and merges their output into one model.
pub async fn build_ucm(root: &Path) -> Result<UnifiedCodeModel> {
    let mut models = Vec::new();
    if root.join("go.mod").exists() {
        models.push(run_sidecar(&go_analyzer_command(root), true).await?);
    }
    if root.join("package.json").exists() || root.join("tsconfig.json").exists() {
        models.push(run_sidecar(&ts_analyzer_command(root), false).await?);
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
    requires_go: bool,
) -> Result<UnifiedCodeModel> {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd);
    if requires_go {
        let go = resolve_executable("go")?;
        command.env("PATH", path_with_executable_dir(&go)?);
    }
    let output = command
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

fn resolve_executable(name: &str) -> Result<PathBuf> {
    if let Some(path) = find_executable(name, env::var_os("PATH").as_deref()) {
        return Ok(path);
    }
    if let Some(path) = find_executable_via_login_shell(name) {
        return Ok(path);
    }
    bail!(
        "required executable `{name}` was not found; install Go or add it to your login shell PATH"
    )
}

fn find_executable(name: &str, path: Option<&OsStr>) -> Option<PathBuf> {
    let path = path?;
    env::split_paths(path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

fn find_executable_via_login_shell(name: &str) -> Option<PathBuf> {
    let shell = env::var_os("SHELL").unwrap_or_else(|| OsString::from("/bin/zsh"));
    let output = StdCommand::new(shell)
        .args(["-lc", &format!("command -v {name}")])
        .output()
        .ok()?;
    let output = String::from_utf8_lossy(&output.stdout);
    let path = PathBuf::from(output.lines().next()?.trim());
    if path.is_absolute() && path.is_file() {
        Some(path)
    } else {
        None
    }
}

fn path_with_executable_dir(executable: &Path) -> Result<OsString> {
    let directory = executable
        .parent()
        .context("resolved executable has no parent directory")?;
    let mut paths = vec![directory.to_path_buf()];
    if let Some(path) = env::var_os("PATH") {
        paths.extend(env::split_paths(&path));
    }
    env::join_paths(paths).context("failed to construct analyzer PATH")
}

fn merge(models: Vec<UnifiedCodeModel>) -> UnifiedCodeModel {
    let mut merged = UnifiedCodeModel {
        language: "mixed".to_string(),
        packages: Vec::new(),
        symbols: Vec::new(),
        calls: Vec::new(),
        effects: Vec::new(),
        entities: Vec::new(),
        errors: Vec::new(),
    };
    for model in models {
        merged.packages.extend(model.packages);
        merged.symbols.extend(model.symbols);
        merged.calls.extend(model.calls);
        merged.effects.extend(model.effects);
        merged.entities.extend(model.entities);
        merged.errors.extend(model.errors);
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_with_executable_dir_preserves_existing_path() {
        let existing = OsStr::new("/usr/bin:/bin");
        let path = path_with_dir(Path::new("/opt/homebrew/bin"), Some(existing)).unwrap();
        let paths: Vec<PathBuf> = env::split_paths(&path).collect();

        assert_eq!(paths[0], PathBuf::from("/opt/homebrew/bin"));
        assert_eq!(
            paths[1..],
            [PathBuf::from("/usr/bin"), PathBuf::from("/bin")]
        );
    }

    #[test]
    fn find_executable_uses_only_existing_path_entries() {
        let path = env::join_paths([Path::new("/definitely-missing")]).unwrap();
        assert_eq!(find_executable("go", Some(&path)), None);
    }

    fn path_with_dir(directory: &Path, existing: Option<&OsStr>) -> Result<OsString> {
        let mut paths = vec![directory.to_path_buf()];
        if let Some(path) = existing {
            paths.extend(env::split_paths(path));
        }
        env::join_paths(paths).context("failed to construct test PATH")
    }
}
