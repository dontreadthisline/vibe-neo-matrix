use std::io;
use std::time::{Duration, Instant};

use rand::rngs::ThreadRng;
use rand::Rng;

use crate::char_source::{CharSource, BuiltinChars};
use crate::color::{ColorMode, ColorTheme, ShadingMode, BoldMode, ThemeColors, get_theme_colors};
use crate::droplet::{Droplet, CharLoc};
use crate::params::SimParams;

pub const CHAR_POOL_SIZE: usize = 2048;
pub const GLITCH_POOL_SIZE: usize = 1024;
pub const MAX_DROPLETS_PER_COL: usize = 4;

/// Per-column status tracking
#[derive(Debug, Clone)]
pub struct ColumnStatus {
    pub max_speed_pct: f32,
    pub num_droplets: u8,
    pub can_spawn: bool,
}

/// A message character pre-positioned on screen.
/// Only rendered when rain passes through its exact location.
#[derive(Debug, Clone)]
pub struct MessageChar {
    pub line: u16,
    pub col: u16,
    pub val: char,
    pub draw: bool,
}

/// The Cloud is the core simulation engine.
/// Manages all columns, droplets, character pools, and rendering state.
pub struct Cloud {
    // Terminal dimensions
    pub lines: u16,
    pub cols: u16,

    // Droplets
    droplets: Vec<Droplet>,
    num_droplets: usize,
    col_stat: Vec<ColumnStatus>,

    // Character source & pools
    char_source: Box<dyn CharSource>,
    char_pool: Vec<char>,
    glitch_pool: Vec<char>,
    pub glitch_pool_idx: usize,

    // Glitch
    glitch_map: Vec<bool>,
    pub glitch_low_ms: u16,
    pub glitch_high_ms: u16,
    pub glitch_pct: f32,
    pub glitchy: bool,

    // Color
    color_pair_map: Vec<u8>,
    pub theme_colors: ThemeColors,
    pub color_theme: ColorTheme,
    pub color_mode: ColorMode,
    pub shading_mode: ShadingMode,
    pub bold_mode: BoldMode,
    num_color_pairs: u8,

    // Message
    pub message: Vec<MessageChar>,
    message_empty: bool,

    // Parameters
    pub chars_per_sec: f32,
    pub droplet_density: f32,
    droplets_per_sec: f32,
    pub max_droplets_per_col: u8,
    pub short_pct: f32,
    pub die_early_pct: f32,
    pub async_scroll: bool,
    pub full_width: bool,
    pub default_background: bool,

    // Timing
    pub last_glitch_time: Instant,
    pub next_glitch_time: Instant,
    last_spawn_time: Instant,
    pause_time: Option<Instant>,

    // State
    pub pause: bool,
    pub raining: bool,
    pub force_draw_everything: bool,

    // RNG
    rng: ThreadRng,

    // RNG distributions (cached ranges)
    rand_color_pair_low: i32,
    rand_color_pair_high: i32,
    linger_low_ms: u16,
    linger_high_ms: u16,
}

impl Cloud {
    pub fn new(
        lines: u16,
        cols: u16,
        color_mode: ColorMode,
        char_source: Box<dyn CharSource>,
    ) -> Self {
        let num_droplets = (1.5 * cols as f32).round() as usize;
        let color_theme = ColorTheme::Green;

        let mut cloud = Cloud {
            lines,
            cols,
            droplets: Vec::new(),
            num_droplets,
            col_stat: Vec::new(),
            char_source,
            char_pool: Vec::new(),
            glitch_pool: Vec::new(),
            glitch_pool_idx: 0,
            glitch_map: Vec::new(),
            glitch_low_ms: 300,
            glitch_high_ms: 400,
            glitch_pct: 0.1,
            glitchy: true,
            color_pair_map: Vec::new(),
            theme_colors: get_theme_colors(color_theme, color_mode),
            color_theme,
            color_mode,
            shading_mode: ShadingMode::Random,
            bold_mode: BoldMode::Random,
            num_color_pairs: 7,
            message: Vec::new(),
            message_empty: true,
            chars_per_sec: 8.0,
            droplet_density: 1.0,
            droplets_per_sec: 0.0,
            max_droplets_per_col: 3,
            short_pct: 0.5,
            die_early_pct: 0.333,
            async_scroll: false,
            full_width: false,
            default_background: true,
            last_glitch_time: Instant::now(),
            next_glitch_time: Instant::now(),
            last_spawn_time: Instant::now(),
            pause_time: None,
            pause: false,
            raining: true,
            force_draw_everything: false,
            rng: rand::thread_rng(),
            rand_color_pair_low: 2,
            rand_color_pair_high: 5,
            linger_low_ms: 1,
            linger_high_ms: 3000,
        };

        cloud.init_chars();
        cloud.reset();
        cloud
    }

