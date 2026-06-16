use std::io::{self, Read};
use std::num::ParseIntError;

/// 字符源 trait —— 统一内置/文件/stdin/Claude 等字符来源
pub trait CharSource {
    fn name(&self) -> &str;
    fn chars(&self) -> &[char];
    fn reload(&mut self) -> io::Result<()> {
        Ok(())
    }
}

// ============================================================
// BuiltinChars — 从 Unicode 范围构建的内置字符集
// ============================================================

pub struct BuiltinChars {
    name: String,
    chars: Vec<char>,
}

impl BuiltinChars {
    pub fn new(name: &str, ranges: &[(u32, u32)]) -> Self {
        let mut chars = Vec::new();
        for &(start, end) in ranges {
            for cp in start..=end {
                if let Some(c) = char::from_u32(cp) {
                    chars.push(c);
                }
            }
        }
        BuiltinChars {
            name: name.to_string(),
            chars,
        }
    }

    /// 从预定义的字符集名称构建
    pub fn from_charset_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "ascii" | "default" => Self::new("ascii", &[
                (0x41, 0x5A),  // A-Z
                (0x61, 0x7A),  // a-z
                (0x30, 0x39),  // 0-9
            ]),
            "extended" | "extended_default" => Self::new("extended", &[
                (0x41, 0x5A),
                (0x61, 0x7A),
                (0x30, 0x39),
                (0xFF64, 0xFF9F),  // half-width katakana
            ]),
            "english" | "letters" => Self::new("english", &[
                (0x41, 0x5A),
                (0x61, 0x7A),
            ]),
            "digits" | "dec" | "decimal" => Self::new("digits", &[
                (0x30, 0x39),
            ]),
            "punc" | "punctuation" => Self::new("punctuation", &[
                (0x21, 0x2F),
                (0x3A, 0x40),
                (0x5B, 0x60),
                (0x7B, 0x7E),
            ]),
            "bin" | "binary" => Self::new("binary", &[
                (0x30, 0x31),  // 0-1
            ]),
            "hex" | "hexadecimal" => Self::new("hex", &[
                (0x30, 0x39),  // 0-9
                (0x41, 0x46),  // A-F
            ]),
            "katakana" => Self::new("katakana", &[
                (0xFF64, 0xFF9F),  // half-width katakana
            ]),
            "greek" => Self::new("greek", &[
                (0x0370, 0x03FF),
            ]),
            "cyrillic" => Self::new("cyrillic", &[
                (0x0410, 0x044F),
            ]),
            "arabic" => Self::new("arabic", &[
                (0x0627, 0x0649),
            ]),
            "hebrew" => Self::new("hebrew", &[
                (0x0590, 0x05FF),
                (0xFB1D, 0xFB4F),
            ]),
            "devanagari" => Self::new("devanagari", &[
                (0x0900, 0x097F),
            ]),
            "braille" => Self::new("braille", &[
                (0x2800, 0x28FF),
            ]),
            "runic" => Self::new("runic", &[
                (0x16A0, 0x16FF),
            ]),
            _ => {
                // fallback: katakana (matching original neo default)
                Self::new("katakana", &[(0xFF64, 0xFF9F)])
            }
        }
    }

    /// 从 Unicode 码点对(pair)列表构建，兼容原版 --chars 参数格式
    pub fn from_unicode_pairs(pairs: &[(u32, u32)]) -> Self {
        Self::new("user-unicode", pairs)
    }
}

/// Parse a `--chars` argument string into Unicode code-point range pairs.
/// Format: even number of hex values separated by commas, like `0x3040,0x309F`.
/// Original neo only supports hex literals; we accept decimal and octal too.
pub fn parse_chars_arg(s: &str) -> Result<Vec<(u32, u32)>, ParseIntError> {
    let mut pairs = Vec::new();
    let parts: Vec<&str> = s.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()).collect();
    let mut i = 0;
    while i + 1 < parts.len() {
        let lo = parse_code_point(parts[i])?;
        let hi = parse_code_point(parts[i + 1])?;
        pairs.push((lo, hi));
        i += 2;
    }
    Ok(pairs)
}

fn parse_code_point(s: &str) -> Result<u32, ParseIntError> {
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16)
    } else if let Some(oct) = s.strip_prefix("0o").or_else(|| s.strip_prefix("0O")) {
        u32::from_str_radix(oct, 8)
    } else {
        s.parse::<u32>()
    }
}

impl CharSource for BuiltinChars {
    fn name(&self) -> &str {
        &self.name
    }
    fn chars(&self) -> &[char] {
        &self.chars
    }
}

// ============================================================
// FileCharSource — 从文件读取字符
// ============================================================

pub struct FileCharSource {
    path: String,
    chars: Vec<char>,
}

