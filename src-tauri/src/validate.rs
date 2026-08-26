use crate::dsl::{EdgeStatus, Level, NodeKind, Visualization, VizNode};
use crate::settings::ElementType;
use crate::ucm::{Call, Symbol, UnifiedCodeModel};
use std::collections::{HashMap, HashSet};

const SCHEMA_JSON: &str = include_str!("dsl.schema.json");

/// Validates the LLM-produced DSL against the JSON schema and the UCM
/// (anti-hallucination: every source reference must exist in the code model).
/// Verified edges not backed by UCM calls are downgraded to Inferred.
pub fn validate(
    viz: &mut Visualization,
    ucm: &UnifiedCodeModel,
    palette: &[ElementType],
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    errors.extend(schema_errors(viz));
    if !errors.is_empty() {
        return Err(errors);
    }

    let files = file_line_counts(ucm);
    let file_lines = file_max_lines(ucm);
    let symbol_by_id: HashMap<&str, &Symbol> = ucm.symbols.iter().map(|s| (s.id.as_str(), s)).collect();
    let symbols: HashSet<&str> = symbol_by_id.keys().copied().collect();
    let calls_by_file = calls_by_file(ucm);
    let palette_types: HashSet<&str> = palette.iter().map(|p| p.kind.as_str()).collect();

    let mut node_ids = HashSet::new();
    collect_node_ids(&viz.nodes, &mut node_ids);
    let node_ids: HashSet<String> = node_ids.iter().map(|s| s.to_string()).collect();
    for node in &mut viz.nodes {
        validate_node(
            node,
            &node_ids,
            &files,
            &file_lines,
            &symbol_by_id,
            &calls_by_file,
            &symbols,
            &palette_types,
            viz.level,
            &mut errors,
        );
    }

    let call_pairs: HashSet<(&str, &str)> = ucm
        .calls
        .iter()
        .map(|c| (c.from.as_str(), c.to.as_str()))
        .collect();
    let symbol_by_node: HashMap<String, String> = collect_symbol_map(&viz.nodes);
    for edge in &mut viz.edges {
        if !node_ids.contains(edge.from.as_str()) {
            errors.push(format!("edge from unknown node '{}'", edge.from));
        }
        if !node_ids.contains(edge.to.as_str()) {
            errors.push(format!("edge to unknown node '{}'", edge.to));
        }
        if edge.status == Some(EdgeStatus::Verified) {
            let backed = match (symbol_by_node.get(&edge.from), symbol_by_node.get(&edge.to)) {
                (Some(from), Some(to)) => call_pairs.contains(&(from, to)),
                _ => false,
            };
            if !backed {
                edge.status = Some(EdgeStatus::Inferred);
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn schema_errors(viz: &Visualization) -> Vec<String> {
    let Ok(schema_value) = serde_json::from_str::<serde_json::Value>(SCHEMA_JSON) else {
        return vec!["internal: broken DSL schema".to_string()];
    };
    let Ok(instance) = serde_json::to_value(viz) else {
        return vec!["internal: cannot serialize visualization".to_string()];
    };
    let Ok(schema) = jsonschema::validator_for(&schema_value) else {
        return vec!["internal: cannot compile DSL schema".to_string()];
    };
    schema
        .iter_errors(&instance)
        .map(|e| format!("schema: {e}"))
        .collect()
}

fn file_line_counts(ucm: &UnifiedCodeModel) -> HashSet<&str> {
    ucm.symbols.iter().map(|s| s.source.file.as_str()).collect()
}

fn file_max_lines(ucm: &UnifiedCodeModel) -> HashMap<&str, u32> {
    let mut out: HashMap<&str, u32> = HashMap::new();
    for symbol in &ucm.symbols {
        let entry = out.entry(symbol.source.file.as_str()).or_default();
        *entry = (*entry).max(symbol.source.end_line);
    }
    out
}

fn calls_by_file(ucm: &UnifiedCodeModel) -> HashMap<&str, Vec<&Call>> {
    let mut out: HashMap<&str, Vec<&Call>> = HashMap::new();
    for call in &ucm.calls {
        out.entry(call.source.file.as_str()).or_default().push(call);
    }
    out
}

fn snap_node_source(
    node: &mut VizNode,
    file_lines: &HashMap<&str, u32>,
    symbol_by_id: &HashMap<&str, &Symbol>,
    calls_by_file: &HashMap<&str, Vec<&Call>>,
) {
    let Some(source) = node.source.as_mut() else { return };
    let max_line = file_lines.get(source.file.as_str()).copied().unwrap_or(1).max(1);

    // Prefer exact symbol position when available.
    if let Some(symbol_id) = &node.symbol {
        if let Some(symbol) = symbol_by_id.get(symbol_id.as_str()) {
            if symbol.source.file == source.file {
                source.start_line = symbol.source.start_line.max(1).min(max_line);
                source.end_line = symbol.source.end_line.max(source.start_line).min(max_line);
                return;
            }
        }
    }

    // For call-like nodes, snap to the nearest UCM call in the same file.
    if matches!(node.kind, NodeKind::Call | NodeKind::Action | NodeKind::Error | NodeKind::Async) {
        if let Some(calls) = calls_by_file.get(source.file.as_str()) {
            let center = (source.start_line + source.end_line) / 2;
            let mut best: Option<&Call> = None;
            let mut best_dist = u32::MAX;
            for call in calls {
                let dist = call.source.start_line.abs_diff(center);
                if dist < best_dist {
                    best_dist = dist;
                    best = Some(call);
                }
            }
            if let Some(call) = best {
                if best_dist <= 5 {
                    source.start_line = call.source.start_line.max(1).min(max_line);
                    source.end_line = call.source.end_line.max(source.start_line).min(max_line);
                }
            }
        }
    }

    // Final clamp and ordering.
    source.start_line = source.start_line.max(1).min(max_line);
    source.end_line = source.end_line.max(1).min(max_line);
    if source.start_line > source.end_line {
        std::mem::swap(&mut source.start_line, &mut source.end_line);
    }
}

fn collect_node_ids<'a>(nodes: &'a [VizNode], out: &mut HashSet<&'a str>) {
    for node in nodes {
        out.insert(node.id.as_str());
        collect_node_ids(&node.children, out);
    }
}

fn collect_symbol_map(nodes: &[VizNode]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    fn walk(nodes: &[VizNode], map: &mut HashMap<String, String>) {
        for node in nodes {
            if let Some(symbol) = &node.symbol {
                map.insert(node.id.clone(), symbol.clone());
            }
            walk(&node.children, map);
        }
    }
    walk(nodes, &mut map);
    map
}

#[allow(clippy::too_many_arguments)]
fn validate_node(
    node: &mut VizNode,
    node_ids: &HashSet<String>,
    files: &HashSet<&str>,
    file_lines: &HashMap<&str, u32>,
    symbol_by_id: &HashMap<&str, &Symbol>,
    calls_by_file: &HashMap<&str, Vec<&Call>>,
    symbols: &HashSet<&str>,
    palette_types: &HashSet<&str>,
    level: Level,
    errors: &mut Vec<String>,
) {
    snap_node_source(node, file_lines, symbol_by_id, calls_by_file);
    if level != Level::Project && node.source.is_none() {
        errors.push(format!("node '{}' missing source ref on scoped level", node.id));
    }
    if let Some(source) = &node.source {
        if source.start_line == 0 || source.start_line > source.end_line {
            errors.push(format!(
                "node '{}' has invalid source range {}-{}",
                node.id, source.start_line, source.end_line
            ));
        }
        if !files.contains(source.file.as_str()) {
            errors.push(format!(
                "node '{}' references unknown file '{}'",
                node.id, source.file
            ));
        }
    }
    if let Some(symbol) = &node.symbol {
        if !symbols.contains(symbol.as_str()) {
            errors.push(format!("node '{}' references unknown symbol '{}'", node.id, symbol));
        }
    }
    if let Some(element_type) = &node.element_type {
        if !palette_types.contains(element_type.as_str()) {
            errors.push(format!(
                "node '{}' uses unknown element_type '{element_type}'",
                node.id
            ));
        }
    }
    for branch in &node.branches {
        if !node_ids.contains(branch.target.as_str()) {
            errors.push(format!(
                "node '{}' branch targets unknown node '{}'",
                node.id, branch.target
            ));
        }
    }
    for child in &mut node.children {
        validate_node(child, node_ids, files, file_lines, symbol_by_id, calls_by_file, symbols, palette_types, level, errors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::{EdgeStatus, Layer, Level, NodeKind, SourceRef, VizEdge, VizNode, Visualization};
    use crate::settings::ElementType;
    use crate::ucm::{SourceRange, Symbol, SymbolKind, UnifiedCodeModel};

    fn palette() -> Vec<ElementType> {
        vec![ElementType {
            kind: "service".to_string(),
            label: "Сервис".to_string(),
            color: "#3b82f6".to_string(),
            icon: "Settings2".to_string(),
            description: String::new(),
        }]
    }

    fn ucm() -> UnifiedCodeModel {
        UnifiedCodeModel {
            language: "go".to_string(),
            packages: vec![],
            symbols: vec![
                Symbol {
                    id: "app/main.Run".to_string(),
                    kind: SymbolKind::Function,
                    name: "Run".to_string(),
                    package: "app/main".to_string(),
                    source: SourceRange { file: "main.go".to_string(), start_line: 10, end_line: 20 },
                    signature: "func()".to_string(),
                    is_exported: true,
                    is_async: false,
                },
                Symbol {
                    id: "app/db.Save".to_string(),
                    kind: SymbolKind::Function,
                    name: "Save".to_string(),
                    package: "app/db".to_string(),
                    source: SourceRange { file: "db/save.go".to_string(), start_line: 5, end_line: 12 },
                    signature: "func()".to_string(),
                    is_exported: true,
                    is_async: false,
                },
            ],
            calls: vec![],
            effects: vec![],
            entities: vec![],
        }
    }

    fn node(id: &str) -> VizNode {
        VizNode {
            id: id.to_string(),
            kind: NodeKind::Group,
            label: id.to_string(),
            layer: Layer::Calls,
            source: None,
            element_type: None,
            symbol: None,
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
    }

    fn viz(nodes: Vec<VizNode>, edges: Vec<VizEdge>) -> Visualization {
        Visualization {
            title: "test".to_string(),
            level: Level::Project,
            nodes,
            edges,
        }
    }

    fn edge(from: &str, to: &str, status: EdgeStatus) -> VizEdge {
        VizEdge {
            from: from.to_string(),
            to: to.to_string(),
            label: None,
            status: Some(status),
        }
    }

    #[test]
    fn accepts_valid_visualization() {
        let mut v = viz(vec![node("a"), node("b")], vec![edge("a", "b", EdgeStatus::Inferred)]);
        assert!(validate(&mut v, &ucm(), &palette()).is_ok());
    }

    #[test]
    fn rejects_unknown_edge_target() {
        let mut v = viz(vec![node("a")], vec![edge("a", "ghost", EdgeStatus::Inferred)]);
        let err = validate(&mut v, &ucm(), &palette()).unwrap_err();
        assert!(err.iter().any(|e| e.contains("ghost")));
    }

    #[test]
    fn rejects_unknown_source_file() {
        let mut n = node("a");
        n.source = Some(SourceRef { file: "nope.go".to_string(), start_line: 1, end_line: 2 });
        let mut v = viz(vec![n], vec![]);
        let err = validate(&mut v, &ucm(), &palette()).unwrap_err();
        assert!(err.iter().any(|e| e.contains("unknown file")));
    }

    #[test]
    fn rejects_unknown_symbol() {
        let mut n = node("a");
        n.symbol = Some("fake.Symbol".to_string());
        let mut v = viz(vec![n], vec![]);
        let err = validate(&mut v, &ucm(), &palette()).unwrap_err();
        assert!(err.iter().any(|e| e.contains("unknown symbol")));
    }

    #[test]
    fn rejects_unknown_element_type() {
        let mut n = node("a");
        n.element_type = Some("spaceship".to_string());
        let mut v = viz(vec![n], vec![]);
        let err = validate(&mut v, &ucm(), &palette()).unwrap_err();
        assert!(err.iter().any(|e| e.contains("spaceship")));
    }

    #[test]
    fn requires_source_on_function_level() {
        let mut v = viz(vec![node("a")], vec![]);
        v.level = Level::Function;
        let err = validate(&mut v, &ucm(), &palette()).unwrap_err();
        assert!(err.iter().any(|e| e.contains("missing source")));
    }

    #[test]
    fn downgrades_unbacked_verified_edge() {
        let mut a = node("a");
        a.symbol = Some("app/main.Run".to_string());
        let mut b = node("b");
        b.symbol = Some("app/db.Save".to_string());
        let mut v = viz(vec![a, b], vec![edge("a", "b", EdgeStatus::Verified)]);
        validate(&mut v, &ucm(), &palette()).unwrap();
        assert_eq!(v.edges[0].status, Some(EdgeStatus::Inferred));
    }

    #[test]
    fn keeps_backed_verified_edge() {
        let mut model = ucm();
        model.calls.push(crate::ucm::Call {
            from: "app/main.Run".to_string(),
            to: "app/db.Save".to_string(),
            source: SourceRange { file: "main.go".to_string(), start_line: 15, end_line: 15 },
        });
        let mut a = node("a");
        a.symbol = Some("app/main.Run".to_string());
        let mut b = node("b");
        b.symbol = Some("app/db.Save".to_string());
        let mut v = viz(vec![a, b], vec![edge("a", "b", EdgeStatus::Verified)]);
        validate(&mut v, &model, &palette()).unwrap();
        assert_eq!(v.edges[0].status, Some(EdgeStatus::Verified));
    }

    #[test]
    fn rejects_schema_violation() {
        let mut bad = node("a");
        bad.label = String::new();
        let mut v = viz(vec![bad], vec![]);
        let err = validate(&mut v, &ucm(), &palette()).unwrap_err();
        assert!(err.iter().any(|e| e.starts_with("schema:")));
    }
}
