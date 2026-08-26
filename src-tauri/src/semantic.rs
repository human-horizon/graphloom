use crate::dsl::{Layer, NodeKind, SourceRef, VizNode, Visualization};
use crate::ucm::{Entity, UnifiedCodeModel};
use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
struct LabelMap {
    labels: HashMap<String, LabelEntry>,
}

#[derive(Debug, Deserialize)]
struct LabelEntry {
    #[serde(default)]
    label: String,
    #[serde(default)]
    summary: Option<String>,
}

/// Builds a scope-tree visualization for a single function/method entity.
pub fn from_function(symbol_id: &str, _source: &str, model: &UnifiedCodeModel) -> Result<Visualization> {
    let found = model
        .entities
        .iter()
        .find(|e| e.symbol == symbol_id && (e.kind == "function" || e.kind == "method"));

    let synthetic = found.is_none().then(|| {
        model.symbols.iter().find(|s| s.id == symbol_id).map(|s| Entity {
            id: format!("func:{}", s.id),
            kind: "function".to_string(),
            name: s.name.clone(),
            label: String::new(),
            symbol: s.id.clone(),
            callee: String::new(),
            condition: String::new(),
            source: s.source.clone(),
            parent_id: String::new(),
            children: Vec::new(),
        })
    }).flatten();

    let entity: &Entity = found
        .or(synthetic.as_ref())
        .context(format!("function entity for symbol '{}' not found", symbol_id))?;

    let by_id: HashMap<&str, &Entity> = model.entities.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut children_by_parent: HashMap<&str, Vec<&Entity>> = HashMap::new();
    for e in &model.entities {
        if e.source.file != entity.source.file {
            continue;
        }
        if e.parent_id.is_empty() {
            continue;
        }
        children_by_parent
            .entry(e.parent_id.as_str())
            .or_default()
            .push(e);
    }

    let root = build_node(entity, &by_id, &children_by_parent);
    let mut viz = Visualization {
        title: format!("{} — {}", entity.source.file, entity.name),
        level: crate::dsl::Level::Function,
        nodes: vec![root],
        edges: vec![],
    };
    attach_cross_refs(&mut viz, model);
    Ok(viz)
}

/// Builds a compact scope-tree visualization for the whole project.
pub fn from_project(_model: &UnifiedCodeModel) -> Visualization {
    let nodes = build_project_nodes(_model);
    let mut viz = Visualization {
        title: "Project map".to_string(),
        level: crate::dsl::Level::Project,
        nodes,
        edges: vec![],
    };
    attach_cross_refs(&mut viz, _model);
    viz
}

fn attach_cross_refs(viz: &mut Visualization, model: &UnifiedCodeModel) {
    let symbol_files: HashMap<&str, &str> = model
        .symbols
        .iter()
        .map(|s| (s.id.as_str(), s.source.file.as_str()))
        .collect();
    let symbol_names: HashMap<&str, &str> = model
        .symbols
        .iter()
        .map(|s| (s.id.as_str(), s.name.as_str()))
        .collect();

    let mut callers_by_target: HashMap<&str, Vec<&crate::ucm::Call>> = HashMap::new();
    for call in &model.calls {
        if symbol_files.contains_key(call.to.as_str()) {
            callers_by_target
                .entry(call.to.as_str())
                .or_default()
                .push(call);
        }
    }

    fn walk(
        nodes: &mut [VizNode],
        symbol_files: &HashMap<&str, &str>,
        symbol_names: &HashMap<&str, &str>,
        callers: &HashMap<&str, Vec<&crate::ucm::Call>>,
    ) {
        for node in nodes.iter_mut() {
            if let Some(symbol) = node.symbol.as_deref() {
                if node.kind == NodeKind::Call {
                    if let Some(&file) = symbol_files.get(symbol) {
                        node.cross_refs.push(crate::dsl::CrossRef {
                            id: symbol.to_string(),
                            file: file.to_string(),
                            label: format!(
                                "→ {} in {}",
                                symbol_names.get(symbol).copied().unwrap_or(symbol),
                                file
                            ),
                        });
                    }
                } else if let Some(caller_list) = callers.get(symbol) {
                    for call in caller_list {
                        if let Some(&file) = symbol_files.get(call.from.as_str()) {
                            node.cross_refs.push(crate::dsl::CrossRef {
                                id: call.from.clone(),
                                file: file.to_string(),
                                label: format!(
                                    "← {} in {}",
                                    symbol_names.get(call.from.as_str()).copied().unwrap_or(&call.from),
                                    file
                                ),
                            });
                        }
                    }
                }
            }
            walk(&mut node.children, symbol_files, symbol_names, callers);
        }
    }

    walk(&mut viz.nodes, &symbol_files, &symbol_names, &callers_by_target);
}

