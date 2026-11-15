use rune::{Color, oklch};
use serde::{Deserialize, Deserializer};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum IconKind {
    Mic,
    Camera,
    Record,
    Layers,
    Scissors,
    Play,
    Sun,
    Terminal,
    Lock,
    Folder,
    Back,
    ArrowUp,
    ArrowDown,
    Volume,
    #[default]
    Circle,
    /// A local SVG file path or an Iconify shortcut (e.g. `solar:accessibility-bold`).
    Custom(String),
}

impl IconKind {
    /// Returns the custom icon specifier (local SVG path or Iconify shortcut) if this is a custom icon.
    pub fn custom_spec(&self) -> Option<&str> {
        match self {
            Self::Custom(spec) => Some(spec.as_str()),
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for IconKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return Ok(Self::Circle);
        }
        Ok(match trimmed.to_ascii_lowercase().as_str() {
            "mic" => Self::Mic,
            "camera" => Self::Camera,
            "record" => Self::Record,
            "layers" => Self::Layers,
            "scissors" => Self::Scissors,
            "play" => Self::Play,
            "sun" => Self::Sun,
            "terminal" => Self::Terminal,
            "lock" => Self::Lock,
            "folder" => Self::Folder,
            "back" => Self::Back,
            "arrow_up" => Self::ArrowUp,
            "arrow_down" => Self::ArrowDown,
            "volume" => Self::Volume,
            "circle" => Self::Circle,
            _ => Self::Custom(trimmed.to_string()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeyAction {
    #[default]
    Toggle,
    Trigger,
    OpenDeck {
        deck: String,
    },
    Back,
    LayerUp,
    LayerDown,
    Command {
        cmd: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Plugin {
        plugin: String,
        command: String,
        #[serde(default)]
        params: serde_json::Value,
    },
    None,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct KeyConfig {
    pub label: String,
    #[serde(default)]
    pub icon: IconKind,
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default)]
    pub toggle: bool,
    #[serde(default)]
    pub close: Option<bool>,
    #[serde(default)]
    pub action: Option<KeyAction>,
}

fn default_accent() -> String {
    "cyan".to_string()
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct LayerConfig {
    #[serde(default = "default_layer_name")]
    pub name: String,
    #[serde(default)]
    pub keys: Vec<KeyConfig>,
}

fn default_layer_name() -> String {
    "Base".to_string()
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct DeckConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub layers: Vec<LayerConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct PluginConfig {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AppConfig {
    #[serde(default = "default_reset_grace_ms")]
    pub reset_grace_ms: u64,
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
    #[serde(default)]
    pub decks: Vec<DeckConfig>,
    #[serde(skip)]
    pub raw_json: serde_json::Value,
}

fn default_reset_grace_ms() -> u64 {
    2000
}

pub fn parse_accent(name: &str) -> Color {
    match name.trim().to_ascii_lowercase().as_str() {
        "red" | "crimson" => oklch(0.66, 0.21, 25.0),
        "blue" => oklch(0.68, 0.18, 240.0),
        "purple" | "violet" => oklch(0.70, 0.17, 285.0),
        "green" => oklch(0.72, 0.16, 155.0),
        "cyan" => oklch(0.72, 0.16, 200.0),
        "amber" | "yellow" => oklch(0.78, 0.16, 80.0),
        "teal" => oklch(0.72, 0.15, 175.0),
        "orange" => oklch(0.72, 0.16, 50.0),
        "pink" => oklch(0.70, 0.19, 340.0),
        _ => oklch(0.72, 0.16, 200.0),
    }
}

pub fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
}

pub fn expand_tilde(raw: &str) -> PathBuf {
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
        && let Some(home) = user_home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(raw)
}

/// Returns `~/.config/vdeck/config.toml` (or `$XDG_CONFIG_HOME/vdeck/config.toml`).
pub fn config_file_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| user_home_dir().map(|h| h.join(".config")))?;
    Some(base.join("vdeck").join("config.toml"))
}

/// Returns `~/.local/share/vdeck/plugins` (or `$XDG_DATA_HOME/vdeck/plugins`).
pub fn plugins_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| user_home_dir().map(|h| h.join(".local").join("share")))?;
    Some(base.join("vdeck").join("plugins"))
}

pub fn load_config() -> AppConfig {
    let candidates = [
        config_file_path().unwrap_or_default(),
        PathBuf::from("vdeck.toml"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("vdeck.toml")))
            .unwrap_or_default(),
    ];

    for path in &candidates {
        if path.as_os_str().is_empty() || !path.exists() {
            continue;
        }
        if let Ok(cfg) = load_config_from_path(path) {
            return cfg;
        }
    }

    default_config()
}

pub fn load_config_from_path(path: &Path) -> Result<AppConfig, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    parse_config(&raw)
}

pub fn parse_config(raw: &str) -> Result<AppConfig, String> {
    let mut cfg: AppConfig = toml::from_str(raw).map_err(|e| format!("toml parse: {e}"))?;
    if cfg.decks.is_empty() {
        return Err("vdeck.toml must define at least one [[decks]]".to_string());
    }
    if cfg.decks.iter().any(|d| d.layers.is_empty()) {
        return Err("each [[decks]] must define at least one [[decks.layers]]".to_string());
    }
    cfg.raw_json = toml::from_str::<toml::Value>(raw)
        .ok()
        .and_then(|v| serde_json::to_value(v).ok())
        .unwrap_or(serde_json::Value::Null);
    Ok(cfg)
}

pub fn default_config() -> AppConfig {
    parse_config(DEFAULT_TOML).expect("embedded default vdeck.toml must be valid")
}

pub const DEFAULT_TOML: &str = include_str!("../vdeck.toml");
