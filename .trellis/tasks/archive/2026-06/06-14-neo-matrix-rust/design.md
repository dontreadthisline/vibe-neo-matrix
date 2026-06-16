# neo-rainst 技术设计

## 1. Crate 结构

单个 bin crate，模块化组织：

```
neo-rainst/
├── Cargo.toml
├── src/
│   ├── main.rs          # 入口、CLI 解析、主循环
│   ├── cloud.rs         # Cloud: 列状态管理、雨滴生命周期
│   ├── droplet.rs       # Droplet: 单列雨滴 Head/Tail 状态机
│   ├── char_source.rs   # CharSource trait 及各实现
│   ├── color.rs         # 颜色主题、渐变计算
│   ├── config.rs        # TOML 配置加载
│   └── render.rs        # ratatui 自定义 Widget 渲染
```

### Dependencies

```toml
[dependencies]
ratatui = "0.29"
crossterm = "0.28"
clap = { version = "4", features = ["derive"] }
toml = "0.8"
serde = { version = "1", features = ["derive"] }
rand = "0.8"
```

`notify` (文件监听) 作为可选 feature，Phase 3 引入。

## 2. 核心数据流

```
main()
  │
  ├─ clap 解析 CLI args
  ├─ 加载 XDG config.toml → Config struct
  ├─ CLI args 覆盖 Config
  │
  ├─ ratatui::init()        ← crossterm backend, raw mode, alternate screen
  ├─ Cloud::new(config)
  ├─ cloud.init_chars()     ← 预计算 char_pool[2048], glitch_pool[1024]
  ├─ cloud.reset()          ← 初始化列、RNG、glitch map、color map
  │
  └─ loop:
       ├─ crossterm::event::poll(0ms) → handle input/resize
       ├─ Cloud::rain()              → 推进模拟
       ├─ terminal.draw(render_widget)  → ratatui 渲染
       └─ sleep(curDelay)            ← 帧率控制
```

## 3. Cloud（核心引擎）

```rust
const CHAR_POOL_SIZE: usize = 2048;
const GLITCH_POOL_SIZE: usize = 1024;
const MAX_DROPLETS_PER_COL: usize = 4;
const NUM_COLOR_PAIRS: usize = 7;

struct Cloud {
    // 终端尺寸
    lines: u16,
    cols: u16,

    // 雨滴
    droplets: Vec<Droplet>,      // 预分配 numDroplets = 1.5 * cols
    col_stat: Vec<ColumnStatus>, // 每列状态

    // 字符池
    char_source: Box<dyn CharSource>,
    char_pool: Vec<char>,        // [CHAR_POOL_SIZE]
    glitch_pool: Vec<char>,      // [GLITCH_POOL_SIZE]
    glitch_pool_idx: usize,

    // Glitch
    glitch_map: Vec<bool>,       // [lines * cols]
    glitch_low_ms: u16,          // 默认 300
    glitch_high_ms: u16,         // 默认 400
    glitch_pct: f32,             // 默认 0.1

    // 颜色
    color_pair_map: Vec<u8>,     // [lines * cols]
    color_theme: ColorTheme,
    color_mode: ColorMode,
    shading_mode: ShadingMode,
    bold_mode: BoldMode,

    // 消息
    message: Vec<MessageChar>,

    // 参数
    chars_per_sec: f32,          // 默认 8.0
    droplet_density: f32,        // 默认 1.0
    droplets_per_sec: f32,       // 计算值
    max_droplets_per_col: u8,    // 默认 3
    short_pct: f32,              // 默认 0.5
    die_early_pct: f32,          // 默认 0.333
    async_scroll: bool,
    full_width: bool,
    default_background: bool,

    // RNG
    rng: ThreadRng,

    // 时间
    last_glitch_time: Instant,
    next_glitch_time: Instant,
    last_spawn_time: Instant,
    pause_time: Option<Instant>,

    // 状态
    pause: bool,
    raining: bool,
    force_draw_everything: bool,
}

struct ColumnStatus {
    max_speed_pct: f32,  // 异步模式下该列速度因子
    num_droplets: u8,
    can_spawn: bool,
}
```

### `Cloud::rain()` 每帧逻辑

