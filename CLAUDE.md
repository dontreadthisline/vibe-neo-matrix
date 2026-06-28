# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 项目概述

neo-rainst 是用 Rust + ratatui 复刻的 Matrix 数字雨屏幕保护程序。支持从 Claude Code 对话 transcript 中提取字符渲染为雨滴。

## 常用命令

```bash
cargo build                          # 编译
cargo run                            # 运行（默认 katakana + 绿色）
cargo run -- --charset claude        # Claude 会话模式
cargo run -- --status                # 带状态栏
cargo test                           # 全部测试
cargo test --test char_source_tests  # 仅集成测试
cargo test test_is_emoji             # 单个测试
cargo clippy                         # lint
```

## 架构分层

四层架构，上层依赖下层，下层不感知上层：

```
main.rs (CLI 解析 + 事件循环)
────────────────────────────
params.rs + config.rs       配置层 — SimParams 分层合并（Default → XDG → project → CLI）
────────────────────────────
cloud.rs + droplet.rs       模拟层 — Cloud 引擎 + Droplet 雨滴状态机
────────────────────────────
char_source.rs + transcript/ 数据层 — CharSource trait 抽象 + LLM 会话 JSONL 提取
────────────────────────────
render.rs + color.rs        渲染层 — ratatui Widget + 颜色主题系统
```

## 关键模块

### char_source.rs — 字符源 trait 体系

`CharSource` trait 统一五种来源：

| 实现 | 用途 |
|------|------|
| `BuiltinChars` | 预定义字符集（katakana, ascii, hex, greek, braille 等 10+ 种），从 Unicode 范围构造 |
| `FileCharSource` | 从文件读取字符，过滤空白和控制符 |
| `StdinCharSource` | 从管道读取 |
| `ClaudeCharSource` | 委托给 `TranscriptCharSource`，自动扫描 ~/.claude/projects/ 目录 |
| `TranscriptCharSource` | 解析 JSONL transcript，提取 user/assistant 文本，过滤 emoji |

核心辅助函数：
- `is_emoji(c)` — 覆盖 Emoticons / Misc Symbols / Dingbats / Regional Indicators 等主要 emoji 区块
- `parse_chars_arg(s)` — 解析 `--chars 0x3040,0x309F` 格式的 Unicode 码点对

### transcript/mod.rs — Claude 会话提取

从 `~/.claude/projects/<flattened-cwd>/` 找到最近修改的 `*.jsonl`，逐行提取 user/assistant 文本，过滤控制符 + emoji，截取末尾 `max_chars` 个字符。`extract_claude_jsonl_entry()` 处理两种 content 结构（`message.content` vs 顶层 `content`）。

### cloud.rs — 模拟引擎中央状态

`Cloud` 持有所有运行时状态：列统计、字符池（2048）、故障池（1024）、雨滴向量、glitch 映射、消息系统。`rain()` 每帧执行：生成雨滴 → 推进雨滴 → 执行故障 → 计算消息位置。

字符池在 `init_chars()` 时从 CharSource 随机采样预计算，运行时仅索引查找。

### droplet.rs — 单列雨滴状态机

生命周期: `空闲 → activate() → Head推进 → Head停止 → Tail暂停(linger) → Tail推进 → tail>=head死亡`

关键字段：`head_put_line`（可见范围上限）、`tail_put_line: Option<u16>`（可见范围下限，None 表示尚未推进）、`CharLoc` 枚举（Tail/Head/Middle）决定每个字符位置的渲染属性。

## 配置合并顺序

```
SimParams::default()           最底层
  .merge(&cfg_params)          TOML 配置（.neo-rainst.toml > XDG）
  .merge(&cli_params)          CLI 参数最高优先级
```

字符源选择优先级：`--chars > --charset-stdin > --charset-file > config.charset.source > claude.enabled`

## Trellis 任务管理

项目使用 Trellis 管理开发流程。当需要创建任务、计划实现或完成工作时，使用对应的 `/trellis:*` slash 命令。

- `.trellis/tasks/` — 活跃和归档的任务（PRD、设计、实现计划）
- `.trellis/workflow.md` — 开发阶段流程指引
- `.trellis/spec/` — 分层编码规范
- `.trellis/workspace/` — 开发者日志

当前活跃任务可通过 `python3 ./.trellis/scripts/task.py list --mine` 查看。