    // ============================================================
    // Initialization
    // ============================================================

    /// Precompute character pools from the current CharSource
    pub fn init_chars(&mut self) {
        let src_chars = self.char_source.chars();
        if src_chars.is_empty() {
            // Fallback: use katakana
            let fallback = BuiltinChars::from_charset_name("katakana");
            let fallback_chars = fallback.chars().to_vec();
            self.char_pool.resize(CHAR_POOL_SIZE, ' ');
            self.glitch_pool.resize(GLITCH_POOL_SIZE, ' ');
            for i in 0..CHAR_POOL_SIZE {
                self.char_pool[i] = fallback_chars[self.rng.gen_range(0..fallback_chars.len())];
            }
            for i in 0..GLITCH_POOL_SIZE {
                self.glitch_pool[i] = fallback_chars[self.rng.gen_range(0..fallback_chars.len())];
            }
        } else {
            self.char_pool.resize(CHAR_POOL_SIZE, ' ');
            self.glitch_pool.resize(GLITCH_POOL_SIZE, ' ');
            let n = src_chars.len();
            for i in 0..CHAR_POOL_SIZE {
                self.char_pool[i] = src_chars[self.rng.gen_range(0..n)];
            }
            for i in 0..GLITCH_POOL_SIZE {
                self.glitch_pool[i] = src_chars[self.rng.gen_range(0..n)];
            }
        }
        self.glitch_pool_idx = 0;
    }

    /// Reset all state (called on startup and resize)
    pub fn reset(&mut self) {
        self.num_droplets = (1.5 * self.cols as f32).round() as usize;
        self.droplets.clear();
        self.droplets.resize_with(self.num_droplets, Droplet::new);
        for d in &mut self.droplets {
            d.reset();
        }

        // RNG
        // Use a fixed-ish seed like original neo for reproducibility
        // (original uses mt.seed(0x1234567))
        self.rng = rand::thread_rng();

        // Color pair ranges for random assignment
        if self.num_color_pairs < 3 {
            self.rand_color_pair_low = 1;
            self.rand_color_pair_high = 1;
        } else if self.num_color_pairs == 3 {
            self.rand_color_pair_low = 2;
            self.rand_color_pair_high = 2;
        } else {
            self.rand_color_pair_low = 2;
            self.rand_color_pair_high = self.num_color_pairs as i32 - 2;
        }

        let screen_size = (self.lines as usize) * (self.cols as usize);
        self.fill_glitch_map(screen_size);
        self.fill_color_map(screen_size);

        let droplet_seconds = self.lines as f32 / self.chars_per_sec;
        self.droplets_per_sec = self.cols as f32 * self.droplet_density / droplet_seconds;

        self.col_stat.clear();
        self.col_stat.resize(self.cols as usize, ColumnStatus {
            max_speed_pct: 1.0,
            num_droplets: 0,
            can_spawn: true,
        });
        self.set_async(self.async_scroll);
        self.set_column_speeds();
        self.update_droplet_speeds();

        if !self.message_empty {
            self.reset_message();
        }

        self.last_glitch_time = Instant::now();
        self.next_glitch_time = self.last_glitch_time
            + Duration::from_millis(self.rand_glitch_ms() as u64);
        self.last_spawn_time = self.last_glitch_time;
    }

