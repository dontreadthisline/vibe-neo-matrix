use serde::{Deserialize, Serialize};

/// 统一参数模型，同时承载 TOML 配置和 CLI 参数的中间表示。
///
/// 所有字段为 `Option<T>`，分层合并时上层 Some 覆盖下层。
/// 合并顺序: Default → XDG config → project config → CLI args
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SimParams {
    // 字符源
    pub charset: Option<String>,
    pub charset_file: Option<std::path::PathBuf>,
    pub charset_stdin: Option<bool>,
    pub chars: Option<String>,

    // 渲染
    pub color: Option<String>,
    pub speed: Option<f32>,
    pub density: Option<f32>,
    pub fps: Option<f64>,
    pub message: Option<String>,
    pub show_status: Option<bool>,
    pub full_width: Option<bool>,
    pub default_bg: Option<bool>,

    // 样式
    pub shading_mode: Option<u8>,
    pub bold: Option<u8>,
    pub color_mode: Option<u16>,
    pub async_scroll: Option<bool>,

    // 雨滴行为
    pub maxdpc: Option<u8>,
    pub short_pct: Option<f32>,
    pub rip_pct: Option<f32>,

    // 故障效果
    pub glitch_ms_low: Option<u16>,
    pub glitch_ms_high: Option<u16>,
    pub glitch_pct: Option<f32>,
    pub no_glitch: Option<bool>,
    pub linger_ms_low: Option<u16>,
    pub linger_ms_high: Option<u16>,

    // 模式
    pub inline: Option<u16>,
    pub screensaver: Option<bool>,
    pub exit_on_key: Option<bool>,
    pub exit_after_secs: Option<f64>,

    // 配置
    pub config_file: Option<std::path::PathBuf>,
}

impl SimParams {
    /// 分层覆盖合并：other 中的 Some 值覆盖 self 中的值（包括 self 中的 Some）
    pub fn merge(self, other: &SimParams) -> Self {
        SimParams {
            charset: other.charset.clone().or(self.charset),
            charset_file: other.charset_file.clone().or(self.charset_file),
            charset_stdin: other.charset_stdin.or(self.charset_stdin),
            chars: other.chars.clone().or(self.chars),
            color: other.color.clone().or(self.color),
            speed: other.speed.or(self.speed),
            density: other.density.or(self.density),
            fps: other.fps.or(self.fps),
            message: other.message.clone().or(self.message),
            show_status: other.show_status.or(self.show_status),
            full_width: other.full_width.or(self.full_width),
            default_bg: other.default_bg.or(self.default_bg),
            shading_mode: other.shading_mode.or(self.shading_mode),
            bold: other.bold.or(self.bold),
            color_mode: other.color_mode.or(self.color_mode),
            async_scroll: other.async_scroll.or(self.async_scroll),
            maxdpc: other.maxdpc.or(self.maxdpc),
            short_pct: other.short_pct.or(self.short_pct),
            rip_pct: other.rip_pct.or(self.rip_pct),
            glitch_ms_low: other.glitch_ms_low.or(self.glitch_ms_low),
            glitch_ms_high: other.glitch_ms_high.or(self.glitch_ms_high),
            glitch_pct: other.glitch_pct.or(self.glitch_pct),
            no_glitch: other.no_glitch.or(self.no_glitch),
            linger_ms_low: other.linger_ms_low.or(self.linger_ms_low),
            linger_ms_high: other.linger_ms_high.or(self.linger_ms_high),
            inline: other.inline.or(self.inline),
            screensaver: other.screensaver.or(self.screensaver),
            exit_on_key: other.exit_on_key.or(self.exit_on_key),
            exit_after_secs: other.exit_after_secs.or(self.exit_after_secs),
            config_file: other.config_file.clone().or(self.config_file),
        }
    }
}
