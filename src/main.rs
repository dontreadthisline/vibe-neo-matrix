mod char_source;
mod cloud;
mod color;
mod config;
mod droplet;
mod render;

use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::{DefaultTerminal, TerminalOptions, Viewport};

use char_source::{parse_chars_arg, BuiltinChars, CharSource, ClaudeCharSource, FileCharSource, StdinCharSource};
use cloud::Cloud;
use color::{ColorMode, ColorTheme, ShadingMode, BoldMode, detect_color_mode};
use config::{load_config, Config};
use render::{RainWidget, StatusBar};

/// Matrix digital rain simulation — Rust port of neo
#[derive(Parser, Debug)]
#[command(name = "neo-rainst", version, about = "Simulate the digital rain from \"The Matrix\"")]
struct Args {
    // Compatible with original neo
    #[arg(short = 'a', long = "async")] async_scroll: bool,
    #[arg(short = 'b', long = "bold", value_name = "NUM")] bold: Option<u8>,
    #[arg(short = 'c', long = "color", value_name = "COLOR")] color: Option<String>,
    #[arg(short = 'C', long = "colorfile", value_name = "FILE")] color_file: Option<PathBuf>,
    #[arg(short = 'D', long = "defaultbg")] default_bg: bool,
    #[arg(short = 'd', long = "density", value_name = "NUM")] density: Option<f32>,
    #[arg(short = 'f', long = "fps", value_name = "NUM", default_value = "60")] fps: f64,
    #[arg(short = 'F', long = "fullwidth")] full_width: bool,
    #[arg(short = 'g', long = "glitchms", value_name = "LOW,HIGH")] glitch_ms: Option<String>,
    #[arg(short = 'G', long = "glitchpct", value_name = "NUM")] glitch_pct: Option<f32>,
    #[arg(short = 'l', long = "lingerms", value_name = "LOW,HIGH")] linger_ms: Option<String>,
    #[arg(short = 'M', long = "shadingmode", value_name = "NUM")] shading_mode: Option<u8>,
    #[arg(short = 'm', long = "message", value_name = "STR")] message: Option<String>,
    #[arg(short = 'r', long = "rippct", value_name = "NUM")] rip_pct: Option<f32>,
    #[arg(short = 's', long = "screensaver")] screensaver: bool,
    #[arg(short = 'S', long = "speed", value_name = "NUM")] speed: Option<f32>,
    #[arg(long = "chars", value_name = "RANGE")] chars: Option<String>,
    #[arg(long = "charset", value_name = "STR")] charset: Option<String>,
    #[arg(long = "colormode", value_name = "NUM")] color_mode_arg: Option<u16>,
    #[arg(long = "maxdpc", value_name = "NUM")] maxdpc: Option<u8>,
    #[arg(long = "noglitch")] no_glitch: bool,
    #[arg(long = "shortpct", value_name = "NUM")] short_pct: Option<f32>,

    // New additions
    #[arg(long = "charset-file", value_name = "FILE")] charset_file: Option<PathBuf>,
    #[arg(long = "charset-stdin")] charset_stdin: bool,
    #[arg(long = "exit-after-secs", value_name = "SECS")] exit_after_secs: Option<f64>,
    #[arg(long = "exit-on-key")] exit_on_key: bool,
    #[arg(long = "inline", value_name = "LINES")] inline: Option<u16>,
    #[arg(long = "config", value_name = "FILE")] config_file: Option<PathBuf>,
    #[arg(long = "status")] show_status: bool,
}

