use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedCodeModel {
    pub language: String,
    #[serde(default)]
    pub packages: Vec<Package>,
    #[serde(default)]
    pub symbols: Vec<Symbol>,
    #[serde(default)]
    pub calls: Vec<Call>,
    #[serde(default)]
    pub effects: Vec<ExternalEffect>,
    #[serde(default)]
    pub entities: Vec<Entity>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub kind: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub symbol: String,
    #[serde(default)]
    pub callee: String,
    #[serde(default)]
    pub condition: String,
    pub source: SourceRange,
    #[serde(default, rename = "parent_id")]
    pub parent_id: String,
    #[serde(default)]
    pub children: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub dir: String,
    #[serde(default)]
    pub files: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolKind {
    Function,
    Method,
    Type,
    Interface,
    Variable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Symbol {
    pub id: String,
    pub kind: SymbolKind,
    pub name: String,
    pub package: String,
    pub source: SourceRange,
    #[serde(default)]
    pub signature: String,
    #[serde(default)]
    pub is_exported: bool,
    #[serde(default)]
    pub is_async: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceRange {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Call {
    pub from: String,
    pub to: String,
    pub source: SourceRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    Network,
    Database,
    FileSystem,
    Queue,
    Log,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalEffect {
    pub symbol: String,
    pub kind: EffectKind,
    pub detail: String,
    pub source: SourceRange,
}