    /// Reset with new terminal size
    pub fn reset_with_size(&mut self, cols: u16, lines: u16) {
        self.cols = cols;
        self.lines = lines;
        self.reset();
        self.force_draw_everything = true;
    }

    fn fill_glitch_map(&mut self, screen_size: usize) {
        if !self.glitchy {
            return;
        }
        self.glitch_map.resize(screen_size, false);
        let pct = self.glitch_pct;
        for entry in self.glitch_map.iter_mut() {
            *entry = self.rng.gen::<f32>() <= pct;
        }
    }

    fn fill_color_map(&mut self, screen_size: usize) {
        self.color_pair_map.resize(screen_size, 0);
        let low = self.rand_color_pair_low;
        let high = self.rand_color_pair_high;
        for entry in self.color_pair_map.iter_mut() {
            *entry = self.rng.gen_range(low..=high) as u8;
        }
    }

    // ============================================================
    // Per-frame update
    // ============================================================

    pub fn rain(&mut self) {
        if self.pause {
            return;
        }

        let now = Instant::now();
        self.spawn_droplets(now);

        let time_for_glitch = self.time_for_glitch(now);

        // Iterate with index to modify droplets in place
        for i in 0..self.droplets.len() {
            if !self.droplets[i].is_alive {
                continue;
            }
            let prev_tail_put = self.droplets[i].tail_put_line;
            self.droplets[i].advance(now);

            // After advancing, check if tail crossed the 1/4 threshold
            // to allow the column to spawn again (matches original neo)
            if self.droplets[i].is_alive {
                let col = self.droplets[i].bound_col as usize;
                let thresh = self.lines / 4;
                let new_tail = self.droplets[i].tail_put_line;
                if col < self.col_stat.len()
                    && prev_tail_put.map_or(true, |p| p <= thresh)
                    && new_tail.map_or(false, |n| n > thresh)
                {
                    self.col_stat[col].can_spawn = true;
                }
            }

            if time_for_glitch {
                self.do_glitch(i);
            }

            if !self.droplets[i].is_alive {
                let col = self.droplets[i].bound_col as usize;
                if col < self.col_stat.len() {
                    self.col_stat[col].num_droplets = self.col_stat[col].num_droplets.saturating_sub(1);
                    // If died early, allow respawn
                    if self.droplets[i].tail_put_line.map_or(true, |t| t <= self.lines / 4) {
                        self.col_stat[col].can_spawn = true;
                    }
                }
            }
        }

        if !self.message_empty {
            self.calc_message();
        }

        if time_for_glitch {
            self.last_glitch_time = now;
            self.next_glitch_time = now
                + Duration::from_millis(self.rand_glitch_ms() as u64);
        }
    }

    fn spawn_droplets(&mut self, now: Instant) {
        if self.last_spawn_time > now {
            self.last_spawn_time = now;
            return;
        }
        let elapsed_secs = (now - self.last_spawn_time).as_secs_f32();
        let to_spawn = (elapsed_secs * self.droplets_per_sec) as usize;
        let to_spawn = to_spawn.min(self.num_droplets);

        if to_spawn == 0 {
            return;
        }

        let droplet_idx = 0usize;
        let mut spawned = 0;

        for _ in 0..to_spawn {
            let mut col = self.rng.gen_range(0..self.cols);
            if self.full_width {
                col &= 0xFFFE; // even columns only
            }

            let col_idx = col as usize;
            if !self.col_stat[col_idx].can_spawn
                || self.col_stat[col_idx].num_droplets >= self.max_droplets_per_col
            {
                continue;
            }

            // Find an inactive droplet slot
            let slot = (droplet_idx..self.num_droplets)
                .find(|&j| !self.droplets[j].is_alive);

            let slot = match slot {
                Some(s) => s,
                None => break,
            };

            self.fill_droplet(slot, col);
            self.droplets[slot].activate(now);
            self.col_stat[col_idx].can_spawn = false;
            self.col_stat[col_idx].num_droplets += 1;
            spawned += 1;
        }

        if spawned > 0 {
            self.last_spawn_time = now;
        }
    }

