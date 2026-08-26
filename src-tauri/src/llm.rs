use crate::settings::{ElementType, Endpoint};
use anyhow::{bail, Context, Result};
use reqwest::Client;
use serde_json::{json, Value};

pub async fn check_connection(endpoint: &Endpoint) -> Result<Vec<String>> {
    let client = Client::new();
    let url = format!("{}/models", endpoint.base_url.trim_end_matches('/'));
    let mut req = client.get(&url);
    if !endpoint.api_key.is_empty() {
        req = req.bearer_auth(&endpoint.api_key);
    }
    let resp = req.send().await.context("endpoint unreachable")?;
    let body: Value = resp.json().await.context("invalid JSON from /models")?;
    let models = body["data"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    Ok(models)
}

/// A stateful chat session: keeps message history so validation errors can be
/// fed back to the model on retry.
pub struct Chat<'a> {
    endpoint: &'a Endpoint,
    client: Client,
    messages: Vec<Value>,
    max_tokens: u32,
}

impl<'a> Chat<'a> {
    pub fn new(endpoint: &'a Endpoint, system: String, user: String, max_tokens: u32) -> Self {
        Self {
            endpoint,
            client: Client::new(),
            max_tokens,
            messages: vec![
                json!({"role": "system", "content": system}),
                json!({"role": "user", "content": user}),
            ],
        }
    }

    pub fn push_user(&mut self, content: String) {
        self.messages.push(json!({"role": "user", "content": content}));
    }

    pub async fn complete(&mut self) -> Result<String> {
        let url = format!("{}/chat/completions", self.endpoint.base_url.trim_end_matches('/'));
        let body = json!({
            "model": self.endpoint.model,
            "messages": self.messages,
            "temperature": 0.2,
            "max_tokens": self.max_tokens,
            "response_format": {"type": "json_object"},
        });
        let mut req = self.client.post(&url).json(&body);
        if !self.endpoint.api_key.is_empty() {
            req = req.bearer_auth(&self.endpoint.api_key);
        }
        let resp = req.send().await.context("LLM request failed")?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if text.contains("exceeds the available context size") {
                bail!("LLM context is too large for this model; Graphloom sent a compact analysis context, but the selected project still exceeds the endpoint limit. Choose a larger-context model or analyze a smaller scope.");
            }
            bail!("LLM endpoint error {status}: {text}");
        }
        let payload: Value = resp.json().await.context("invalid JSON from LLM endpoint")?;
        let content = payload["choices"][0]["message"]["content"]
            .as_str()
            .context("no content in LLM response")?
            .to_string();
        self.messages.push(json!({"role": "assistant", "content": content}));
        Ok(content)
    }
}

pub fn palette_description(palette: &[ElementType]) -> String {
    let mut out = String::new();
    for t in palette {
        out.push_str(&format!(
            "- \"{}\": {} — {}\n",
            t.kind, t.label, t.description
        ));
    }
    out
}

const PROJECT_MAP_TEMPLATE: &str = include_str!("../prompts/project_map.md");
const FILE_MAP_TEMPLATE: &str = include_str!("../prompts/file_map.md");
const FUNCTION_FLOW_TEMPLATE: &str = include_str!("../prompts/function_flow.md");
const SCHEMA_JSON: &str = include_str!("dsl.schema.json");

const ENTITY_LABELS_TEMPLATE: &str = include_str!("../prompts/entity_labels.md");

fn render_template(template: &str, palette: &[ElementType]) -> String {
    template
        .replace("{{PALETTE}}", &palette_description(palette))
        .replace("{{SCHEMA}}", SCHEMA_JSON)
}

pub fn project_map_prompt(palette: &[ElementType]) -> String {
    render_template(PROJECT_MAP_TEMPLATE, palette)
}

pub fn file_map_prompt(palette: &[ElementType]) -> String {
    render_template(FILE_MAP_TEMPLATE, palette)
}

pub fn function_flow_prompt(palette: &[ElementType]) -> String {
    render_template(FUNCTION_FLOW_TEMPLATE, palette)
}

pub fn entity_labels_prompt(_palette: &[ElementType], tree_json: &str) -> String {
    ENTITY_LABELS_TEMPLATE.replace("__TREE__", tree_json)
}