```rust
fn rain(&mut self) {
    if self.pause { return; }
    let now = Instant::now();
    self.spawn_droplets(now);        // 按时间积分为新雨滴分配列

    if self.force_draw_everything {
        // ratatui 的 Buffer 默认是空的，实际上每帧都会重绘
        // force_draw_everything 在 resize/切换主题后强制重绘所有非变化区域
    }

    let time_for_glitch = self.time_for_glitch(now);
    for droplet in &mut self.droplets {
        if !droplet.is_alive { continue; }
        droplet.advance(now, self.lines);
        if time_for_glitch {
            self.do_glitch(droplet);
        }
        // Draw 在 render() 阶段执行
        if !droplet.is_alive {
            let col = droplet.bound_col;
            self.col_stat[col as usize].num_droplets -= 1;
            if droplet.tail_put_line <= self.lines / 4 {
                self.col_stat[col as usize].can_spawn = true;
            }
        }
    }

    if !self.message.is_empty() {
        self.calc_message();  // 检查哪些消息位置被雨滴覆盖
    }

    if time_for_glitch {
        self.last_glitch_time = now;
        self.next_glitch_time = now + Duration::from_millis(self.rand_glitch_ms());
    }
    self.force_draw_everything = false;
}
```

## 4. Droplet 状态机

```rust
struct Droplet {
    is_alive: bool,
    is_head_crawling: bool,
    is_tail_crawling: bool,
    bound_col: u16,
    head_put_line: u16,      // Head 目标行
    head_cur_line: u16,      // Head 当前已绘制行（跳过优化用）
    tail_put_line: u16,      // Tail 目标行
    tail_cur_line: u16,      // Tail 当前已擦除行
    end_line: u16,           // Head 停止行
    char_pool_idx: u16,      // char_pool 索引起点
    length: u16,
    chars_per_sec: f32,
    last_time: Instant,
    head_stop_time: Instant,
    time_to_linger: Duration,
}
```

### `advance(now, screen_lines)` 状态转换

```
elapsed = (now - last_time).as_secs_f32()
chars_advanced = round(chars_per_sec * elapsed)

if is_head_crawling:
    head_put_line += chars_advanced
    head_put_line = min(head_put_line, end_line)
    if head_put_line == end_line:
        is_head_crawling = false
        if time_to_linger > 0:
            is_tail_crawling = false   // 暂停 tail
            head_stop_time = now

if is_tail_crawling && (head_put_line >= length || head_put_line >= end_line):
    if tail_put_line == 0xFFFF:        // 首次开始 tail
        tail_put_line = chars_advanced
    else:
        tail_put_line += chars_advanced
    tail_put_line = min(tail_put_line, end_line)
    // tail 越过屏幕 1/4 时允许该列重新生成雨滴
    if tail_cur_line <= screen_lines/4 && tail_put_line > screen_lines/4:
        pCloud.allow_spawn(bound_col)

if !is_tail_crawling && (now - head_stop_time) >= time_to_linger:
    is_tail_crawling = true             // linger 结束，tail 恢复

if tail_put_line == head_put_line:
    is_alive = false                    // tail 追上 head → 死亡

last_time = now
```

## 5. CharSource 抽象

```rust
pub trait CharSource {
    fn name(&self) -> &str;
    fn chars(&self) -> &[char];
    fn reload(&mut self) -> io::Result<()>;
}

// Builtin: 从 Unicode 范围构建
pub struct BuiltinChars {
    name: String,
    chars: Vec<char>,
}

// File: 从文件读取
pub struct FileCharSource {
    path: PathBuf,
    chars: Vec<char>,
}

// Stdin: 从管道读取
pub struct StdinCharSource {
    chars: Vec<char>,
}
```

`Cloud::init_chars()` 构建 `char_pool` 和 `glitch_pool`：
```rust
fn init_chars(&mut self) {
    let src_chars = self.char_source.chars();
    let n = src_chars.len();
    self.char_pool.resize(CHAR_POOL_SIZE, ' ');
    self.glitch_pool.resize(GLITCH_POOL_SIZE, ' ');
    for i in 0..CHAR_POOL_SIZE {
        self.char_pool[i] = src_chars[self.rng.gen_range(0..n)];
    }
    for i in 0..GLITCH_POOL_SIZE {
        self.glitch_pool[i] = src_chars[self.rng.gen_range(0..n)];
    }
}

fn get_char(&self, line: u16, cp_idx: u16) -> char {
    self.char_pool[((cp_idx as usize + line as usize) % CHAR_POOL_SIZE)]
}
```

## 6. 渲染：ratatui 自定义 Widget

实现一个 `RainWidget` 持有 `&Cloud` 引用：