impl FileCharSource {
    pub fn new(path: &str) -> io::Result<Self> {
        let mut source = FileCharSource {
            path: path.to_string(),
            chars: Vec::new(),
        };
        source.reload()?;
        Ok(source)
    }

    fn load_from_reader<R: Read>(&mut self, reader: R) -> io::Result<()> {
        let mut content = String::new();
        let mut buf_reader = std::io::BufReader::new(reader);
        buf_reader.read_to_string(&mut content)?;
        self.chars = content.chars()
            .filter(|c| !c.is_whitespace() && !c.is_control())
            .collect();
        Ok(())
    }
}

impl CharSource for FileCharSource {
    fn name(&self) -> &str {
        &self.path
    }
    fn chars(&self) -> &[char] {
        &self.chars
    }
    fn reload(&mut self) -> io::Result<()> {
        let file = std::fs::File::open(&self.path)?;
        self.load_from_reader(file)
    }
}

// ============================================================
// StdinCharSource — 从管道/stdin 读取字符
// ============================================================

pub struct StdinCharSource {
    chars: Vec<char>,
}

impl StdinCharSource {
    pub fn new() -> io::Result<Self> {
        let mut source = StdinCharSource { chars: Vec::new() };
        source.reload()?;
        Ok(source)
    }
}

impl CharSource for StdinCharSource {
    fn name(&self) -> &str {
        "stdin"
    }
    fn chars(&self) -> &[char] {
        &self.chars
    }
    fn reload(&mut self) -> io::Result<()> {
        let stdin = io::stdin();
        let mut content = String::new();
        stdin.lock().read_to_string(&mut content)?;
        self.chars = content.chars()
            .filter(|c| !c.is_whitespace() && !c.is_control())
            .collect();
        Ok(())
    }
}

// ============================================================
// ClaudeCharSource — 自动发现并解析 Claude Code transcript JSONL
// ============================================================
///
/// 从 `~/.claude/projects/<project-dir>/` 目录中找到最近修改的 `.jsonl`
/// transcript 文件，提取 user/assistant 消息中的纯文本作为字符源。
/// 无需依赖 SessionEnd hook —— 每次 reload 时自动重新扫描目录。
///
pub struct ClaudeCharSource {
    transcript_dir: std::path::PathBuf,
    chars: Vec<char>,
    max_chars: usize,
    /// 上次 reload 时已加载的文件 mtime，避免重复解析同一文件
    last_mtime: Option<std::time::SystemTime>,
}

impl ClaudeCharSource {
    /// `transcript_dir` — Claude Code 项目 transcript 目录，例如
    /// `~/.claude/projects/-home-zsl-projects-kinds_exer-vibe-demo-vibe-neo-matrix/`
    pub fn new(transcript_dir: &std::path::Path, max_chars: usize) -> io::Result<Self> {
        let mut source = ClaudeCharSource {
            transcript_dir: transcript_dir.to_path_buf(),
            chars: Vec::new(),
            max_chars,
            last_mtime: None,
        };
        source.reload()?;
        Ok(source)
    }

    /// 根据 CWD 自动推导 Claude Code transcript 目录路径。
    /// 规律: `/a/b/c` → `~/.claude/projects/-a-b-c/`
    pub fn transcript_dir_from_cwd() -> Option<std::path::PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        let abs = cwd.canonicalize().ok()?;
        let dir_name = dir_name_from_path(&abs.to_string_lossy());
        let home = dirs_fallback();
        Some(home.join(".claude").join("projects").join(dir_name))
    }
}

/// 将绝对路径转换为 Claude Code project 目录名。
/// Claude Code 会把 `/` 和 `_` 统一转为 `-`。
/// `/home/user/my_proj` → `-home-user-my-proj`
fn dir_name_from_path(abs_path: &str) -> String {
    format!("-{}", abs_path.trim_start_matches('/').replace(['/', '_'], "-"))
}

/// 在不引入 dirs 依赖的情况下获取 home 目录
fn dirs_fallback() -> std::path::PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home);
    }
    // fallback: /root for uid 0, otherwise /tmp
    std::path::PathBuf::from("/tmp")
}

