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

/// 判断一个字符是否为 emoji 符号。
///
/// 覆盖主要的 emoji Unicode 区块：Supplemental Symbols & Pictographs、
/// Emoticons、Dingbats、Miscellaneous Symbols、Variation Selectors、
/// Regional Indicators 等。
pub fn is_emoji(c: char) -> bool {
    let cp = c as u32;
    // 排除 ASCII 范围，避免误判
    if cp <= 0x7F {
        return false;
    }
    matches!(cp,
        // Supplemental Symbols and Pictographs (U+1F900–U+1F9FF)
        0x1F900..=0x1F9FF
        // Emoticons (U+1F600–U+1F64F)
        | 0x1F600..=0x1F64F
        // Miscellaneous Symbols and Pictographs (U+1F300–U+1F5FF)
        | 0x1F300..=0x1F5FF
        // Transport and Map Symbols (U+1F680–U+1F6FF)
        | 0x1F680..=0x1F6FF
        // Symbols and Pictographs Extended-A (U+1FA70–U+1FAFF)
        | 0x1FA70..=0x1FAFF
        // Chess Symbols, Geometric Shapes Extended (U+1FA00–U+1FA6F)
        | 0x1FA00..=0x1FA6F
        // Regional Indicator Symbols (U+1F1E0–U+1F1FF)
        | 0x1F1E0..=0x1F1FF
        // Miscellaneous Symbols (U+2600–U+26FF)
        | 0x2600..=0x26FF
        // Dingbats (U+2700–U+27BF)
        | 0x2700..=0x27BF
        // Variation Selectors (U+FE00–U+FE0F)
        | 0xFE00..=0xFE0F
        // Zero Width Joiner (U+200D)
        | 0x200D
        // Combining Enclosing Keycap (U+20E3)
        | 0x20E3
        // Additional single emoji codepoints
        | 0x231A..=0x231B   // watch, hourglass
        | 0x23E9..=0x23F3   // double arrows, hourglass with sand
        | 0x23F8..=0x23FA   // power symbols
        | 0x25AA..=0x25AB   // small squares
        | 0x25B6 | 0x25C0   // play/rewind triangles
        | 0x25FB..=0x25FE   // medium squares
        | 0x2934..=0x2935   // curved arrows
        | 0x2B05..=0x2B07   // arrows
        | 0x2B1B..=0x2B1C   // large squares
        | 0x2B50 | 0x2B55   // star, no-entry
        | 0x3030 | 0x303D   // wavy dash, part alternation
        | 0x3297 | 0x3299   // circled ideographs
        | 0x00A9 | 0x00AE   // copyright, registered
        | 0x2122 | 0x2139   // TM, info
    )
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
    fn test_file_char_source() {
        use std::io::Cursor;
        let data = "ABC 123 !@#";
        let mut source = FileCharSource {
            path: "test".into(),
            chars: Vec::new(),
        };
        source.load_from_reader(Cursor::new(data)).unwrap();
        assert_eq!(source.chars().len(), 9);
    }

    #[test]
    fn test_is_emoji_detects_smiley() {
        assert!(is_emoji('\u{1F600}'));  // Grinning face
        assert!(is_emoji('\u{1F60D}'));  // Heart eyes
        assert!(is_emoji('\u{1F609}'));  // Wink
    }

    #[test]
    fn test_is_emoji_detects_symbols() {
        assert!(is_emoji('\u{1F4A9}'));  // Pile of poo
        assert!(is_emoji('\u{1F389}'));  // Party popper
        assert!(is_emoji('\u{2705}'));   // Check mark
        assert!(is_emoji('\u{26A0}'));   // Warning
    }

    #[test]
    fn test_is_emoji_rejects_ascii() {
        assert!(!is_emoji('A'));
        assert!(!is_emoji('z'));
        assert!(!is_emoji('0'));
        assert!(!is_emoji('.'));
        assert!(!is_emoji('!'));
    }

    #[test]
    fn test_is_emoji_rejects_cjk() {
        assert!(!is_emoji('\u{4E2D}'));  // 中
        assert!(!is_emoji('\u{6587}'));  // 文
        assert!(!is_emoji('\u{30A2}'));  // ア (katakana)
    }

    #[test]
    fn test_is_emoji_rejects_newline_and_control() {
        assert!(!is_emoji('\n'));
        assert!(!is_emoji('\t'));
        assert!(!is_emoji('\u{0000}'));
    }
}