```rust
struct RainWidget<'a> {
    cloud: &'a Cloud,
}

impl<'a> Widget for RainWidget<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        for droplet in &self.cloud.droplets {
            if !droplet.is_alive { continue; }
            let col = droplet.bound_col;
            if col >= area.width { continue; }

            let start = droplet.tail_cur_line.saturating_add(1);
            let end = droplet.head_put_line;

            for line in start..=end {
                if line >= area.height { break; }
                let ch = self.cloud.get_char(line, droplet.char_pool_idx);
                let attr = self.cloud.get_attr(line, col, ch, droplet.char_loc(line));
                let cell = buf.get_mut(col, line);
                cell.set_char(ch);
                cell.set_style(attr.style);
            }
        }
        // 消息渲染
        for msg_ch in &self.cloud.message {
            if msg_ch.draw && msg_ch.line < area.height && msg_ch.col < area.width {
                let cell = buf.get_mut(msg_ch.col, msg_ch.line);
                cell.set_char(msg_ch.val);
                cell.set_style(/* brightest color pair */);
            }
        }
    }
}
```

**与 ncurses 的关键差异**：ratatui 使用双缓冲，每帧自动 diff 增量刷新。不需要像原版那样手动跟踪 `_headCurLine`/`_tailCurLine` 做跳过优化 —— Buffer diff 自动处理。但保留这些字段供 `is_head_crawling` / `advance()` 状态判断。

## 7. 颜色系统

```rust
enum ColorTheme {
    Green, Green2, Green3, Yellow, Orange, Red, Blue, Cyan,
    Gold, Rainbow, Purple, Pink, Pink2, Vaporwave, Gray, User,
}

enum ColorMode { Mono, Color16, Color256, Truecolor }

struct ThemeColors {
    // 7 级渐变色，索引 0 不用，1=尾部最暗 → 7=头部最亮
    pairs: [Color; 8],  // pairs[1..=7] used
}
```

不同 ColorMode 下的 `Style` 构造：
- **Truecolor**: `Color::Rgb(r, g, b)` 直接用原始 RGB 值（0-1000 缩放到 0-255）
- **Color256**: `Color::Indexed(n)` 使用 256 色调色板索引
- **Color16**: `Color::from_u16()` 映射到终端 16 色
- **Mono**: 仅 bold 区分

## 8. 主循环与帧率控制

```rust
fn main_loop(cloud: &mut Cloud, fps: f64, terminal: &mut DefaultTerminal) -> io::Result<()> {
    let target_period = Duration::from_secs_f64(1.0 / fps);
    let mut prev_time = Instant::now();
    let mut prev_delay = Duration::from_millis(5);

    while cloud.raining() {
        // 非阻塞输入
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    handle_key(key, cloud);
                }
                Event::Resize(cols, rows) => {
                    cloud.reset_with_size(cols, rows);
                }
                _ => {}
            }
        }

        cloud.rain();
        terminal.draw(|f| f.render_widget(RainWidget { cloud }, f.area()))?;

        let now = Instant::now();
        let elapsed = now - prev_time;
        let calc_delay = if elapsed >= target_period {
            Duration::ZERO
        } else {
            target_period - elapsed
        };
        // 加权移动平均 (与原始 neo 一致)
        let cur_delay = (prev_delay * 7 + calc_delay) / 8;
        sleep(cur_delay);
        prev_time = now;
        prev_delay = cur_delay;
    }
    Ok(())
}
```

## 9. 输入处理

```rust
fn handle_key(key: KeyEvent, cloud: &mut Cloud) {
    match key.code {
        KeyCode::Char('q') | KeyCode::Esc => cloud.set_raining(false),
        KeyCode::Char(' ') => { cloud.reset(); cloud.force_draw_everything(); }
        KeyCode::Char('p') => cloud.toggle_pause(),
        KeyCode::Up => cloud.adjust_speed_up(),
        KeyCode::Down => cloud.adjust_speed_down(),
        KeyCode::Left => cloud.adjust_glitch_down(),
        KeyCode::Right => cloud.adjust_glitch_up(),
        KeyCode::Tab => cloud.toggle_shading_mode(),
        KeyCode::Char('a') => cloud.toggle_async(),
        KeyCode::Char('1') => cloud.set_color(ColorTheme::Green),
        // ... 2-0, !@#$% → 各颜色主题
        KeyCode::Char('-') => cloud.adjust_density_down(),
        KeyCode::Char('+') => cloud.adjust_density_up(),
        _ => {}
    }
}
```

## 10. CLI 设计（clap derive）