/// 从 JSONL transcript 行中提取纯文本字符
fn extract_text_from_entry(entry: &serde_json::Value) -> String {
    let entry_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let mut text = String::new();

    match entry_type {
        "user" => {
            // user message: message.content is a string
            if let Some(content) = entry.get("message").and_then(|m| m.get("content")) {
                if let Some(s) = content.as_str() {
                    text.push_str(s);
                }
            }
            // Also check top-level content
            if let Some(content) = entry.get("content").and_then(|c| c.as_str()) {
                text.push_str(content);
            }
        }
        "assistant" => {
            // assistant message: message.content is [{type: "text", text: "..."}, ...]
            if let Some(blocks) = entry.get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
            {
                for block in blocks {
                    if let Some(t) = block.get("type").and_then(|v| v.as_str()) {
                        if t == "text" {
                            if let Some(txt) = block.get("text").and_then(|v| v.as_str()) {
                                text.push_str(txt);
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    text
}

impl CharSource for ClaudeCharSource {
    fn name(&self) -> &str {
        "claude-session"
    }
    fn chars(&self) -> &[char] {
        &self.chars
    }
    fn reload(&mut self) -> io::Result<()> {
        // 1. 找到最近修改的 .jsonl 文件
        let mut latest: Option<(std::time::SystemTime, std::path::PathBuf)> = None;
        let dir = match std::fs::read_dir(&self.transcript_dir) {
            Ok(d) => d,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.chars.clear();
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        for entry in dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            if let Ok(meta) = path.metadata() {
                if let Ok(mtime) = meta.modified() {
                    match &latest {
                        Some((t, _)) if mtime <= *t => {}
                        _ => latest = Some((mtime, path)),
                    }
                }
            }
        }

        // 2. 如果文件没有变化，跳过
        let (mtime, transcript_path) = match latest {
            Some(v) => v,
            None => {
                self.chars.clear();
                return Ok(());
            }
        };
        if self.last_mtime == Some(mtime) {
            return Ok(());
        }
        self.last_mtime = Some(mtime);

        // 3. 解析 JSONL，提取文本
        let content = std::fs::read_to_string(&transcript_path)?;
        let mut all_text = String::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                all_text.push_str(&extract_text_from_entry(&entry));
            }
        }

        // 4. 截取最后 max_chars 个非控制字符
        let chars: Vec<char> = all_text.chars()
            .filter(|c| !c.is_control())
            .collect();
        let start = if chars.len() > self.max_chars {
            chars.len() - self.max_chars
        } else {
            0
        };
        self.chars = chars[start..].to_vec();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_katakana_charset() {
        let src = BuiltinChars::from_charset_name("katakana");
        assert!(!src.chars().is_empty());
        // 半角片假名范围 0xFF64-0xFF9F
        assert!(src.chars().contains(&'\u{FF64}'));
        assert!(src.chars().contains(&'\u{FF9F}'));
    }

    #[test]
    fn test_hex_charset() {
        let src = BuiltinChars::from_charset_name("hex");
        assert_eq!(src.chars().len(), 16); // 0-9 A-F
        assert!(src.chars().contains(&'0'));
        assert!(src.chars().contains(&'F'));
    }

    #[test]
    fn test_binary_charset() {
        let src = BuiltinChars::from_charset_name("binary");
        assert_eq!(src.chars(), &['0', '1']);
    }

    #[test]
    fn test_file_char_source() {
        use std::io::Cursor;
        let data = "ABC 123 !@#";
        let mut source = FileCharSource {
            path: "test".into(),
            chars: Vec::new(),
        };
        source.load_from_reader(Cursor::new(data)).unwrap();
        // 非空白字符: A,B,C,1,2,3,!,@,#
        assert_eq!(source.chars().len(), 9);
    }

    // ============================================================
    // transcript_dir_from_cwd() path normalisation tests
    // ============================================================

    /// Verify that `_` in CWD is normalised to `-` in the transcript dir name.
    /// Claude Code replaces both `/` and `_` with `-`.
    #[test]
    fn test_transcript_dir_from_cwd_normalises_underscore() {
        let dir = super::dir_name_from_path("/home/zsl/projects/kinds_exer/vibe-neo-matrix");
        // Expect all / replaced, and _ replaced by -
        assert_eq!(dir, "-home-zsl-projects-kinds-exer-vibe-neo-matrix");
    }

    #[test]
    fn test_transcript_dir_from_cwd_simple_path() {
        let dir = super::dir_name_from_path("/home/user/my_project");
        assert_eq!(dir, "-home-user-my-project");
    }

    #[test]
    fn test_transcript_dir_from_cwd_root() {
        let dir = super::dir_name_from_path("/");
        assert_eq!(dir, "-");
    }

    // ============================================================
    // extract_text_from_entry() tests
    // ============================================================

    #[test]
    fn test_extract_text_from_user_entry() {
        let json = serde_json::json!({
            "type": "user",
            "message": {
                "content": "Hello, how do I fix this bug?"
            }
        });
        let text = extract_text_from_entry(&json);
        assert_eq!(text, "Hello, how do I fix this bug?");
    }

    #[test]
    fn test_extract_text_from_assistant_entry() {
        let json = serde_json::json!({
            "type": "assistant",
            "message": {
                "content": [
                    {"type": "text", "text": "Here is the fix:"},
                    {"type": "text", "text": "  use std::io;"}
                ]
            }
        });
        let text = extract_text_from_entry(&json);
        assert_eq!(text, "Here is the fix:  use std::io;");
    }

    #[test]
    fn test_extract_text_from_user_entry_top_level_content() {
        let json = serde_json::json!({
            "type": "user",
            "content": "top level prompt text"
        });
        let text = extract_text_from_entry(&json);
        assert_eq!(text, "top level prompt text");
    }

    #[test]
    fn test_extract_text_skips_system_types() {
        // attachment, file-history-snapshot etc should return empty
        for sys_type in &["attachment", "file-history-snapshot", "mode", "system"] {
            let json = serde_json::json!({
                "type": sys_type,
                "message": { "content": "should be ignored" }
            });
            let text = extract_text_from_entry(&json);
            assert!(text.is_empty(), "type={} should be skipped, got '{}'", sys_type, text);
        }
    }

    #[test]
    fn test_extract_text_empty_entry() {
        let json = serde_json::json!({});
        let text = extract_text_from_entry(&json);
        assert!(text.is_empty());
    }

    // ============================================================
    // ClaudeCharSource integration test (temp dir + jsonl file)
    // ============================================================

    #[test]
    fn test_claude_char_source_reload() {
        let tmp = std::env::temp_dir().join("neo-rainst-test-claude-source");
        let _ = std::fs::create_dir_all(&tmp);

        let jsonl_path = tmp.join("test-session.jsonl");
        let content = r#"{"type":"user","message":{"content":"help me with rust"}}
{"type":"assistant","message":{"content":[{"type":"text","text":"use std::io::Result;"}]}}
{"type":"system","message":{"content":"should be ignored"}}
{"type":"user","message":{"content":"thanks!"}}
"#;
        std::fs::write(&jsonl_path, content).unwrap();

        let mut source = ClaudeCharSource {
            transcript_dir: tmp.clone(),
            chars: Vec::new(),
            max_chars: 10000,
            last_mtime: None,
        };
        source.reload().unwrap();

        // "help me with rust" + "use std::io::Result;" + "thanks!" — no whitespace, no control chars
        let chars_str: String = source.chars.iter().collect();
        assert!(chars_str.contains("help me with rust"));
        assert!(chars_str.contains("use std::io::Result;"));
        assert!(chars_str.contains("thanks!"));
        assert!(!chars_str.contains("should be ignored"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_claude_char_source_max_chars_truncation() {
        let tmp = std::env::temp_dir().join("neo-rainst-test-claude-trunc");
        let _ = std::fs::create_dir_all(&tmp);

        let jsonl_path = tmp.join("test-session.jsonl");
        // Write 200 characters (letter 'A' repeated)
        let long_text = "A".repeat(200);
        let content = format!(r#"{{"type":"user","message":{{"content":"{}"}}}}"#, long_text);
        std::fs::write(&jsonl_path, content).unwrap();

        let mut source = ClaudeCharSource {
            transcript_dir: tmp.clone(),
            chars: Vec::new(),
            max_chars: 100, // only keep last 100 chars
            last_mtime: None,
        };
        source.reload().unwrap();
        assert_eq!(source.chars.len(), 100);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_claude_char_source_empty_dir() {
        let tmp = std::env::temp_dir().join("neo-rainst-test-empty");
        let _ = std::fs::create_dir_all(&tmp);

        let mut source = ClaudeCharSource {
            transcript_dir: tmp.clone(),
            chars: vec!['x'],
            max_chars: 100,
            last_mtime: None,
        };
        // No jsonl files — should clear chars
        source.reload().unwrap();
        assert!(source.chars.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_claude_char_source_nonexistent_dir() {
        let tmp = std::env::temp_dir().join("neo-rainst-test-nonexistent-12345");

        let mut source = ClaudeCharSource {
            transcript_dir: tmp.clone(),
            chars: vec!['x'],
            max_chars: 100,
            last_mtime: None,
        };
        // Dir doesn't exist — should clear chars without error
        source.reload().unwrap();
        assert!(source.chars.is_empty());
    }

    #[test]
    fn test_claude_char_source_no_duplicate_reload() {
        let tmp = std::env::temp_dir().join("neo-rainst-test-nodup");
        let _ = std::fs::create_dir_all(&tmp);

        let jsonl_path = tmp.join("test-session.jsonl");
        std::fs::write(&jsonl_path, r#"{"type":"user","message":{"content":"first"}}"#).unwrap();

        let mut source = ClaudeCharSource {
            transcript_dir: tmp.clone(),
            chars: Vec::new(),
            max_chars: 100,
            last_mtime: None,
        };
        source.reload().unwrap();
        let first_chars = source.chars.clone();

        // Reload without changing the file — should skip (same mtime)
        source.reload().unwrap();
        assert_eq!(source.chars, first_chars);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
