use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::params::SimParams;

/// Application configuration loaded from TOML
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub render: RenderConfig,
    #[serde(default)]
    pub charset: CharsetConfig,
    #[serde(default)]
    pub rain: RainConfig,
    #[serde(default)]
    pub glitch: GlitchConfig,
    #[serde(default)]
    pub linger: LingerConfig,
    #[serde(default)]
    pub exit: ExitConfig,
    #[serde(default)]
    pub claude: ClaudeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    #[serde(default = "default_fps")]
    pub fps: f64,
    #[serde(default = "default_color")]
    pub color: String,
    #[serde(default)]
    pub shading_mode: u8,
    #[serde(default)]
    pub bold_mode: u8,
    #[serde(default)]
    pub full_width: bool,
    #[serde(default = "default_true")]
    pub default_bg: bool,
    #[serde(default)]
    pub show_status: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        RenderConfig {
            fps: 60.0,
            color: "green".into(),
            shading_mode: 0,
            bold_mode: 1,
            full_width: false,
            default_bg: true,
            show_status: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharsetConfig {
    #[serde(default = "default_charset_source")]
    pub source: String,  // "katakana", "file:/path/to/chars.txt", "stdin"
}

impl Default for CharsetConfig {
    fn default() -> Self {
        CharsetConfig {
            source: "katakana".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RainConfig {
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default = "default_density")]
    pub density: f32,
    #[serde(default)]
    pub async_scroll: bool,
    #[serde(default = "default_max_dpc")]
    pub max_droplets_per_col: u8,
    #[serde(default = "default_short_pct")]
    pub short_pct: f32,
    #[serde(default = "default_die_early_pct")]
    pub die_early_pct: f32,
}

impl Default for RainConfig {
    fn default() -> Self {
        RainConfig {
            speed: 8.0,
            density: 1.0,
            async_scroll: false,
            max_droplets_per_col: 3,
            short_pct: 50.0,
            die_early_pct: 33.3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlitchConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_glitch_pct")]
    pub pct: f32,
    #[serde(default = "default_glitch_low_ms")]
    pub low_ms: u16,
    #[serde(default = "default_glitch_high_ms")]
    pub high_ms: u16,
}

impl Default for GlitchConfig {
    fn default() -> Self {
        GlitchConfig {
            enabled: true,
            pct: 10.0,
            low_ms: 300,
            high_ms: 400,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LingerConfig {
    #[serde(default = "default_linger_low_ms")]
    pub low_ms: u16,
    #[serde(default = "default_linger_high_ms")]
    pub high_ms: u16,
}

impl Default for LingerConfig {
    fn default() -> Self {
        LingerConfig {
            low_ms: 1,
            high_ms: 3000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitConfig {
    #[serde(default = "default_exit_mode")]
    pub mode: String, // "normal" | "on-key" | "after-secs"
    #[serde(default)]
    pub secs: f64,
}

impl Default for ExitConfig {
    fn default() -> Self {
        ExitConfig {
            mode: "on-key".into(),
            secs: 10.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeConfig {
    #[serde(default)]
    pub enabled: bool,
    /// transcript 目录路径。留空则根据 CWD 自动推导。
    #[serde(default)]
    pub transcript_dir: String,
    #[serde(default = "default_max_chars")]
    pub max_chars: usize,
}

impl Default for ClaudeConfig {
    fn default() -> Self {
        ClaudeConfig {
            enabled: false,
            transcript_dir: String::new(),
            max_chars: 10000,
        }
    }
}

// Default value helpers
fn default_fps() -> f64 { 60.0 }
fn default_color() -> String { "green".into() }
fn default_charset_source() -> String { "katakana".into() }
fn default_speed() -> f32 { 8.0 }
fn default_density() -> f32 { 1.0 }
fn default_max_dpc() -> u8 { 3 }
fn default_short_pct() -> f32 { 50.0 }
fn default_die_early_pct() -> f32 { 33.3 }
fn default_true() -> bool { true }
fn default_glitch_pct() -> f32 { 10.0 }
fn default_glitch_low_ms() -> u16 { 300 }
fn default_glitch_high_ms() -> u16 { 400 }
fn default_linger_low_ms() -> u16 { 1 }
fn default_linger_high_ms() -> u16 { 3000 }
fn default_exit_mode() -> String { "normal".into() }
fn default_max_chars() -> usize { 10000 }

/// Load config from standard XDG paths plus project-level overrides.
pub fn load_config(explicit_path: Option<&PathBuf>) -> Config {
    // If explicitly specified, use that
    if let Some(path) = explicit_path {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(cfg) = toml::from_str(&content) {
                return cfg;
            }
        }
    }

    // Try project-level .neo-rainst.toml first (highest non-explicit priority)
    let project_config = PathBuf::from(".neo-rainst.toml");
    if project_config.exists() {
        if let Ok(content) = std::fs::read_to_string(&project_config) {
            if let Ok(cfg) = toml::from_str(&content) {
                return cfg;
            }
        }
    }

    // Try XDG config
    let xdg_path = xdg_config_path();
    if let Some(path) = xdg_path {
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str(&content) {
                    return cfg;
                }
            }
        }
    }

    Config::default()
}

fn xdg_config_path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("neo-rainst").join("config.toml"))
}

impl From<&Config> for SimParams {
    fn from(cfg: &Config) -> Self {
        SimParams {
            charset: if cfg.charset.source.is_empty() { None } else { Some(cfg.charset.source.clone()) },
            speed: Some(cfg.rain.speed),
            density: Some(cfg.rain.density),
            fps: Some(cfg.render.fps),
            color: if cfg.render.color.is_empty() { None } else { Some(cfg.render.color.clone()) },
            show_status: Some(cfg.render.show_status),
            full_width: Some(cfg.render.full_width),
            default_bg: Some(cfg.render.default_bg),
            shading_mode: Some(cfg.render.shading_mode),
            bold: Some(cfg.render.bold_mode),
            async_scroll: Some(cfg.rain.async_scroll),
            maxdpc: Some(cfg.rain.max_droplets_per_col),
            short_pct: Some(cfg.rain.short_pct),
            rip_pct: Some(cfg.rain.die_early_pct),
            glitch_ms_low: Some(cfg.glitch.low_ms),
            glitch_ms_high: Some(cfg.glitch.high_ms),
            glitch_pct: Some(cfg.glitch.pct),
            no_glitch: Some(!cfg.glitch.enabled),
            linger_ms_low: Some(cfg.linger.low_ms),
            linger_ms_high: Some(cfg.linger.high_ms),
            screensaver: Some(false),
            exit_on_key: Some(cfg.exit.mode == "on-key"),
            exit_after_secs: if cfg.exit.mode == "after-secs" && cfg.exit.secs > 0.0 { Some(cfg.exit.secs) } else { None },
            ..Default::default()
        }
    }
}