```rust
#[derive(Parser)]
#[command(name = "neo-rainst", version, about = "Matrix digital rain")]
struct Args {
    // 兼容原版
    #[arg(short = 'a', long = "async")]
    async_scroll: bool,
    #[arg(short = 'b', long = "bold", value_name = "NUM")]
    bold: Option<u8>,
    #[arg(short = 'c', long = "color", value_name = "COLOR")]
    color: Option<String>,
    #[arg(short = 'C', long = "colorfile", value_name = "FILE")]
    color_file: Option<PathBuf>,
    #[arg(short = 'D', long = "defaultbg")]
    default_bg: bool,
    #[arg(short = 'd', long = "density", value_name = "NUM")]
    density: Option<f32>,
    #[arg(short = 'f', long = "fps", value_name = "NUM", default_value = "60")]
    fps: f64,
    #[arg(short = 'F', long = "fullwidth")]
    full_width: bool,
    #[arg(short = 'g', long = "glitchms", value_name = "LOW,HIGH")]
    glitch_ms: Option<String>,
    #[arg(short = 'G', long = "glitchpct", value_name = "NUM")]
    glitch_pct: Option<f32>,
    #[arg(short = 'l', long = "lingerms", value_name = "LOW,HIGH")]
    linger_ms: Option<String>,
    #[arg(short = 'M', long = "shadingmode", value_name = "NUM")]
    shading_mode: Option<u8>,
    #[arg(short = 'm', long = "message", value_name = "STR")]
    message: Option<String>,
    #[arg(short = 'r', long = "rippct", value_name = "NUM")]
    rip_pct: Option<f32>,
    #[arg(short = 's', long = "screensaver")]
    screensaver: bool,
    #[arg(short = 'S', long = "speed", value_name = "NUM")]
    speed: Option<f32>,
    #[arg(long = "chars", value_name = "NUM1,NUM2")]
    chars: Option<String>,
    #[arg(long = "charset", value_name = "STR")]
    charset: Option<String>,
    #[arg(long = "colormode", value_name = "NUM")]
    color_mode: Option<u8>,
    #[arg(long = "maxdpc", value_name = "NUM")]
    max_dpc: Option<u8>,
    #[arg(long = "noglitch")]
    no_glitch: bool,
    #[arg(long = "shortpct", value_name = "NUM")]
    short_pct: Option<f32>,

    // 新增
    #[arg(long = "charset-file", value_name = "FILE")]
    charset_file: Option<PathBuf>,
    #[arg(long = "charset-stdin")]
    charset_stdin: bool,
    #[arg(long = "exit-after-secs", value_name = "SECS")]
    exit_after_secs: Option<f64>,
    #[arg(long = "exit-on-key")]
    exit_on_key: bool,
    #[arg(long = "fullscreen", default_value = "true")]
    fullscreen: bool,
    #[arg(long = "inline", value_name = "LINES")]
    inline: Option<u16>,
    #[arg(long = "config", value_name = "FILE")]
    config: Option<PathBuf>,
}
```

## 11. 配置文件格式

```toml
# ~/.config/neo-rainst/config.toml
[render]
fps = 60.0
color = "green"
shading-mode = 0      # 0=random, 1=distance-from-head
bold-mode = 1         # 0=off, 1=random, 2=all
full-width = false
default-bg = false

[charset]
source = "katakana"   # 或 "file:/path/to/chars.txt" 或 "stdin"

[rain]
speed = 8.0           # chars_per_sec
density = 1.0
async = false
max-droplets-per-col = 3
short-pct = 50.0
die-early-pct = 33.3

[glitch]
enabled = true
pct = 10.0
low-ms = 300
high-ms = 400

[linger]
low-ms = 1
high-ms = 3000

[exit]
mode = "on-key"       # "on-key" | "after-secs"
secs = 10.0

[claude]
enabled = false
session-file = "/tmp/neo-rainst-claude-session.txt"
max-chars = 10000
```

## 12. 与原始 neo 的差异说明

| 原始 neo | neo-rainst |
|----------|------------|
| ncurses `mvadd_wch` | ratatui `Buffer::get_mut(x,y)` |
| ncurses color pair | ratatui `Style` + `Color` |
| `getopt_long` | `clap` derive |
| `autotools` | `cargo` |
| 颜色文件 `neo_color_version` | 兼容解析 + TOML 替代 |
| `--chars` hex 范围 | 兼容 + `--charset-file` / `--charset-stdin` |
| `-p` profiling 模式 | 不实现 |
