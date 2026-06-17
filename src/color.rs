use std::collections::HashMap;

use ratatui::style::{Color, Style};
use serde::Deserialize;

/// 颜色模式（自动检测或用户指定）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Mono,
    Color16,
    Color256,
    Truecolor,
}

/// Shading 模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadingMode {
    Random,
    DistanceFromHead,
}

/// Bold 模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoldMode {
    Off,
    Random,
    All,
}

/// 颜色主题枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTheme {
    #[allow(dead_code)]
    User,
    Green,
    Green2,
    Green3,
    Yellow,
    Orange,
    Red,
    Blue,
    Cyan,
    Gold,
    Rainbow,
    Purple,
    Pink,
    Pink2,
    Vaporwave,
    Gray,
}

/// 主题颜色表：pairs[1] 最暗（尾部） → pairs[7] 最亮（头部）
pub struct ThemeColors {
    pub pairs: [Color; 8],
    pub num_pairs: usize,
}

impl ThemeColors {
    fn from_indexed(indexed: &[u8]) -> Self {
        let mut colors = [Color::Reset; 8];
        let n = indexed.len().min(7);
        for i in 0..n {
            colors[i + 1] = Color::Indexed(indexed[i]);
        }
        ThemeColors { pairs: colors, num_pairs: n }
    }

    fn from_rgb(rgbs: &[[u8; 3]]) -> Self {
        let mut colors = [Color::Reset; 8];
        let n = rgbs.len().min(7);
        for i in 0..n {
            colors[i + 1] = Color::Rgb(rgbs[i][0], rgbs[i][1], rgbs[i][2]);
        }
        ThemeColors { pairs: colors, num_pairs: n }
    }

    pub fn style(&self, pair: usize, is_bold: bool) -> Style {
        let pair = pair.clamp(1, self.num_pairs);
        Style::default()
            .fg(self.pairs[pair])
            .add_modifier(if is_bold {
                ratatui::style::Modifier::BOLD
            } else {
                ratatui::style::Modifier::empty()
            })
    }

    pub fn message_style(&self) -> Style {
        Style::default()
            .fg(self.pairs[self.num_pairs])
            .add_modifier(ratatui::style::Modifier::BOLD)
    }
}

// ---- TOML 数据结构 ----

#[derive(Deserialize)]
struct ThemesFile {
    #[serde(flatten)]
    themes: HashMap<String, ThemeDef>,
}

#[derive(Deserialize)]
struct ThemeDef {
    indexed: Option<Vec<u8>>,
    rgb: Vec<[u8; 3]>,
}

const THEMES_TOML: &str = include_str!("colors.toml");

fn load_theme_data() -> HashMap<String, ThemeDef> {
    toml::from_str::<ThemesFile>(THEMES_TOML)
        .expect("colors.toml parse failure")
        .themes
}

thread_local! {
    static THEME_DATA: HashMap<String, ThemeDef> = load_theme_data();
}

fn theme_name(theme: ColorTheme) -> &'static str {
    match theme {
        ColorTheme::Green => "green",
        ColorTheme::Green2 => "green2",
        ColorTheme::Green3 => "green3",
        ColorTheme::Yellow => "yellow",
        ColorTheme::Orange => "orange",
        ColorTheme::Red => "red",
        ColorTheme::Blue => "blue",
        ColorTheme::Cyan => "cyan",
        ColorTheme::Gold => "gold",
        ColorTheme::Rainbow => "rainbow",
        ColorTheme::Purple => "purple",
        ColorTheme::Pink => "pink",
        ColorTheme::Pink2 => "pink2",
        ColorTheme::Vaporwave => "vaporwave",
        ColorTheme::Gray => "gray",
        ColorTheme::User => "green", // fallback
    }
}

pub fn get_theme_colors(theme: ColorTheme, mode: ColorMode) -> ThemeColors {
    THEME_DATA.with(|data| {
        let name = theme_name(theme);
        let def = data.get(name).unwrap_or_else(|| &data["green"]);
        match mode {
            ColorMode::Color16 => {
                if let Some(ref indexed) = def.indexed {
                    ThemeColors::from_indexed(indexed)
                } else {
                    ThemeColors::from_rgb(&def.rgb)
                }
            }
            _ => ThemeColors::from_rgb(&def.rgb),
        }
    })
}

impl ColorTheme {
    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "green" => ColorTheme::Green,
            "green2" => ColorTheme::Green2,
            "green3" => ColorTheme::Green3,
            "yellow" => ColorTheme::Yellow,
            "orange" => ColorTheme::Orange,
            "red" => ColorTheme::Red,
            "blue" => ColorTheme::Blue,
            "cyan" => ColorTheme::Cyan,
            "gold" => ColorTheme::Gold,
            "rainbow" => ColorTheme::Rainbow,
            "purple" => ColorTheme::Purple,
            "pink" => ColorTheme::Pink,
            "pink2" => ColorTheme::Pink2,
            "vaporwave" => ColorTheme::Vaporwave,
            "gray" => ColorTheme::Gray,
            _ => ColorTheme::Green,
        }
    }
}

/// 根据终端能力检测最佳颜色模式
pub fn detect_color_mode() -> ColorMode {
    if let Ok(ct) = std::env::var("COLORTERM") {
        if ct == "truecolor" || ct == "24bit" {
            return ColorMode::Truecolor;
        }
    }
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("256color") {
            return ColorMode::Color256;
        }
    }
    ColorMode::Truecolor
}
