use ratatui::style::{Color, Style};

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

/// 颜色主题枚举 — 对应原版 neo 全部 16 种
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
    pub pairs: [Color; 8],  // Index 0 unused
    pub num_pairs: usize,   // Actual number (2-15 depending on theme)
}

impl ThemeColors {
    /// Create from indexed 256-color values (same as ncurses color pair indices)
    fn from_indexed(pairs: &[u8]) -> Self {
        let mut colors = [Color::Reset; 8];
        let n = pairs.len().min(7);
        for i in 0..n {
            colors[i + 1] = Color::Indexed(pairs[i]);
        }
        ThemeColors {
            pairs: colors,
            num_pairs: n,
        }
    }

    /// Create from RGB values (for truecolor mode)
    fn from_rgb(rgbs: &[(u8, u8, u8)]) -> Self {
        let mut colors = [Color::Reset; 8];
        let n = rgbs.len().min(7);
        for i in 0..n {
            colors[i + 1] = Color::Rgb(rgbs[i].0, rgbs[i].1, rgbs[i].2);
        }
        ThemeColors {
            pairs: colors,
            num_pairs: n,
        }
    }

    /// Get ratatui Style for a given color pair index (1-based) and bold flag
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

    /// Get the style for the message display (brightest pair)
    pub fn message_style(&self) -> Style {
        Style::default()
            .fg(self.pairs[self.num_pairs])
            .add_modifier(ratatui::style::Modifier::BOLD)
    }
}