fn build_project_nodes(model: &UnifiedCodeModel) -> Vec<VizNode> {
    let mut package_nodes: Vec<VizNode> = Vec::new();
    for package in &model.packages {
        let mut file_nodes: Vec<VizNode> = Vec::new();
        for file in &package.files {
            let symbols: Vec<&crate::ucm::Symbol> = model
                .symbols
                .iter()
                .filter(|s| s.source.file == *file && (s.is_exported || s.package == "main" || s.package.ends_with("/main")))
                .collect();
            if symbols.is_empty() {
                continue;
            }
            let children = symbols
                .into_iter()
                .map(|s| {
                    let kind = match s.kind {
                        crate::ucm::SymbolKind::Function | crate::ucm::SymbolKind::Method => NodeKind::Group,
                        crate::ucm::SymbolKind::Type | crate::ucm::SymbolKind::Interface => NodeKind::Group,
                        _ => NodeKind::Action,
                    };
                    VizNode {
                        id: s.id.clone(),
                        kind,
                        label: s.name.clone(),
                        layer: Layer::Flow,
                        source: Some(SourceRef {
                            file: s.source.file.clone(),
                            start_line: s.source.start_line,
                            end_line: s.source.end_line,
                        }),
                        element_type: None,
                        symbol: Some(s.id.clone()),
                        summary: None,
                        tests: None,
                        confidence: None,
                        children: vec![],
                        branches: vec![],
                        data_in: vec![],
                        data_out: vec![],
                        effects: vec![],
                        cross_refs: vec![],
                    }
                })
                .collect();
            file_nodes.push(VizNode {
                id: format!("file:{}", file),
                kind: NodeKind::Group,
                label: file.rsplit_once('/').map(|(_, name)| name).unwrap_or(file).to_string(),
                layer: Layer::Flow,
                source: Some(SourceRef {
                    file: file.clone(),
                    start_line: 1,
                    end_line: 1,
                }),
                element_type: None,
                symbol: None,
                summary: None,
                tests: None,
                confidence: None,
                children,
                branches: vec![],
                data_in: vec![],
                data_out: vec![],
                effects: vec![],
                cross_refs: vec![],
            });
        }
        if file_nodes.is_empty() {
            continue;
        }
        file_nodes.sort_by(|a, b| a.id.cmp(&b.id));
        package_nodes.push(VizNode {
            id: package.id.clone(),
            kind: NodeKind::Group,
            label: package.name.clone(),
            layer: Layer::Flow,
            source: None,
            element_type: None,
            symbol: None,
            summary: None,
            tests: None,
            confidence: None,
            children: file_nodes,
            branches: vec![],
            data_in: vec![],
            data_out: vec![],
            effects: vec![],
            cross_refs: vec![],
        });
    }
    package_nodes.sort_by(|a, b| a.id.cmp(&b.id));
    package_nodes
}

fn extract_json(raw: &str) -> &str {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') {
        return trimmed;
    }
    let start = trimmed.find('{').unwrap_or(trimmed.len());
    let end = trimmed.rfind('}').map(|i| i + 1).unwrap_or(trimmed.len());
    &trimmed[start..end]
}

pub fn apply_labels(viz: &mut Visualization, labels_json: &str) -> Result<()> {
    let json = extract_json(labels_json);
    let map: LabelMap = serde_json::from_str(json).context("invalid labels JSON")?;
    fn walk(nodes: &mut [VizNode], map: &HashMap<String, LabelEntry>) {
        for node in nodes.iter_mut() {
            if let Some(entry) = map.get(&node.id) {
                if !entry.label.is_empty() {
                    node.label = entry.label.clone();
                }
                if entry.summary.is_some() {
                    node.summary = entry.summary.clone();
                }
            }
            walk(&mut node.children, map);
        }
    }
    walk(&mut viz.nodes, &map.labels);
    Ok(())
}
pub fn from_entities(file: &str, _source: &str, model: &UnifiedCodeModel) -> Visualization {
    let by_id: HashMap<&str, &Entity> = model.entities.iter().map(|e| (e.id.as_str(), e)).collect();
    let mut roots: Vec<&Entity> = Vec::new();
    let mut children_by_parent: HashMap<&str, Vec<&Entity>> = HashMap::new();
    for entity in &model.entities {
        if entity.source.file != file {
            continue;
        }
        if entity.parent_id.is_empty() {
            roots.push(entity);
        } else {
            children_by_parent
                .entry(entity.parent_id.as_str())
                .or_default()
                .push(entity);
        }
    }

    let file_root = roots.into_iter().find(|e| e.kind == "file").cloned();
    let file_root: Entity = file_root.unwrap_or(Entity {
        id: format!("file:{file}"),
        kind: "file".to_string(),
        name: file.to_string(),
        label: String::new(),
        symbol: String::new(),
        callee: String::new(),
        condition: String::new(),
        source: crate::ucm::SourceRange {
            file: file.to_string(),
            start_line: 1,
            end_line: 1,
        },
        parent_id: String::new(),
        children: Vec::new(),
    });

    let root = build_node(&file_root, &by_id, &children_by_parent);
    let mut viz = Visualization {
        title: file.to_string(),
        level: crate::dsl::Level::File,
        nodes: vec![root],
        edges: vec![],
    };
    attach_cross_refs(&mut viz, model);
    viz
}