    fn fill_droplet(&mut self, slot: usize, col: u16) {
        let end_line = if self.rng.gen::<f32>() <= self.die_early_pct {
            self.rng.gen_range(0..self.lines.saturating_sub(1))
        } else {
            self.lines.saturating_sub(1)
        };

        let cp_idx = self.rng.gen_range(0..CHAR_POOL_SIZE as u16);
        let len = if self.rng.gen::<f32>() <= self.short_pct {
            self.rng.gen_range(1..self.lines.saturating_sub(1).max(2))
        } else {
            self.lines
        };

        let ttl = if end_line <= len {
            Duration::from_millis(self.rand_linger_ms() as u64)
        } else {
            Duration::from_millis(1)
        };

        let speed = self.col_stat[col as usize].max_speed_pct * self.chars_per_sec;

        let d = &mut self.droplets[slot];
        d.bound_col = col;
        d.end_line = end_line;
        d.char_pool_idx = cp_idx;
        d.length = len;
        d.chars_per_sec = speed;
        d.time_to_linger = ttl;
    }

    // ============================================================
    // Glitch
    // ============================================================

    fn time_for_glitch(&self, now: Instant) -> bool {
        self.glitchy && now >= self.next_glitch_time
    }

    fn do_glitch(&mut self, droplet_idx: usize) {
        if !self.glitchy {
            return;
        }

        let d = &self.droplets[droplet_idx];
        let start_line = d.tail_put_line.map_or(0, |t| t.saturating_add(1));
        let end_line = d.head_put_line;
        let col = d.bound_col;
        let cp_idx = d.char_pool_idx;

        for line in start_line..=end_line {
            if self.is_glitched(line, col) {
                let char_idx = (cp_idx as usize + line as usize) % CHAR_POOL_SIZE;
                self.char_pool[char_idx] = self.glitch_pool[self.glitch_pool_idx];
                self.glitch_pool_idx = (self.glitch_pool_idx + 1) % GLITCH_POOL_SIZE;
            }
        }
    }

    pub fn is_glitched(&self, line: u16, col: u16) -> bool {
        if !self.glitchy {
            return false;
        }
        let idx = (col as usize) * (self.lines as usize) + (line as usize);
        self.glitch_map.get(idx).copied().unwrap_or(false)
    }

    /// Is the current time in the "bright" phase (0–25% of glitch cycle)?
    pub fn is_bright(&self, now: Instant) -> bool {
        if now < self.last_glitch_time {
            return false;
        }
        let since_glitch = (now - self.last_glitch_time).as_nanos() as f64;
        let between_glitches = (self.next_glitch_time - self.last_glitch_time).as_nanos() as f64;
        if between_glitches <= 0.0 {
            return false;
        }
        since_glitch / between_glitches <= 0.25
    }

    /// Is the current time in the "dim" phase (75%+ of glitch cycle)?
    pub fn is_dim(&self, now: Instant) -> bool {
        if now > self.next_glitch_time {
            return true;
        }
        let since_glitch = (now - self.last_glitch_time).as_nanos() as f64;
        let between_glitches = (self.next_glitch_time - self.last_glitch_time).as_nanos() as f64;
        if between_glitches <= 0.0 {
            return false;
        }
        since_glitch / between_glitches >= 0.75
    }

    // ============================================================
    // Character access
    // ============================================================

    /// Get the character at a specific (col, line) position within a droplet
    pub fn get_char(&self, line: u16, cp_idx: u16) -> char {
        let idx = (cp_idx as usize + line as usize) % CHAR_POOL_SIZE;
        self.char_pool.get(idx).copied().unwrap_or(' ')
    }

    // ============================================================
    // Color / Attribute calculation
    // ============================================================

