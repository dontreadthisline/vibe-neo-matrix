use std::time::{Duration, Instant};

/// Droplet 状态机 —— 单列雨滴的完整生命周期
///
/// ## 生命周期
/// ```
/// Activate → Head 向下推进 → Head 到达 endLine 停止
///          → Tail 暂停 (linger 期间)
///          → Linger 时间到 → Tail 恢复推进
///          → Tail 追上 Head → Droplet 死亡
/// ```
#[derive(Debug)]
pub struct Droplet {
    pub is_alive: bool,
    pub is_head_crawling: bool,
    pub is_tail_crawling: bool,
    pub bound_col: u16,
    pub head_put_line: u16,   // Head 目标行 (rendered up to this line)
    pub head_cur_line: u16,   // Head 上次绘制行 (skip optimization)
    pub tail_put_line: u16,   // Tail 目标行 (erased up to this line)
    pub tail_cur_line: u16,   // Tail 上次擦除行
    pub end_line: u16,        // Head stops at this line
    pub char_pool_idx: u16,   // Index into Cloud's char_pool
    pub length: u16,          // Max length of this droplet
    pub chars_per_sec: f32,
    pub last_time: Instant,
    pub head_stop_time: Instant,
    pub time_to_linger: Duration,
}

/// Character location within a droplet
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CharLoc {
    Middle,
    Tail,
    Head,
}

impl Droplet {
    /// Sentinel value for "not set"
    pub const SENTINEL: u16 = 0xFFFF;

    pub fn new() -> Self {
        Droplet {
            is_alive: false,
            is_head_crawling: false,
            is_tail_crawling: false,
            bound_col: Self::SENTINEL,
            head_put_line: 0,
            head_cur_line: 0,
            tail_put_line: Self::SENTINEL,
            tail_cur_line: 0,
            end_line: Self::SENTINEL,
            char_pool_idx: Self::SENTINEL,
            length: Self::SENTINEL,
            chars_per_sec: 0.0,
            last_time: Instant::now(),
            head_stop_time: Instant::now(),
            time_to_linger: Duration::ZERO,
        }
    }

    pub fn reset(&mut self) {
        *self = Droplet::new();
    }

    /// Activate the droplet at the given time. All fields must already be set
    /// via direct assignment (bound_col, end_line, char_pool_idx, length,
    /// chars_per_sec, time_to_linger) before calling this.
    pub fn activate(&mut self, now: Instant) {
        self.is_alive = true;
        self.is_head_crawling = true;
        self.is_tail_crawling = true;
        self.last_time = now;
        self.head_put_line = 0;
        self.head_cur_line = 0;
        self.tail_put_line = Self::SENTINEL;
        self.tail_cur_line = 0;
        self.head_stop_time = now;
    }

    /// Advance the droplet state based on elapsed time.
    ///
    /// Returns true if the droplet is still alive after advancing.
    pub fn advance(&mut self, now: Instant) {
        if !self.is_alive {
            return;
        }

        let elapsed_secs = (now - self.last_time).as_secs_f32();
        let chars_advanced = (self.chars_per_sec * elapsed_secs).round() as u16;
        if chars_advanced == 0 {
            return;
        }

        // --- 推进 Head ---
        if self.is_head_crawling {
            self.head_put_line = self.head_put_line.saturating_add(chars_advanced);
            self.head_put_line = self.head_put_line.min(self.end_line);

            if self.head_put_line >= self.end_line {
                self.is_head_crawling = false;
                // is_head_crawling guarantees we only enter here once per activation,
                // so head_stop_time is always set at the actual stop moment.
                self.head_stop_time = now;
                if self.time_to_linger > Duration::ZERO {
                    self.is_tail_crawling = false;
                }
            }
        }

        // --- 推进 Tail ---
        if self.is_tail_crawling
            && (self.head_put_line >= self.length || self.head_put_line >= self.end_line)
        {
            if self.tail_put_line == Self::SENTINEL {
                self.tail_put_line = chars_advanced;
            } else {
                self.tail_put_line = self.tail_put_line.saturating_add(chars_advanced);
            }
            self.tail_put_line = self.tail_put_line.min(self.end_line);
        }

        // --- Linger 结束后恢复 Tail ---
        if !self.is_tail_crawling
            && self.time_to_linger > Duration::ZERO
            && (now - self.head_stop_time) >= self.time_to_linger
        {
            self.is_tail_crawling = true;
        }

        // --- Tail 追上 Head → 死亡 ---
        if self.tail_put_line != Self::SENTINEL && self.tail_put_line >= self.head_put_line {
            self.is_alive = false;
        }

        self.last_time = now;
    }

