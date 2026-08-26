use crate::dsl::Visualization;
use crate::settings::ElementType;
use anyhow::Result;
use std::collections::BTreeMap;

/// Escapes `</` sequences so embedded JSON cannot break out of the script tag.
fn embed_json<T: serde::Serialize + ?Sized>(value: &T) -> Result<String> {
    Ok(serde_json::to_string(value)?.replace("</", "<\\/"))
}

/// Renders a validated visualization into a self-contained deterministic HTML file.
/// `sources` maps relative file paths to their full contents for the source panel.
pub fn render_html(
    viz: &Visualization,
    palette: &[ElementType],
    sources: &BTreeMap<String, String>,
) -> Result<String> {
    let html = TEMPLATE
        .replace("__DATA__", &embed_json(viz)?)
        .replace("__PALETTE__", &embed_json(palette)?)
        .replace("__SOURCES__", &embed_json(sources)?);
    Ok(html)
}

const TEMPLATE: &str = include_str!("render.html");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::{Layer, Level, NodeKind, Visualization, VizEdge, VizNode};

    fn sample_viz() -> Visualization {
        Visualization {
            title: "Sample".to_string(),
            level: Level::Project,
            nodes: vec![VizNode {
                id: "a".to_string(),
                kind: NodeKind::Group,
                label: "Node A".to_string(),
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
            }],
            edges: vec![VizEdge {
                from: "a".to_string(),
                to: "a".to_string(),
                label: None,
                status: None,
            }],
        }
    }

    #[test]
    fn render_is_deterministic() {
        let viz = sample_viz();
        let sources = BTreeMap::new();
        let first = render_html(&viz, &[], &sources).unwrap();
        let second = render_html(&viz, &[], &sources).unwrap();
        assert_eq!(first, second);
        assert!(first.contains("#layers { display: flex"));
        assert!(first.contains(".elabel-bg"));
        assert!(first.contains("treeLayout") || first.contains("layeredLayout"));
    }

    #[test]
    fn render_embeds_data_without_script_breakout() {
        let mut viz = sample_viz();
        viz.nodes[0].label = "x</script><script>alert(1)</script>".to_string();
        let html = render_html(&viz, &[], &BTreeMap::new()).unwrap();
        assert!(!html.contains("x</script><script>alert"));
        assert!(html.contains("alert"));
    }
}
