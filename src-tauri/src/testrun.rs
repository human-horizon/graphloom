use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestKind {
    Unit,
    Integration,
    E2e,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct TestCommands {
    pub unit: Option<String>,
    pub integration: Option<String>,
    pub e2e: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TestRunResult {
    pub success: bool,
    pub output: String,
    pub command: String,
}

fn from_package_json(root: &Path, cmds: &mut TestCommands) {
    let Ok(raw) = std::fs::read_to_string(root.join("package.json")) else {
        return;
    };
    let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return;
    };
    let scripts = &json["scripts"];
    let pick = |names: &[&str]| {
        names
            .iter()
            .find(|n| scripts[**n].is_string())
            .map(|n| format!("pnpm run {n}"))
    };
    cmds.unit = pick(&["test:unit", "test"]);
    cmds.integration = pick(&["test:integration"]);
    cmds.e2e = pick(&["test:e2e", "e2e"]);
}

pub fn detect(root: &Path) -> TestCommands {
    let mut cmds = TestCommands::default();
    from_package_json(root, &mut cmds);
    if root.join("Cargo.toml").exists() {
        if cmds.unit.is_none() {
            cmds.unit = Some("cargo test".to_string());
        }
        if cmds.integration.is_none() {
            cmds.integration = Some("cargo test --test '*'".to_string());
        }
    }
    if root.join("go.mod").exists() {
        if cmds.unit.is_none() {
            cmds.unit = Some("go test ./...".to_string());
        }
        if cmds.integration.is_none() {
            cmds.integration = Some("go test -tags=integration ./...".to_string());
        }
    }
    cmds
}

pub async fn run(root: &Path, kind: TestKind) -> Result<TestRunResult> {
    let cmds = detect(root);
    let command = match kind {
        TestKind::Unit => cmds.unit,
        TestKind::Integration => cmds.integration,
        TestKind::E2e => cmds.e2e,
    }
    .context("no command detected for this test kind")?;

    // Shell is used so commands like `cargo test --test '*'` keep their quoting.
    let output = Command::new("sh")
        .arg("-c")
        .arg(&command)
        .current_dir(root)
        .output()
        .await
        .context("failed to spawn test process")?;

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(TestRunResult {
        success: output.status.success(),
        output: text,
        command,
    })
}
