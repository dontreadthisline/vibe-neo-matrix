use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

/// 文本提取策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptFormat {
    /// Claude Code JSONL: type=user/assistant, message.content
    ClaudeJsonl,
    /// 直接当纯文本字符源
    Plain,
}

/// Agent 元数据 —— 每个 coding agent 的 transcript 位置和格式
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub name: String,
    /// transcript 目录路径模板，支持 {cwd_flat} 变量
    pub transcript_dir: String,
    pub format: TranscriptFormat,
    pub file_glob: String,
}

/// 从 JSONL transcript 条目中提取文本 (Claude Code 格式)
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

/// 将 CWD 绝对路径转换为 Claude Code 风格的目录名
/// `/home/user/my_proj` → `-home-user-my-proj`
pub fn flatten_cwd(cwd: &str) -> String {
    format!("-{}", cwd.trim_start_matches('/').replace(['/', '_'], "-"))
}

/// 根据 CWD + agent config 推导 transcript 目录路径
pub fn resolve_transcript_dir(agent: &AgentConfig) -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let abs = cwd.canonicalize().ok()?;
    let cwd_flat = flatten_cwd(&abs.to_string_lossy());
    let resolved = agent.transcript_dir.replace("{cwd_flat}", &cwd_flat);
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
    let resolved = if resolved.starts_with("~/") {
        home.join(resolved.strip_prefix("~/").unwrap())
    } else {
        PathBuf::from(resolved)
    };
    Some(resolved)
}

/// 知名 agent 预设
pub fn builtin_agent(name: &str) -> Option<AgentConfig> {
    match name {
        "claude-code" => Some(AgentConfig {
            name: "claude-code".into(),
            transcript_dir: "~/.claude/projects/-{cwd_flat}".into(),
            format: TranscriptFormat::ClaudeJsonl,
            file_glob: "*.jsonl".into(),
        }),
        _ => None,
    }
}

/// 通用 transcript 字符源，通过 AgentConfig 驱动
pub struct TranscriptCharSource {
    pub agent: AgentConfig,
    transcript_dir: PathBuf,
    chars: Vec<char>,
    max_chars: usize,
    last_mtime: Option<SystemTime>,
}

impl TranscriptCharSource {
    pub fn new(agent: AgentConfig, max_chars: usize) -> io::Result<Self> {
        let dir = resolve_transcript_dir(&agent)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "cannot resolve transcript dir"))?;
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

    /// 直接指定目录路径（兼容旧版 [claude] config 指定 transcript_dir 的场景）
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

    /// 内部刷新逻辑 (供 CharSource trait 实现和构造函数调用)
    fn do_reload(&mut self) -> io::Result<()> {
        // 1. 找到最近修改的匹配文件
        let mut latest: Option<(SystemTime, PathBuf)> = None;
        let dir = match std::fs::read_dir(&self.transcript_dir) {
            Ok(d) => d,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                self.chars.clear();
                return Ok(());
            }
            Err(e) => return Err(e),
        };

        let glob_pattern = &self.agent.file_glob;
        for entry in dir.flatten() {
            let path = entry.path();
            let fname = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !simple_glob_match(glob_pattern, fname) {
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

        // 3. 解析文件，提取文本
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

/// 极简 glob 匹配: `*.jsonl` 匹配以 `.jsonl` 结尾的文件名
fn simple_glob_match(pattern: &str, fname: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return fname.ends_with(suffix);
    }
    fname == pattern
}

// CharSource trait impl for TranscriptCharSource — delegates to inherent methods
impl crate::char_source::CharSource for TranscriptCharSource {
    fn name(&self) -> &str {
        &self.agent.name
    }

    fn chars(&self) -> &[char] {
        &self.chars
    }

    fn reload(&mut self) -> io::Result<()> {
        self.do_reload()
    }
}