    /// Get color pair and bold flag for a character at (line, col)
    #[allow(clippy::too_many_arguments)]
    pub fn get_attr(
        &self,
        line: u16,
        col: u16,
        val: char,
        ct: CharLoc,
        now: Instant,
        head_put_line: u16,
        length: u16,
    ) -> (u8, bool) {
        let mut is_bold = match self.bold_mode {
            BoldMode::Random => (line as u32 ^ val as u32) % 2 == 1,
            BoldMode::All => true,
            BoldMode::Off => false,
        };

        let idx = (col as usize) * (self.lines as usize) + (line as usize);
        let mut color_pair = *self.color_pair_map.get(idx).unwrap_or(&2);

        if self.shading_mode == ShadingMode::DistanceFromHead {
            let dist = head_put_line.saturating_sub(line);
            let ratio = dist as f32 / length.max(1) as f32;
            color_pair = self.num_color_pairs
                - (ratio * (self.num_color_pairs as f32 - 1.0)).round() as u8;
            color_pair = color_pair.clamp(1, self.num_color_pairs);
        }

        if self.glitchy && self.is_glitched(line, col) {
            if self.is_bright(now) {
                color_pair = (color_pair + 1).min(self.num_color_pairs);
                is_bold = true;
            } else if self.is_dim(now) {
                color_pair = color_pair.saturating_sub(1).max(1);
                is_bold = false;
            }
        }

        match ct {
            CharLoc::Tail => {
                color_pair = 1;
                is_bold = false;
            }
            CharLoc::Head => {
                color_pair = self.num_color_pairs;
                is_bold = true;
            }
            CharLoc::Middle => {
                color_pair = color_pair.clamp(1, self.num_color_pairs.saturating_sub(1));
                color_pair = color_pair.max(1);
            }
        }

        if self.bold_mode == BoldMode::Off {
            is_bold = false;
        } else if self.bold_mode == BoldMode::All {
            is_bold = true;
        }

        (color_pair, is_bold)
    }

    // ============================================================
    // Setters / Getters
    // ============================================================

    pub fn droplets(&self) -> &[Droplet] {
        &self.droplets
    }

    pub fn set_chars_per_sec(&mut self, cps: f32) {
        self.chars_per_sec = cps;
        let droplet_seconds = self.lines as f32 / self.chars_per_sec;
        self.droplets_per_sec = self.cols as f32 * self.droplet_density / droplet_seconds;
        self.set_column_speeds();
        self.update_droplet_speeds();
    }

    pub fn set_droplet_density(&mut self, density: f32) {
        self.droplet_density = density;
        let droplet_seconds = self.lines as f32 / self.chars_per_sec;
        self.droplets_per_sec = self.cols as f32 * self.droplet_density / droplet_seconds;
    }

    pub fn set_color(&mut self, theme: ColorTheme) {
        self.color_theme = theme;
        self.theme_colors = get_theme_colors(theme, self.color_mode);
        // Determine number of color pairs from the theme
        self.num_color_pairs = self.theme_colors.num_pairs as u8;
        self.reset();
        self.force_draw_everything = true;
    }

    pub fn set_shading_mode(&mut self, mode: ShadingMode) {
        self.shading_mode = mode;
        self.force_draw_everything = true;
    }

    pub fn set_bold_mode(&mut self, mode: BoldMode) {
        self.bold_mode = mode;
    }

    pub fn set_glitch_pct(&mut self, pct: f32) {
        self.glitch_pct = pct.clamp(0.0, 1.0);
        let screen_size = (self.lines as usize) * (self.cols as usize);
        self.fill_glitch_map(screen_size);
    }

    pub fn set_glitch_times(&mut self, low_ms: u16, high_ms: u16) {
        self.glitch_low_ms = low_ms;
        self.glitch_high_ms = high_ms;
    }

    pub fn set_glitchy(&mut self, b: bool) {
        self.glitchy = b;
        if !b {
            self.glitch_pct = 0.0;
            self.glitch_low_ms = 0xFFFF;
            self.glitch_high_ms = 0xFFFF;
        }
    }

