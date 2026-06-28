use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

use crate::char_source::is_emoji;

/// 文本提取策略
#[derive(Debug, Clone)]
pub enum TranscriptFormat {
    /// Claude Code JSONL 格式
    ClaudeJsonl,
    /// 纯文本
    Plain,
}

/// Agent 元数据
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    pub format: TranscriptFormat,
    pub file_glob: String,
}

/// 提取 Claude Code JSONL 条目中的文本
pub fn extract_claude_jsonl_entry(entry: &serde_json::Value) -> String {
    let entry_type = entry.get("type").and_then(|v| v.as_str()).unwrap_or("");
    let mut text = String::new();

    match entry_type {
        "user" => {
            if let Some(content) = entry.get("message").and_then(|m| m.get("content")) {
                if let Some(s) = content.as_str() {
                    text.push_str(s);
                }
            }
            if let Some(content) = entry.get("content").and_then(|c| c.as_str()) {
                text.push_str(content);
            }
        }
        "assistant" => {
            if let Some(blocks) = entry.get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
            {
                for block in blocks {
                    if block.get("type").and_then(|v| v.as_str()) == Some("text") {
                        if let Some(txt) = block.get("text").and_then(|v| v.as_str()) {
                            text.push_str(txt);
                        }
                    }
                }
            }
        }
        _ => {}
    }
    text
}

/// CWD 路径 → Claude Code 风格目录名。
///
/// Claude Code 的命名规则：
/// - 去掉前导 `/`，剩余部分按 `/` 拆分
/// - 每个路径段中的 `_` 替换为 `-`
/// - 段之间用 `-` 连接，最终前缀 `-`
/// - **隐藏目录（以 `.` 开头）特殊处理**：去掉前导 `.`，并在段前额外添加一个 `-`
///
/// 示例：
/// - `/home/user/my_proj` → `-home-user-my-proj`
/// - `/home/user/.config`  → `-home-user--config`
/// - `/home/user/.cache/a` → `-home-user--cache-a`
pub fn flatten_cwd(cwd: &str) -> String {
    let mut result = String::new();
    for seg in cwd.trim_start_matches('/').split('/') {
        if seg.is_empty() {
            continue;
        }
        let normalized = seg.replace('_', "-");
        if let Some(stripped) = normalized.strip_prefix('.') {
            // 隐藏目录：去掉前导 . 并额外添加一个 -
            result.push('-');
            if !stripped.is_empty() {
                result.push('-');
                result.push_str(stripped);
            }
        } else {
            result.push('-');
            result.push_str(&normalized);
        }
    }
    // 根目录 `/` → trim 后为空，split 无有效段，返回 `-`
    if result.is_empty() {
        result.push('-');
    }
    result
}

pub struct TranscriptCharSource {
    agent: AgentConfig,
    transcript_dir: PathBuf,
    chars: Vec<char>,
    max_chars: usize,
    last_mtime: Option<SystemTime>,
}

impl TranscriptCharSource {
    /// 直接指定目录路径
    pub fn with_dir(agent: AgentConfig, dir: PathBuf, max_chars: usize) -> io::Result<Self> {
        let mut source = TranscriptCharSource {
            agent,
            transcript_dir: dir,
            chars: Vec::new(),
            max_chars,
            last_mtime: None,
        };
        source.do_reload()?;
        Ok(source)
    }

    fn do_reload(&mut self) -> io::Result<()> {
        let mut latest: Option<(SystemTime, PathBuf)> = None;
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
            let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !simple_glob_match(&self.agent.file_glob, fname) {
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

        let content = std::fs::read_to_string(&transcript_path)?;
        let all_text = match self.agent.format {
            TranscriptFormat::ClaudeJsonl => {
                let mut text = String::new();
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(entry) = serde_json::from_str::<serde_json::Value>(line) {
                        text.push_str(&extract_claude_jsonl_entry(&entry));
                    }
                }
                text
            }
            TranscriptFormat::Plain => {
                content.chars()
                    .filter(|c| !c.is_whitespace() && !c.is_control())
                    .collect()
            }
        };

        let chars: Vec<char> = all_text.chars()
            .filter(|c| !c.is_control() && !is_emoji(*c))
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

impl crate::char_source::CharSource for TranscriptCharSource {
    fn name(&self) -> &str { &self.agent.name }
    fn chars(&self) -> &[char] { &self.chars }
    fn reload(&mut self) -> io::Result<()> { self.do_reload() }
}

fn simple_glob_match(pattern: &str, fname: &str) -> bool {
    if pattern == "*" { return true; }
    if let Some(suffix) = pattern.strip_prefix('*') { return fname.ends_with(suffix); }
    fname == pattern
}