fn main() -> io::Result<()> {
    let args = Args::parse();

    // Load config
    let cfg = load_config(args.config_file.as_ref());

    // Build CharSource.
    // Priority: CLI --chars > --charset-stdin > --charset-file > config charset.source > claude.enabled
    let (char_source, transcript_dir): (Box<dyn CharSource>, Option<std::path::PathBuf>) =
        if let Some(ref chars_arg) = args.chars {
        match parse_chars_arg(chars_arg) {
            Ok(pairs) => {
                let cs = BuiltinChars::from_unicode_pairs(&pairs);
                (Box::new(cs), None)
            }
            Err(e) => {
                eprintln!("Warning: failed to parse --chars argument: {}", e);
                (Box::new(BuiltinChars::from_charset_name("katakana")), None)
            }
        }
    } else if args.charset_stdin {
        (Box::new(StdinCharSource::new()?), None)
    } else if let Some(ref path) = args.charset_file {
        (Box::new(FileCharSource::new(&path.to_string_lossy())?), None)
    } else {
        let charset_name = args.charset.as_deref()
            .unwrap_or(&cfg.charset.source);
        match charset_name {
            s if s.starts_with("file:") => {
                (Box::new(FileCharSource::new(&s[5..])?), None)
            }
            "stdin" => (Box::new(StdinCharSource::new()?), None),
            "claude" => {
                let dir = resolve_transcript_dir(&cfg.claude.transcript_dir);
                let cs = Box::new(ClaudeCharSource::new(&dir, cfg.claude.max_chars)?);
                (cs, Some(dir))
            }
            name => {
                // If claude.enabled is true, override to claude mode
                if cfg.claude.enabled {
                    let dir = resolve_transcript_dir(&cfg.claude.transcript_dir);
                    let cs = Box::new(ClaudeCharSource::new(&dir, cfg.claude.max_chars)?);
                    (cs, Some(dir))
                } else {
                    (Box::new(BuiltinChars::from_charset_name(name)), None)
                }
            }
        }
    };

    // Fullscreen mode (alternate screen) by default: use ratatui::init()
    // which explicitly enters the alternate screen buffer.
    // Pass --inline <LINES> for inline mode (no alternate screen).
    let terminal = if let Some(lines) = args.inline {
        ratatui::init_with_options(TerminalOptions {
            viewport: Viewport::Inline(lines),
        })
    } else {
        ratatui::init()
    };
    let result = run_app(terminal, args, cfg, char_source, transcript_dir);
    ratatui::restore();
    result
}

fn run_app(
    mut terminal: DefaultTerminal,
    args: Args,
    cfg: Config,
    char_source: Box<dyn CharSource>,
    transcript_dir: Option<std::path::PathBuf>,
) -> io::Result<()> {
    let color_mode = if let Some(cm) = args.color_mode_arg {
        match cm {
            0 => ColorMode::Mono,
            16 => ColorMode::Color16,
            256 => ColorMode::Color256,
            32 => ColorMode::Truecolor,
            _ => detect_color_mode(),
        }
    } else {
        detect_color_mode()
    };

    let (cols, lines) = {
        let size = terminal.size()?;
        (size.width, size.height)
    };

    let mut cloud = Cloud::new(lines, cols, color_mode, char_source);

    // Apply CLI args (overriding config defaults)
    apply_args(&mut cloud, &args, &cfg);

    // Store char source name and color theme for status bar display
    let char_source_name = cloud.char_source_name().to_string();
    let color_theme = format!("{:?}", cloud.color_theme);


    // Frame rate control
    let fps = args.fps;
    let target_period = Duration::from_secs_f64(1.0 / fps);
    let mut prev_time = Instant::now();
    let mut prev_delay = Duration::from_millis(5);

    // Exit-after-secs timer
    let start_time = Instant::now();
    let exit_after = args.exit_after_secs.or_else(|| {
        if cfg.exit.mode == "after-secs" && cfg.exit.secs > 0.0 {
            Some(cfg.exit.secs)
        } else {
            None
        }
    });
    let exit_on_any_key = args.exit_on_key
        || args.screensaver
        || cfg.exit.mode == "on-key";

    // Claude mode: poll transcript dir for new sessions (every ~1s)
    let mut claude_check_time = Instant::now();

    // Main loop
    while cloud.raining {
        // Handle input (non-blocking)
        while event::poll(Duration::ZERO)? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    // In exit-on-any-key mode, quit immediately
                    if exit_on_any_key {
                        cloud.set_raining(false);
                        break;
                    }
                    if handle_key(key.code, &mut cloud, &args) {
                        break;
                    }
                }
                Event::Resize(w, h) => {
                    cloud.reset_with_size(w, h);
                    let _ = terminal.resize(Rect::new(0, 0, w, h));
                }
                _ => {}
            }
        }

        // Check exit-after-secs
        if let Some(secs) = exit_after {
            if start_time.elapsed().as_secs_f64() >= secs {
                cloud.set_raining(false);
                break;
            }
        }

        // Advance simulation
        cloud.rain();

        // Poll Claude transcript dir for new/changed sessions (every ~1s)
        if transcript_dir.is_some()
            && claude_check_time.elapsed() >= Duration::from_secs(1)
        {
            claude_check_time = Instant::now();
            let _ = cloud.reload_chars();
        }

        // Render
        let now = Instant::now();
        let show_status = args.show_status || cfg.render.show_status;
        terminal.draw(|frame| {
            let area = frame.area();
            if show_status && area.height > 1 {
                let [main_area, status_area] = Layout::vertical([
                    Constraint::Fill(1),
                    Constraint::Length(1),
                ]).areas(area);
                frame.render_widget(
                    RainWidget { cloud: &cloud, now, show_message: true },
                    main_area,
                );
                frame.render_widget(
                    StatusBar {
                        chars_per_sec: cloud.chars_per_sec,
                        droplet_density: cloud.droplet_density,
                        glitch_pct: cloud.glitch_pct,
                        char_source_name: char_source_name.as_str(),
                        color_theme: color_theme.as_str(),
                        pause: cloud.pause,
                    },
                    status_area,
                );
            } else {
                frame.render_widget(
                    RainWidget { cloud: &cloud, now, show_message: true },
                    area,
                );
            }
        })?;

        // Frame rate control with EMA smoothing (matches original neo)
        let elapsed = prev_time.elapsed();
        let calc_delay = if elapsed >= target_period {
            Duration::ZERO
        } else {
            target_period - elapsed
        };
        let cur_delay = (prev_delay * 7 + calc_delay) / 8;
        std::thread::sleep(cur_delay);
        prev_time = Instant::now();
        prev_delay = cur_delay;
    }

    // Clear screen before exit using Color::Reset so the terminal's
    // default background is preserved after ratatui::restore().
    terminal.draw(|frame| {
        let area = frame.area();
        let clear_style = ratatui::style::Style::default().bg(ratatui::style::Color::Reset);
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let cell = &mut frame.buffer_mut()[(x, y)];
                cell.set_char(' ');
                cell.set_style(clear_style);
            }
        }
    })?;

    Ok(())
}