    pub fn set_short_pct(&mut self, pct: f32) {
        self.short_pct = pct.clamp(0.0, 1.0);
    }

    pub fn set_die_early_pct(&mut self, pct: f32) {
        self.die_early_pct = pct.clamp(0.0, 1.0);
    }

    pub fn set_linger_times(&mut self, low_ms: u16, high_ms: u16) {
        self.linger_low_ms = low_ms;
        self.linger_high_ms = high_ms;
    }

    pub fn set_max_droplets_per_col(&mut self, val: u8) {
        self.max_droplets_per_col = val.min(MAX_DROPLETS_PER_COL as u8);
    }

    pub fn set_async(&mut self, b: bool) {
        self.async_scroll = b;
    }

    /// 批量应用参数，顺序敏感：speed > density > color > 其他
    pub fn apply_params(&mut self, p: &SimParams) {
        // Speed (must precede density — both affect droplets_per_sec)
        if let Some(s) = p.speed {
            self.set_chars_per_sec(s);
        }
        // Density
        if let Some(d) = p.density {
            self.set_droplet_density(d);
        }
        // Color (calls reset() — must come before glitch/short/die_early changes)
        if let Some(ref c) = p.color {
            self.set_color(ColorTheme::from_name(c));
        }
        // Shading mode
        if let Some(m) = p.shading_mode {
            self.set_shading_mode(match m { 0 => ShadingMode::Random, _ => ShadingMode::DistanceFromHead });
        }
        // Bold mode
        if let Some(b) = p.bold {
            self.set_bold_mode(match b { 0 => BoldMode::Off, 2 => BoldMode::All, _ => BoldMode::Random });
        }
        // Glitch
        if p.no_glitch.unwrap_or(false) {
            self.set_glitchy(false);
        } else {
            if let Some(pct) = p.glitch_pct {
                self.set_glitch_pct(pct / 100.0);
            }
            if let (Some(lo), Some(hi)) = (p.glitch_ms_low, p.glitch_ms_high) {
                self.set_glitch_times(lo, hi);
            } else if let Some(lo) = p.glitch_ms_low {
                self.set_glitch_times(lo, self.glitch_high_ms);
            } else if let Some(hi) = p.glitch_ms_high {
                self.set_glitch_times(self.glitch_low_ms, hi);
            }
        }
        // Linger
        if let (Some(lo), Some(hi)) = (p.linger_ms_low, p.linger_ms_high) {
            self.set_linger_times(lo, hi);
        } else if let Some(lo) = p.linger_ms_low {
            self.set_linger_times(lo, self.linger_high_ms);
        } else if let Some(hi) = p.linger_ms_high {
            self.set_linger_times(self.linger_low_ms, hi);
        }
        // Short / die-early
        if let Some(pct) = p.short_pct {
            self.set_short_pct(pct / 100.0);
        }
        if let Some(pct) = p.rip_pct {
            self.set_die_early_pct(pct / 100.0);
        }
        // Max droplets per col
        if let Some(m) = p.maxdpc {
            self.set_max_droplets_per_col(m);
        }
        // Async
        if p.async_scroll.unwrap_or(false) {
            self.set_async(true);
        }
        // Full width / default bg
        if p.full_width.unwrap_or(false) {
            self.full_width = true;
        }
        if p.default_bg.unwrap_or(false) {
            self.default_background = true;
        }
        // Screensaver
        if p.screensaver.unwrap_or(false) {
            self.raining = true;
        }
        // Message
        if let Some(ref msg) = p.message {
            self.set_message(msg);
        }
    }

    fn set_column_speeds(&mut self) {
        if self.async_scroll {
            for cs in &mut self.col_stat {
                cs.max_speed_pct = self.rng.gen_range(0.333..=1.0);
            }
        } else {
            for cs in &mut self.col_stat {
                cs.max_speed_pct = 1.0;
            }
        }
    }