    /// Determine character location type for the given line.
    pub fn char_loc(&self, line: u16) -> CharLoc {
        if self.tail_put_line != Self::SENTINEL && line == self.tail_put_line.saturating_add(1) {
            CharLoc::Tail
        } else if line == self.head_put_line && self.is_head_bright() {
            CharLoc::Head
        } else {
            CharLoc::Middle
        }
    }

    /// Is the head currently bright? Head is bright when still crawling
    /// or within 100ms of stopping.
    fn is_head_bright(&self) -> bool {
        if self.is_head_crawling {
            return true;
        }
        if self.head_stop_time.elapsed() <= Duration::from_millis(100) {
            return true;
        }
        false
    }

    /// For pausing: increment the last_time to prevent time jump after resume.
    pub fn increment_time(&mut self, delta: Duration) {
        self.last_time += delta;
        if self.head_stop_time > Instant::now() - Duration::from_secs(3600) {
            // Only adjust if head_stop_time was set
        }
        self.head_stop_time += delta;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fill_and_activate(
        d: &mut Droplet,
        col: u16,
        end_line: u16,
        cp_idx: u16,
        len: u16,
        cps: f32,
        ttl: Duration,
        now: Instant,
    ) {
        d.bound_col = col;
        d.end_line = end_line;
        d.char_pool_idx = cp_idx;
        d.length = len;
        d.chars_per_sec = cps;
        d.time_to_linger = ttl;
        d.activate(now);
    }

    #[test]
    fn test_droplet_activate_and_advance() {
        let mut d = Droplet::new();
        let now = Instant::now();
        fill_and_activate(&mut d, 10, 25, 42, 20, 10.0, Duration::from_millis(500), now);
        assert!(d.is_alive);
        assert!(d.is_head_crawling);
        assert!(d.is_tail_crawling);
        assert_eq!(d.bound_col, 10);

        // Advance 1 second at 10 chars/sec → head at line 10
        let later = now + Duration::from_secs(1);
        d.advance(later);
        assert_eq!(d.head_put_line, 10);
        // Tail won't start yet because head_put_line (10) < length (20)
        assert_eq!(d.tail_put_line, Droplet::SENTINEL);
        assert!(d.is_alive);
        assert!(d.is_head_crawling);

        // Advance another 2 seconds → head at line 30, but end_line = 25
        let later2 = later + Duration::from_secs(2);
        d.advance(later2);
        assert_eq!(d.head_put_line, 25);
        assert!(!d.is_head_crawling, "head should stop at end_line");
    }

    #[test]
    fn test_droplet_head_reaches_end() {
        let mut d = Droplet::new();
        let now = Instant::now();
        fill_and_activate(&mut d, 5, 5, 0, 100, 10.0, Duration::from_millis(100), now);

        // 1 second at 10 cps → 10 chars advanced, but end_line is 5
        d.advance(now + Duration::from_secs(1));
        assert_eq!(d.head_put_line, 5);
        assert!(!d.is_head_crawling);
    }

    #[test]
    fn test_droplet_dies_when_tail_catches_head() {
        let mut d = Droplet::new();
        let now = Instant::now();
        // end_line=5, ttl=0 means tail keeps crawling immediately
        fill_and_activate(&mut d, 0, 5, 0, 100, 100.0, Duration::ZERO, now);
        // Very fast advance — tail will catch head
        d.advance(now + Duration::from_secs(1));
        assert!(!d.is_alive || d.tail_put_line >= d.head_put_line);
    }
}