/// Apply CLI args and config values to the Cloud.
fn apply_args(cloud: &mut Cloud, args: &Args, cfg: &Config) {
    // Speed
    if let Some(s) = args.speed {
        cloud.set_chars_per_sec(s);
    } else {
        cloud.set_chars_per_sec(cfg.rain.speed);
    }

    // Density
    if let Some(d) = args.density {
        cloud.set_droplet_density(d);
    } else {
        cloud.set_droplet_density(cfg.rain.density);
    }

    // Color
    if let Some(ref c) = args.color {
        cloud.set_color(ColorTheme::from_name(c));
    } else if !cfg.render.color.is_empty() {
        cloud.set_color(ColorTheme::from_name(&cfg.render.color));
    }

    // Shading mode
    if let Some(m) = args.shading_mode {
        cloud.set_shading_mode(match m {
            0 => ShadingMode::Random,
            _ => ShadingMode::DistanceFromHead,
        });
    } else {
        cloud.set_shading_mode(match cfg.render.shading_mode {
            0 => ShadingMode::Random,
            _ => ShadingMode::DistanceFromHead,
        });
    }

    // Bold mode
    if let Some(b) = args.bold {
        cloud.set_bold_mode(match b {
            0 => BoldMode::Off,
            2 => BoldMode::All,
            _ => BoldMode::Random,
        });
    } else {
        cloud.set_bold_mode(match cfg.render.bold_mode {
            0 => BoldMode::Off,
            2 => BoldMode::All,
            _ => BoldMode::Random,
        });
    }

    // Glitch
    if args.no_glitch || !cfg.glitch.enabled {
        cloud.set_glitchy(false);
    } else {
        if let Some(p) = args.glitch_pct {
            cloud.set_glitch_pct(p / 100.0);
        } else {
            cloud.set_glitch_pct(cfg.glitch.pct / 100.0);
        }
        if let Some(ref ms_str) = args.glitch_ms {
            if let Some((lo, hi)) = parse_pair(ms_str) {
                cloud.set_glitch_times(lo, hi);
            }
        } else {
            cloud.set_glitch_times(cfg.glitch.low_ms, cfg.glitch.high_ms);
        }
    }

    // Linger times
    if let Some(ref ms_str) = args.linger_ms {
        if let Some((lo, hi)) = parse_pair(ms_str) {
            cloud.set_linger_times(lo, hi);
        }
    } else {
        cloud.set_linger_times(cfg.linger.low_ms, cfg.linger.high_ms);
    }

    // Short pct
    if let Some(p) = args.short_pct {
        cloud.set_short_pct(p / 100.0);
    } else {
        cloud.set_short_pct(cfg.rain.short_pct / 100.0);
    }

    // Die early pct
    if let Some(p) = args.rip_pct {
        cloud.set_die_early_pct(p / 100.0);
    } else {
        cloud.set_die_early_pct(cfg.rain.die_early_pct / 100.0);
    }

    // Max droplets per column
    if let Some(m) = args.maxdpc {
        cloud.set_max_droplets_per_col(m);
    } else {
        cloud.set_max_droplets_per_col(cfg.rain.max_droplets_per_col);
    }

    // Async
    if args.async_scroll || cfg.rain.async_scroll {
        cloud.set_async(true);
    }

    // Full width
    if args.full_width || cfg.render.full_width {
        cloud.full_width = true;
    }

    // Default background
    if args.default_bg || cfg.render.default_bg {
        cloud.default_background = true;
    }

    // Screensaver mode
    if args.screensaver {
        cloud.raining = true;
    }

    // Message
    if let Some(ref msg) = args.message {
        cloud.set_message(msg);
    }
}

