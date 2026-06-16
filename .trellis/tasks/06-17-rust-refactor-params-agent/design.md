# 技术设计

## 模块变更总览

```
src/
  main.rs          ← 大幅缩减 (500→350行), apply_args() 消除
  cloud.rs         ← 轻微改动, set_* 方法改为 apply_params()
  droplet.rs       ← SENTINEL→Option<u16>, 不影响公开API
  char_source.rs   ← ClaudeCharSource→TranscriptCharSource, 新增 AgentConfig
  config.rs        ← 新增 AgentConfig, 保留 ClaudeConfig (deprecated 兼容)
  params.rs        ← 新文件, 收敛的参数模型 + clap+serde derive
  transcript/      ← 新目录
    mod.rs         ← TranscriptFormat trait, agent 注册表
    builtin.rs     ← 知名 agent 默认值
```

## 关键设计决策

### 1. 参数模型 (params.rs)

```rust
/// 收敛所有仿真参数。clap 负责 CLI 解析，serde 负责 TOML 加载。
#[derive(Parser, Serialize, Deserialize, Debug, Clone)]
pub struct SimParams {
    // 字符源
    #[arg(long)] pub charset: Option<String>,
    #[arg(long)] pub charset_file: Option<PathBuf>,
    #[arg(long)] pub charset_stdin: bool,
    #[arg(long)] pub chars: Option<String>,

    // 渲染
    #[arg(short='c', long)] pub color: Option<String>,
    #[arg(short='S', long)] pub speed: Option<f32>,
    #[arg(short='d', long)] pub density: Option<f32>,
    #[arg(short='f', long, default_value="60")] pub fps: f64,
    #[arg(short='m', long)] pub message: Option<String>,
    #[arg(long)] pub status: bool,
    // ... 全部参数
}
```

**关键**: 所有字段用 `Option<T>` 包裹以支持分层合并 (CLI > project-config > XDG-config > default)。

合并顺序:
```rust
let params = SimParams::default()       // 硬编码默认值
    .merge(xdg_config)                   // ~/.config/neo-rainst/config.toml
    .merge(project_config)               // .neo-rainst.toml
    .merge(cli_args);                    // --flags
```

### 2. droplet.rs: SENTINEL 消除

```rust
// Before (C++ style)
pub tail_put_line: u16,  // Self::SENTINEL = 0xFFFF
if self.tail_put_line == Self::SENTINEL { ... }

// After (Rust style)
pub tail_put_line: Option<u16>,  // None = not set
if self.tail_put_line.is_none() { ... }
```

这是纯内部改动，`Droplet` 字段不对外暴露。

### 3. Agent 配置驱动化

```rust
// transcript/mod.rs
pub enum TranscriptFormat {
    ClaudeJsonl,    // type: user/assistant, message.content
    GenericJsonl,   // 递归提取所有字符串值
    PlainText,       // 直接当字符用
}

pub struct AgentConfig {
    pub name: String,
    pub transcript_dir_template: String,  // "~/.claude/projects/-{cwd_flat}"
    pub format: TranscriptFormat,
    pub file_glob: String,                // "*.jsonl"
}
```

内置知名 agent:
```rust
fn BUILTIN_AGENTS: &[AgentConfig] = &[
    AgentConfig {
        name: "claude-code",
        transcript_dir_template: "~/.claude/projects/-{cwd_flat}",
        format: TranscriptFormat::ClaudeJsonl,
        file_glob: "*.jsonl",
    },
    // 后续新增: opencode, pi, ...
];
```

### 4. 向后兼容

`[claude]` 在 config.rs 中保留为 deprecated:
```rust
#[derive(Deserialize, Default)]
pub struct ClaudeConfig {
    pub enabled: bool,
    pub transcript_dir: String,
    pub max_chars: usize,
}
```

加载时，如果检测到 `[claude]` 节且 `enabled=true`，自动映射为:
```
charset.source = "agent:claude-code"
agent.claude-code.transcript_dir = <value>
agent.claude-code.max_chars = <value>
```

## 不做的

- 不改 `ratatui`/`crossterm` 版本
- 不改颜色主题数据表
- 不改配置优先级语义
- 不引入异步运行时 (notify 太重)
- 不引入 figment (过度设计，20行合并逻辑不需要框架)