    fn update_droplet_speeds(&mut self) {
        for i in 0..self.droplets.len() {
            if !self.droplets[i].is_alive {
                continue;
            }
            let col = self.droplets[i].bound_col as usize;
            if col < self.col_stat.len() {
                self.droplets[i].chars_per_sec =
                    self.col_stat[col].max_speed_pct * self.chars_per_sec;
            }
        }
    }

    pub fn toggle_pause(&mut self) {
        if self.pause {
            // Resuming
            if let Some(pt) = self.pause_time {
                let elapsed = Instant::now() - pt;
                self.last_spawn_time += elapsed;
                for d in &mut self.droplets {
                    if d.is_alive {
                        d.increment_time(elapsed);
                    }
                }
            }
            self.pause = false;
        } else {
            self.pause_time = Some(Instant::now());
            self.pause = true;
        }
    }

    pub fn char_source_name(&self) -> &str {
        self.char_source.name()
    }

    pub fn set_raining(&mut self, b: bool) {
        self.raining = b;
    }

    /// Reload char source and rebuild character pools.
    /// Used for live-updating sources like Claude session file.
    pub fn reload_chars(&mut self) -> io::Result<()> {
        self.char_source.reload()?;
        self.init_chars();
        self.force_draw_everything = true;
        Ok(())
    }

    // ============================================================
    // Message
    // ============================================================

    pub fn set_message(&mut self, msg: &str) {
        self.message = msg.chars().map(|c| MessageChar {
            line: 0,
            col: 0,
            val: c,
            draw: false,
        }).collect();
        self.message_empty = self.message.is_empty();
    }

    fn reset_message(&mut self) {
        if self.message.is_empty() {
            return;
        }
        let first_col = self.cols / 4;
        let last_col = 3 * self.cols / 4;
        let chars_per_col = (last_col - first_col + 1) as usize;
        let msg_lines = (self.message.len() / chars_per_col.max(1)) as u16 + 1;
        let first_line = self.lines / 2 - msg_lines / 2;

        let total = self.message.len();
        let mut chars_remaining = total;
        let mut line = first_line;
        let mut col = first_col;

        if chars_remaining < chars_per_col {
            col += ((chars_per_col - chars_remaining) / 2) as u16;
        }

        for mc in &mut self.message {
            mc.draw = false;
            if line < self.lines {
                mc.line = line;
                mc.col = col;
            } else {
                mc.line = 0xFFFF;
                mc.col = 0xFFFF;
            }
            if col == last_col {
                line += 1;
                col = first_col;
                if chars_remaining < chars_per_col {
                    col += ((chars_per_col - chars_remaining) / 2) as u16;
                }
            } else {
                col += 1;
            }
            chars_remaining = chars_remaining.saturating_sub(1);
        }
    }

    /// Check which message positions currently have rain characters rendered.
    /// In the original neo, this uses `mvinnwstr` to read the screen.
    /// In our implementation, we check if any alive droplet covers the position.
    fn calc_message(&mut self) {
        for mc in &mut self.message {
            if mc.line == 0xFFFF || mc.col == 0xFFFF {
                continue;
            }
            // Check if any alive droplet covers this position.
            // Use tail_put_line (actual tail position) instead of tail_cur_line
            // (NCurses skip-optimization field) for the visible range.
            mc.draw = self.droplets.iter().any(|d| {
                if !d.is_alive || d.bound_col != mc.col {
                    return false;
                }
                let tail_start = d.tail_put_line.map_or(0, |t| t.saturating_add(1));
                mc.line >= tail_start && mc.line <= d.head_put_line
            });
        }
    }

    // ============================================================
    // RNG helpers
    // ============================================================

    fn rand_glitch_ms(&mut self) -> u16 {
        if self.glitch_low_ms >= self.glitch_high_ms {
            return self.glitch_low_ms;
        }
        self.rng.gen_range(self.glitch_low_ms..=self.glitch_high_ms)
    }

    fn rand_linger_ms(&mut self) -> u16 {
        if self.linger_low_ms >= self.linger_high_ms {
            return self.linger_low_ms;
        }
        self.rng.gen_range(self.linger_low_ms..=self.linger_high_ms)
    }
}