/// Generate theme colors for a given ColorTheme and ColorMode.
///
/// Color values are from the original neo source (`cloud.cpp` `SetColor`).
/// Truecolor uses init_color RGB (0-1000 scaled to 0-255). 256-color uses indexed palette.
pub fn get_theme_colors(theme: ColorTheme, mode: ColorMode) -> ThemeColors {
    match (theme, mode) {
        (ColorTheme::Green, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[10, 15])
        }
        (ColorTheme::Green, _) => {
            // RGB from original neo: (71,141,83)→(149,243,161)→(188,596,318)... scaled /1000*255
            ThemeColors::from_rgb(&[
                (18, 36, 21),       //  234: (71,141,83)  → (18,36,21)
                (38, 62, 41),       //   22: (149,243,161) → (38,62,41)
                (48, 152, 81),      //   28: (188,596,318) → (48,152,81)
                (48, 182, 101),     //   35: (188,714,397) → (48,182,101)
                (58, 236, 143),     //   78: (227,925,561) → (58,236,143)
                (69, 248, 170),     //   84: (271,973,667) → (69,248,170)
                (170, 255, 240),    //  159: (667,1000,941) → (170,255,240)
            ])
        }

        (ColorTheme::Rainbow, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[9, 1, 11, 10, 12, 13])
        }
        (ColorTheme::Rainbow, _) => {
            ThemeColors::from_rgb(&[
                (255, 0, 0),     // 196
                (255, 135, 0),   // 208
                (255, 255, 0),   // 226
                (0, 255, 0),     // 46
                (0, 0, 255),     // 21
                (135, 0, 255),   // 93
                (255, 0, 255),   // 201
            ])
        }

        (ColorTheme::Vaporwave, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[5, 13, 11, 14, 15])
        }
        (ColorTheme::Vaporwave, _) => {
            ThemeColors::from_rgb(&[
                (95, 0, 95),      // 53 dark purple
                (95, 0, 135),     // 54
                (95, 0, 175),     // 55
                (175, 0, 215),    // 134
                (215, 0, 255),    // 177
                (255, 175, 255),  // 219
                (255, 215, 95),   // 229 orange/yellow
            ])
        }

        (ColorTheme::Blue, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[4, 12, 15])
        }
        (ColorTheme::Blue, _) => {
            ThemeColors::from_rgb(&[
                (18, 36, 21),     // 234
                (0, 0, 95),       // 17
                (0, 0, 135),      // 18
                (0, 0, 175),      // 19
                (0, 0, 215),      // 20
                (0, 0, 255),      // 21
                (95, 95, 255),    // 75 → head
            ])
        }

        (ColorTheme::Gold, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[8, 3, 11, 15])
        }
        (ColorTheme::Gold, _) => {
            ThemeColors::from_rgb(&[
                (214, 139, 55),   // 172
                (211, 137, 53),   // 178
                (231, 212, 144),  // 225 (approximation)
                (255, 235, 144),  // 228
                (255, 235, 203),  // 229
                (255, 243, 203),  // 230
                (255, 255, 255),  // 231
            ])
        }

        (ColorTheme::Red, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[1, 9, 15])
        }
        (ColorTheme::Red, _) => {
            ThemeColors::from_rgb(&[
                (175, 0, 0),      // 124
                (215, 0, 0),      // 160
                (255, 0, 0),      // 196
                (255, 95, 95),    // 217 head
            ])
        }

        (ColorTheme::Cyan, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[6, 14, 15])
        }
        (ColorTheme::Cyan, _) => {
            ThemeColors::from_rgb(&[
                (0, 135, 135),    // 31
                (0, 175, 175),    // 32
                (0, 215, 215),    // 38
                (0, 255, 255),    // 45
                (170, 255, 240),  // 159 head
            ])
        }

        (ColorTheme::Orange, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[1, 7])
        }
        (ColorTheme::Orange, _) => {
            ThemeColors::from_rgb(&[
                (135, 78, 26),    // 94
                (175, 95, 0),     // 130
                (215, 135, 0),    // 166
                (255, 175, 0),    // 202
                (255, 135, 0),    // 208
                (255, 255, 255),  // 231
            ])
        }

        (ColorTheme::Gray, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[8, 7, 15])
        }
        (ColorTheme::Gray, _) => {
            ThemeColors::from_rgb(&[
                (95, 95, 95),     // 240
                (135, 135, 135),  // 243
                (175, 175, 175),  // 246
                (215, 215, 215),  // 249
                (235, 235, 235),  // 251
                (245, 245, 245),  // 252
                (255, 255, 255),  // 231
            ])
        }

        // Green2, Green3, Yellow, Purple, Pink, Pink2 — from neo source
        (ColorTheme::Green2, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[8, 2, 10, 15])
        }
        (ColorTheme::Green2, _) => {
            ThemeColors::from_rgb(&[
                (15, 46, 15),     //  28: (16,180,59) → (4,46,15)
                (15, 63, 30),     //  34: (59,246,117) → (15,63,30)
                (12, 131, 44),    //  76: (46,512,172) → (12,131,44)
                (67, 191, 85),    //  84: (262,749,332) → (67,191,85)
                (133, 241, 147),  // 120: (520,945,578) → (133,241,147)
                (172, 247, 193),  // 157: (676,969,758) → (172,247,193)
                (231, 255, 229),  // 231: (906,1000,898) → (231,255,229)
            ])
        }

        (ColorTheme::Green3, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[2, 15])
        }
        (ColorTheme::Green3, _) => {
            ThemeColors::from_rgb(&[
                (0, 95, 0),       //  22: (0,373,0) → (0,95,0)
                (0, 135, 0),      //  28: (0,529,0) → (0,135,0)
                (0, 175, 0),      //  34: (0,686,0) → (0,175,0)
                (95, 175, 0),     //  70: (373,686,0) → (95,175,0)
                (95, 215, 0),     //  76: (373,843,0) → (95,215,0)
                (95, 255, 0),     //  82: (373,1000,0) → (95,255,0)
                (175, 255, 175),  // 157: (686,1000,686) → (175,255,175)
            ])
        }

        (ColorTheme::Yellow, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[8, 11, 15])
        }
        (ColorTheme::Yellow, _) => {
            ThemeColors::from_rgb(&[
                (175, 175, 0),    // 184
                (255, 255, 0),    // 226
                (255, 255, 95),   // 227
                (255, 255, 175),  // 229
                (255, 255, 215),  // 230
            ])
        }

        (ColorTheme::Purple, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[5, 7])
        }
        (ColorTheme::Purple, _) => {
            ThemeColors::from_rgb(&[
                (95, 95, 0),      // 60 approximation
                (95, 95, 0),      // 61
                (95, 95, 0),      // 62
                (95, 95, 135),    // 63
                (135, 95, 135),   // 69
                (175, 135, 175),  // 111
                (255, 215, 255),  // 225
            ])
        }

        (ColorTheme::Pink, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[13, 15])
        }
        (ColorTheme::Pink, _) => {
            ThemeColors::from_rgb(&[
                (175, 95, 135),   // 133
                (175, 135, 175),  // 139
                (215, 135, 175),  // 176
                (255, 175, 215),  // 212
                (255, 175, 255),  // 218
                (255, 215, 255),  // 224
                (255, 255, 255),  // 231
            ])
        }

        (ColorTheme::Pink2, ColorMode::Color16) => {
            ThemeColors::from_indexed(&[5, 13, 15])
        }
        (ColorTheme::Pink2, _) => {
            ThemeColors::from_rgb(&[
                (175, 95, 175),   // 145 approximation
                (215, 135, 215),  // 181 approximation
                (255, 175, 255),  // 217
                (255, 175, 255),  // 218
                (255, 215, 255),  // 224
                (255, 215, 255),  // 225
                (255, 255, 255),  // 231
            ])
        }

        // Fallback: default green
        (_, _) => get_theme_colors(ColorTheme::Green, mode),
    }
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
    // 检查是否支持 truecolor (通过 COLORTERM 环境变量)
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
    // 默认回退到 truecolor（大多数现代终端都支持）
    ColorMode::Truecolor
}