/// Resolve the transcript directory for Claude mode.
/// If config specifies a path, use it; otherwise auto-derive from CWD.
fn resolve_transcript_dir(config_dir: &str) -> std::path::PathBuf {
    if !config_dir.is_empty() {
        return std::path::PathBuf::from(config_dir);
    }
    ClaudeCharSource::transcript_dir_from_cwd()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
}

/// Parse "low,high" pair from a string like "300,400"
fn parse_pair(s: &str) -> Option<(u16, u16)> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 2 {
        return None;
    }
    let lo = parts[0].trim().parse().ok()?;
    let hi = parts[1].trim().parse().ok()?;
    Some((lo, hi))
}

/// Handle a key event. Returns true if the app should exit.
fn handle_key(code: KeyCode, cloud: &mut Cloud, args: &Args) -> bool {
    match code {
        KeyCode::Char('q') | KeyCode::Esc => {
            if args.screensaver {
                return true;
            }
            cloud.set_raining(false);
            return true;
        }
        KeyCode::Char(' ') => {
            cloud.reset();
            cloud.force_draw_everything = true;
        }
        KeyCode::Char('p') => {
            cloud.toggle_pause();
        }
        KeyCode::Up => {
            let cps = cloud.chars_per_sec;
            let new_cps = if cps <= 0.5 { cps * 2.0 } else { cps + 1.0 };
            cloud.set_chars_per_sec(new_cps.min(1000.0));
        }
        KeyCode::Down => {
            let cps = cloud.chars_per_sec;
            let new_cps = if cps <= 1.0 { cps / 2.0 } else { cps - 1.0 };
            cloud.set_chars_per_sec(new_cps.max(0.1));
        }
        KeyCode::Left => {
            if cloud.glitchy {
                let gpct = cloud.glitch_pct - 0.05;
                cloud.set_glitch_pct(gpct.max(0.0));
            }
        }
        KeyCode::Right => {
            if cloud.glitchy {
                let gpct = cloud.glitch_pct + 0.05;
                cloud.set_glitch_pct(gpct.min(1.0));
            }
        }
        KeyCode::Tab => {
            match cloud.shading_mode {
                ShadingMode::Random => cloud.set_shading_mode(ShadingMode::DistanceFromHead),
                ShadingMode::DistanceFromHead => cloud.set_shading_mode(ShadingMode::Random),
            }
        }
        KeyCode::Char('a') => {
            cloud.set_async(!cloud.async_scroll);
        }
        KeyCode::Char('1') => cloud.set_color(ColorTheme::Green),
        KeyCode::Char('2') => cloud.set_color(ColorTheme::Green2),
        KeyCode::Char('3') => cloud.set_color(ColorTheme::Green3),
        KeyCode::Char('4') => cloud.set_color(ColorTheme::Gold),
        KeyCode::Char('5') => cloud.set_color(ColorTheme::Pink2),
        KeyCode::Char('6') => cloud.set_color(ColorTheme::Red),
        KeyCode::Char('7') => cloud.set_color(ColorTheme::Blue),
        KeyCode::Char('8') => cloud.set_color(ColorTheme::Cyan),
        KeyCode::Char('9') => cloud.set_color(ColorTheme::Purple),
        KeyCode::Char('0') => cloud.set_color(ColorTheme::Gray),
        KeyCode::Char('!') => cloud.set_color(ColorTheme::Rainbow),
        KeyCode::Char('@') => cloud.set_color(ColorTheme::Yellow),
        KeyCode::Char('#') => cloud.set_color(ColorTheme::Orange),
        KeyCode::Char('$') => cloud.set_color(ColorTheme::Pink),
        KeyCode::Char('%') => cloud.set_color(ColorTheme::Vaporwave),
        KeyCode::Char('-') => {
            let density = cloud.droplet_density - 0.25;
            cloud.set_droplet_density(density.max(0.01));
        }
        KeyCode::Char('+') | KeyCode::Char('=') => {
            let density = cloud.droplet_density + 0.25;
            cloud.set_droplet_density(density.min(5.0));
        }
        _ => {}
    }
    false
}
