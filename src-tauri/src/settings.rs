use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementType {
    #[serde(rename = "type")]
    pub kind: String,
    pub label: String,
    pub color: String,
    pub icon: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub endpoint: Endpoint,
    pub palette: Vec<ElementType>,
}

impl Default for Endpoint {
    fn default() -> Self {
        // .env next to the app (or any parent dir) provides the defaults;
        // settings.json saved from the UI overrides them.
        let manifest_env = concat!(env!("CARGO_MANIFEST_DIR"), "/../.env");
        let _ = dotenvy::from_path(manifest_env);
        let _ = dotenvy::dotenv();
        Self {
            base_url: std::env::var("GRAPHLOOM_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080/v1".to_string()),
            model: std::env::var("GRAPHLOOM_MODEL").unwrap_or_default(),
            api_key: std::env::var("GRAPHLOOM_API_KEY").unwrap_or_default(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            endpoint: Endpoint::default(),
            palette: default_palette(),
        }
    }
}

fn default_palette() -> Vec<ElementType> {
    let entries = [
        (
            "service",
            "Сервис",
            "#3b82f6",
            "Settings2",
            "Самостоятельный сервис или приложение",
        ),
        (
            "database",
            "База данных",
            "#10b981",
            "Database",
            "Хранилище данных, слой доступа к БД",
        ),
        (
            "module",
            "Модуль",
            "#a78bfa",
            "Box",
            "Внутренний модуль или пакет",
        ),
        (
            "library",
            "Библиотека",
            "#f59e0b",
            "Library",
            "Переиспользуемая библиотека",
        ),
        (
            "entrypoint",
            "Точка входа",
            "#ef4444",
            "Rocket",
            "main / CLI / server entrypoint",
        ),
    ];
    entries
        .iter()
        .map(|(kind, label, color, icon, description)| ElementType {
            kind: kind.to_string(),
            label: label.to_string(),
            color: color.to_string(),
            icon: icon.to_string(),
            description: description.to_string(),
        })
        .collect()
}

fn settings_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("config dir not found")?;
    Ok(dir.join("graphloom").join("settings.json"))
}

pub fn load() -> Settings {
    let Ok(path) = settings_path() else {
        return Settings::default();
    };
    let Ok(raw) = fs::read_to_string(path) else {
        return Settings::default();
    };
    let Ok(mut settings) = serde_json::from_str::<Settings>(&raw) else {
        return Settings::default();
    };
    let mut migrated = false;
    for item in &mut settings.palette {
        if let Some(icon) = migrate_icon(&item.icon) {
            item.icon = icon.to_string();
            migrated = true;
        }
    }
    if migrated {
        let _ = save(&settings);
    }
    settings
}

fn migrate_icon(icon: &str) -> Option<&'static str> {
    match icon {
        "⚙️" => Some("Settings2"),
        "🗄️" => Some("Database"),
        "📦" => Some("Box"),
        "📚" => Some("Library"),
        "🚀" => Some("Rocket"),
        _ => None,
    }
}

pub fn save(settings: &Settings) -> Result<()> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_string_pretty(settings)?)?;
    Ok(())
}
