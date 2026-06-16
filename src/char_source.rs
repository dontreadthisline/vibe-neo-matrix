use std::io::{self, Read};
use std::num::ParseIntError;

use crate::transcript::{AgentConfig, TranscriptCharSource, TranscriptFormat};

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
// ClaudeCharSource — 向后兼容封装，委托给 TranscriptCharSource
// ============================================================
///
/// 从 `~/.claude/projects/<project-dir>/` 目录中找到最近修改的 `.jsonl`
/// transcript 文件，提取 user/assistant 消息中的纯文本作为字符源。
///
/// 内部委托给通用的 TranscriptCharSource，使用 ClaudeJsonl 格式策略。
pub struct ClaudeCharSource {
    inner: TranscriptCharSource,
}

impl ClaudeCharSource {
    /// `transcript_dir` — Claude Code 项目 transcript 目录
    pub fn new(transcript_dir: &std::path::Path, max_chars: usize) -> io::Result<Self> {
        let agent = AgentConfig {
            name: "claude-session".into(),
            transcript_dir: String::new(), // unused — dir passed directly
            format: TranscriptFormat::ClaudeJsonl,
            file_glob: "*.jsonl".into(),
        };
        Ok(ClaudeCharSource {
            inner: TranscriptCharSource::with_dir(agent, transcript_dir.to_path_buf(), max_chars)?,
        })
    }

    /// 根据 CWD 自动推导 Claude Code transcript 目录路径
    pub fn transcript_dir_from_cwd() -> Option<std::path::PathBuf> {
        let cwd = std::env::current_dir().ok()?;
        let abs = cwd.canonicalize().ok()?;
        let dir_name = crate::transcript::flatten_cwd(&abs.to_string_lossy());
        let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"));
        Some(home.join(".claude").join("projects").join(dir_name))
    }
}

impl CharSource for ClaudeCharSource {
    fn name(&self) -> &str {
        self.inner.name()
    }

    fn chars(&self) -> &[char] {
        self.inner.chars()
    }

    fn reload(&mut self) -> io::Result<()> {
        self.inner.reload()
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
    #[test]
    fn test_transcript_dir_from_cwd_normalises_underscore() {
        let dir = crate::transcript::flatten_cwd("/home/zsl/projects/kinds_exer/vibe-neo-matrix");
        assert_eq!(dir, "-home-zsl-projects-kinds-exer-vibe-neo-matrix");
    }

    #[test]
    fn test_transcript_dir_from_cwd_simple_path() {
        let dir = crate::transcript::flatten_cwd("/home/user/my_project");
        assert_eq!(dir, "-home-user-my-project");
    }

    #[test]
    fn test_transcript_dir_from_cwd_root() {
        let dir = crate::transcript::flatten_cwd("/");
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
        let text = crate::transcript::extract_claude_jsonl_entry(&json);
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
        let text = crate::transcript::extract_claude_jsonl_entry(&json);
        assert_eq!(text, "Here is the fix:  use std::io;");
    }

    #[test]
    fn test_extract_text_from_user_entry_top_level_content() {
        let json = serde_json::json!({
            "type": "user",
            "content": "top level prompt text"
        });
        let text = crate::transcript::extract_claude_jsonl_entry(&json);
        assert_eq!(text, "top level prompt text");
    }

    #[test]
    fn test_extract_text_skips_system_types() {
        for sys_type in &["attachment", "file-history-snapshot", "mode", "system"] {
            let json = serde_json::json!({
                "type": sys_type,
                "message": { "content": "should be ignored" }
            });
            let text = crate::transcript::extract_claude_jsonl_entry(&json);
            assert!(text.is_empty(), "type={} should be skipped, got '{}'", sys_type, text);
        }
    }

    #[test]
    fn test_extract_text_empty_entry() {
        let json = serde_json::json!({});
        let text = crate::transcript::extract_claude_jsonl_entry(&json);
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

        let source = ClaudeCharSource::new(&tmp, 10000).unwrap();

        let chars_str: String = source.chars().iter().collect();
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
        let long_text = "A".repeat(200);
        let content = format!(r#"{{"type":"user","message":{{"content":"{}"}}}}"#, long_text);
        std::fs::write(&jsonl_path, content).unwrap();

        let source = ClaudeCharSource::new(&tmp, 100).unwrap();
        assert_eq!(source.chars().len(), 100);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_claude_char_source_empty_dir() {
        let tmp = std::env::temp_dir().join("neo-rainst-test-empty");
        let _ = std::fs::create_dir_all(&tmp);

        let source = ClaudeCharSource::new(&tmp, 100).unwrap();
        assert!(source.chars().is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_claude_char_source_nonexistent_dir() {
        let tmp = std::env::temp_dir().join("neo-rainst-test-nonexistent-12345");

        // Non-existent dir: TranscriptCharSource will clear chars on reload
        let result = ClaudeCharSource::new(&tmp, 100);
        // May succeed with empty chars or fail; either is acceptable
        if let Ok(source) = result {
            assert!(source.chars().is_empty());
        }
    }

    #[test]
    fn test_claude_char_source_no_duplicate_reload() {
        let tmp = std::env::temp_dir().join("neo-rainst-test-nodup");
        let _ = std::fs::create_dir_all(&tmp);

        let jsonl_path = tmp.join("test-session.jsonl");
        std::fs::write(&jsonl_path, r#"{"type":"user","message":{"content":"first"}}"#).unwrap();

        let mut source = ClaudeCharSource::new(&tmp, 100).unwrap();
        let first_chars = source.chars().to_vec();

        // Reload without changing the file — should skip (same mtime)
        source.reload().unwrap();
        assert_eq!(source.chars(), first_chars.as_slice());

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
