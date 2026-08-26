use crate::graph::TestCoverage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Project,
    File,
    Function,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Action,
    Decision,
    Call,
    Input,
    Output,
    State,
    Storage,
    External,
    Loop,
    Error,
    Async,
    Group,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Flow,
    Calls,
    Data,
    State,
    Effects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeStatus {
    Verified,
    Inferred,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRef {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Branch {
    pub condition: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossRef {
    pub id: String,
    pub file: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizNode {
    pub id: String,
    pub kind: NodeKind,
    pub label: String,
    pub layer: Layer,
    #[serde(default)]
    pub source: Option<SourceRef>,
    #[serde(default)]
    pub element_type: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub tests: Option<TestCoverage>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub children: Vec<VizNode>,
    #[serde(default)]
    pub branches: Vec<Branch>,
    #[serde(default)]
    pub data_in: Vec<String>,
    #[serde(default)]
    pub data_out: Vec<String>,
    #[serde(default)]
    pub effects: Vec<String>,
    #[serde(default)]
    pub cross_refs: Vec<CrossRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VizEdge {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub status: Option<EdgeStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Visualization {
    pub title: String,
    pub level: Level,
    #[serde(default)]
    pub nodes: Vec<VizNode>,
    #[serde(default)]
    pub edges: Vec<VizEdge>,
}
