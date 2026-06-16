use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

use crate::cloud::Cloud;
use crate::droplet::Droplet;

/// Custom ratatui Widget that renders the entire Matrix rain effect.
///
/// Iterates all alive droplets and writes each visible character
/// to the appropriate cell in the Buffer.
pub struct RainWidget<'a> {
    pub cloud: &'a Cloud,
    pub now: std::time::Instant,
    pub show_message: bool,
}

impl<'a> Widget for RainWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Use Color::Reset for background so the terminal's default
        // background shows through — the rain appears on a transparent
        // surface rather than forcing black.
        let clear_style = Style::default().bg(Color::Reset);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let cell = &mut buf[(x, y)];
                cell.set_char(' ');
                cell.set_style(clear_style);
            }
        }

        let cloud = self.cloud;

        // Render each alive droplet
        for droplet in cloud.droplets() {
            if !droplet.is_alive {
                continue;
            }

            let col = droplet.bound_col;
            if col >= area.width {
                continue;
            }

            // Use tail_put_line for the visible range (not tail_cur_line,
            // which is a NCurses skip-optimization concept that doesn't
            // apply with ratatui's double-buffering).
            let tail_start = if droplet.tail_put_line != Droplet::SENTINEL {
                droplet.tail_put_line.saturating_add(1)
            } else {
                0
            };
            let head_end = droplet.head_put_line;

            for line in tail_start..=head_end {
                if line >= area.height {
                    break;
                }

                let ch = cloud.get_char(line, droplet.char_pool_idx);
                let ct = droplet.char_loc(line);
                let (pair, is_bold) = cloud.get_attr(
                    line, col, ch, ct, self.now,
                    droplet.head_put_line, droplet.length,
                );

                let style = cloud.theme_colors.style(pair as usize, is_bold);

                let cell = &mut buf[(col, line)];
                cell.set_char(ch);
                cell.set_style(style);
            }
        }

        // Render message characters that are "revealed" by rain
        if self.show_message {
            for mc in &cloud.message {
                if !mc.draw || mc.line >= area.height || mc.col >= area.width {
                    continue;
                }
                let style = cloud.theme_colors.message_style();
                let cell = &mut buf[(mc.col, mc.line)];
                cell.set_char(mc.val);
                cell.set_style(style);
            }
        }
    }
}

/// Overlay widget that renders a status bar at the bottom
pub struct StatusBar<'a> {
    pub chars_per_sec: f32,
    pub droplet_density: f32,
    pub glitch_pct: f32,
    pub char_source_name: &'a str,
    pub color_theme: &'a str,
    pub pause: bool,
}

impl<'a> Widget for StatusBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height == 0 {
            return;
        }

        let pause_str = if self.pause { "[PAUSED]" } else { "" };
        let msg = format!(
            " speed:{:.0} density:{:.1} glitch:{:.0}% chars:{} color:{} {}",
            self.chars_per_sec,
            self.droplet_density,
            self.glitch_pct * 100.0,
            self.char_source_name,
            self.color_theme,
            pause_str,
        );

        let style = Style::default().fg(Color::Gray).bg(Color::Reset);
        for (i, ch) in msg.chars().enumerate() {
            let x = area.left() + i as u16;
            if x >= area.right() {
                break;
            }
            let cell = &mut buf[(x, area.top())];
            cell.set_char(ch);
            cell.set_style(style);
        }
    }
}
