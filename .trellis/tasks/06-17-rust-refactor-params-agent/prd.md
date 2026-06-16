# Rust 风格重构: 参数模型收敛 + Agent 配置驱动化

## Goal

消除 C++ 复刻痕迹，收敛重复的参数定义，将 Agent 支持从硬编码改为配置驱动，同时保持完全向后兼容。

## Background

- 代码从 C++ neo 复刻而来，保留 C++ 惯用模式（哨兵值、两阶段初始化）
- clap Args / Config struct / Cloud setters 三处定义同一套参数
- ClaudeCharSource 硬编码 Claude Code 特有逻辑，扩展其他 agent 需改源码
- 手动实现 XDG 路径拼接，可用 `dirs` crate 替代

## Requirements

### R1: 参数模型收敛
- 单个 `SimParams` struct 同时承载 clap 和 serde，消除三处重复
- 用 clap `#[command(flatten)]` + serde `#[serde(flatten)]` 统一参数定义
- 消除 `apply_args()` 手动搬运逻辑

### R2: 引入 dirs crate
- 替代 `dirs_fallback()` 和 `xdg_config_path()` 手动路径拼接

### R3: Agent 配置驱动化
- `ClaudeCharSource` → `TranscriptCharSource`，内置 format 策略
- 知名 agent (claude-code) 内置默认目录模板
- TOML `[[agents]]` 支持自定义 agent
- `[claude]` 配置节继续有效（deprecated 但不报错）

### R4: 消除 C++ 痕迹
- `SENTINEL: u16 = 0xFFFF` → `Option<u16>`
- 两阶段 `new() + reset()` → `Default` trait

### R5: 零破坏
- `cargo run -- --help` 输出不变
- 现有 CLI 参数全部保持
- `[claude]` 配置节向后兼容

## Acceptance Criteria

- [ ] `cargo build` 成功
- [ ] `cargo test` 全部通过（含新增测试）
- [ ] `cargo run -- --help` 输出与重构前一致
- [ ] `cargo run -- --charset claude --exit-on-key` 行为不变
- [ ] 现有 `[claude]` TOML 配置继续生效
- [ ] 新增 `[[agents]]` TOML 配置可加载自定义 agent
- [ ] 代码量净减少 100+ 行