fn build_node<'a>(
    entity: &'a Entity,
    _by_id: &HashMap<&str, &'a Entity>,
    children_by_parent: &HashMap<&str, Vec<&'a Entity>>,
) -> VizNode {
    let children: Vec<VizNode> = children_by_parent
        .get(entity.id.as_str())
        .map(|list| {
            let mut list = list.to_vec();
            list.sort_by_key(|e| e.source.start_line);
            list.into_iter().map(|e| build_node(e, _by_id, children_by_parent)).collect()
        })
        .unwrap_or_default();

    // If this is a function, also fold call/if/loop children into the same list; the analyzer
    // already produced them as children of the function entity.
    let kind = match entity.kind.as_str() {
        "function" | "method" => NodeKind::Group,
        "type" | "interface" => NodeKind::Group,
        "call" => NodeKind::Call,
        "if" | "else" => NodeKind::Decision,
        "loop" => NodeKind::Loop,
        "return" => NodeKind::Output,
        "variable" => NodeKind::State,
        "file" => NodeKind::Group,
        _ => NodeKind::Action,
    };

    let label = if entity.label.is_empty() {
        humanize(&entity.name, entity.kind.as_str())
    } else {
        entity.label.clone()
    };

    let mut node = VizNode {
        id: entity.id.clone(),
        kind,
        label,
        layer: layer_for(entity.kind.as_str()),
        source: Some(SourceRef {
            file: entity.source.file.clone(),
            start_line: entity.source.start_line,
            end_line: entity.source.end_line,
        }),
        element_type: None,
        symbol: if entity.symbol.is_empty() { None } else { Some(entity.symbol.clone()) },
        summary: None,
        tests: None,
        confidence: None,
        children,
        branches: vec![],
        data_in: vec![],
        data_out: vec![],
        effects: vec![],
        cross_refs: vec![],
    };

    // For decision nodes, attach condition as summary if present.
    if kind == NodeKind::Decision && !entity.condition.is_empty() {
        node.summary = Some(entity.condition.clone());
    }

    // For call nodes, attach callee as summary and symbol for cross-file navigation.
    if kind == NodeKind::Call && !entity.callee.is_empty() {
        node.summary = Some(format!("→ {}", entity.callee));
        node.symbol = Some(entity.callee.clone());
    }

    node
}

fn layer_for(kind: &str) -> Layer {
    match kind {
        "function" | "method" | "type" | "interface" | "file" => Layer::Flow,
        "call" => Layer::Calls,
        "if" | "else" | "switch" | "case" => Layer::Flow,
        "loop" => Layer::Flow,
        "return" => Layer::Data,
        "variable" => Layer::State,
        _ => Layer::Flow,
    }
}

fn humanize(name: &str, kind: &str) -> String {
    if name.is_empty() {
        return kind.to_string();
    }
    match kind {
        "function" | "method" => format!("Функция {}", name),
        "type" => format!("Тип {}", name),
        "interface" => format!("Интерфейс {}", name),
        "variable" => format!("Переменная {}", name),
        "call" => format!("Вызов {}", name),
        "if" => "Условие".to_string(),
        "else" => "Иначе".to_string(),
        "loop" => "Цикл".to_string(),
        "return" => "Вернуть".to_string(),
        "file" => name.to_string(),
        _ => name.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_returns_plain_object() {
        let raw = r#"{"labels": {}}"#;
        assert_eq!(extract_json(raw), r#"{"labels": {}}"#);
    }

    #[test]
    fn extract_json_strips_markdown_fence() {
        let raw = "Here is the JSON:\n```json\n{\"labels\": {}}\n```\nDone.";
        assert_eq!(extract_json(raw), "{\"labels\": {}}");
    }

    #[test]
    fn extract_json_returns_empty_when_no_braces() {
        let raw = "no json here";
        assert_eq!(extract_json(raw), "");
    }
}
