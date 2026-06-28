# neo-rainst 项目架构

neo-rainst 是经典 `neo` Matrix 数字雨屏幕保护程序的 Rust 移植版。以终端 TUI 形式渲染 Matrix 风格的绿色代码雨动画。

## 项目概览

```
vibe-neo-matrix/
├── src/
│   ├── main.rs              # 入口：CLI 解析 + 事件循环
│   ├── lib.rs               # 模块声明
│   ├── cloud.rs             # 模拟引擎核心 (Cloud)
│   ├── droplet.rs           # 单列雨滴状态机
│   ├── render.rs            # ratatui Widget 渲染
│   ├── char_source.rs       # 字符源抽象层
│   ├── transcript/mod.rs    # 从 LLM 会话历史提取字符
│   ├── color.rs             # 颜色主题系统
│   ├── config.rs            # TOML 配置加载
│   └── params.rs            # 统一参数模型
├── tests/
│   └── char_source_tests.rs # 字符源集成测试
├── colors.toml              # 颜色主题数据
├── .neo-rainst.toml         # 项目级默认配置
├── install.sh               # 一键安装脚本
└── docs/                    # 文档站点 (Starlight/Astro)
```

## 分层架构

```
┌─────────────────────────────────────────┐
│              main.rs                     │
│   CLI 解析 → 配置合并 → 事件循环         │
├─────────────────────────────────────────┤
│   params.rs     config.rs               │  配置层
│   (参数模型)    (TOML 加载)              │
├─────────────────────────────────────────┤
│   cloud.rs      droplet.rs              │  模拟层
│   (模拟引擎)    (雨滴状态机)              │
├─────────────────────────────────────────┤
│   char_source.rs   transcript/mod.rs    │  数据层
│   (字符源抽象)     (LLM 会话提取)         │
├─────────────────────────────────────────┤
│   render.rs      color.rs               │  渲染层
│   (ratatui 绘制)  (颜色主题)             │
└─────────────────────────────────────────┘
```

## 模块详解

### main.rs — 应用入口

**职责:** CLI 参数解析、配置分层合并、终端初始化和事件循环。

```
CLI Args  ──→  SimParams  ──→  merge(default, config, cli)
                                      │
                    ┌─────────────────┘
                    ▼
           build CharSource  ──→  Cloud::new()
                    │
                    ▼
           事件循环 (poll / rain / render / sleep)
```

- 参数合并顺序: `Default → XDG config → project config → CLI args`
- 字符源解析优先级: `--chars > --charset-stdin > --charset-file > config.charset.source > claude.enabled`
- 支持 `Inline`（内联）和 `Fullscreen` 两种终端模式
- 事件循环处理按键、窗口大小变化、Claude 会话目录轮询

### params.rs — 统一参数模型

**职责:** 同时承载 TOML 配置和 CLI 参数，统一为 `SimParams` 结构体。

所有字段均为 `Option<T>`，通过 `merge()` 方法实现分层覆盖：

```rust
let merged = SimParams::default()   // 最底层默认值
    .merge(&config_params)          // TOML 配置覆盖
    .merge(&cli_params);            // CLI 参数最高优先级
```

参数分为四类:
- **字符源:** `charset`, `chars`, `charset_file`, `charset_stdin`
- **渲染:** `color`, `speed`, `density`, `fps`, `shading_mode`, `bold`, `full_width`
- **雨滴行为:** `maxdpc`, `short_pct`, `rip_pct`, `async_scroll`
- **效果:** `glitch_*`, `linger_*`, `no_glitch`

### config.rs — TOML 配置加载

**职责:** 从多个位置加载 TOML 配置，转换为 `SimParams`。

加载优先级:
1. `--config <FILE>` 显式路径
2. `./.neo-rainst.toml` 项目级配置
3. `$XDG_CONFIG_HOME/neo-rainst/config.toml` 用户配置
4. 内置默认值

配置结构:
- `[render]` — 帧率、颜色、shading/bold 模式
- `[charset]` — 字符集来源（名称/文件/stdin/claude）
- `[rain]` — 速度、密度、异步滚动、雨滴限制
- `[glitch]` — 故障效果开关、概率、时间范围
- `[linger]` — 停留时间范围
- `[exit]` — 退出模式（normal/on-key/after-secs）
- `[claude]` — Claude Code 会话集成

### char_source.rs — 字符源抽象

**职责:** 统一多种字符来源的 trait 接口，外加 emoji 过滤。

```rust
pub trait CharSource {
    fn name(&self) -> &str;
    fn chars(&self) -> &[char];
    fn reload(&mut self) -> io::Result<()>;
}
```

五种实现:

| 实现 | 来源 | 说明 |
|------|------|------|
| `BuiltinChars` | Unicode 范围 | 预定义字符集（katakana, ascii, hex 等 10+ 种） |
| `FileCharSource` | 文件 | 从文件读取字符（过滤空白和控制符） |
| `StdinCharSource` | 管道 | 从 stdin 读取字符 |
| `ClaudeCharSource` | LLM 会话 | 委托给 TranscriptCharSource |
| `TranscriptCharSource` | JSONL | 从 transcript 文件提取 + emoji 过滤 |

辅助函数:
- `parse_chars_arg()` — 解析 `--chars` 的 Unicode 码点对格式
- `is_emoji()` — 判断字符是否为 emoji（覆盖 Emoticons、Misc Symbols、Dingbats、Regional Indicators 等区块）

### transcript/mod.rs — LLM 会话文本提取

**职责:** 从 Claude Code 的 JSONL transcript 文件中提取文本内容。

核心流程:
1. 扫描 `~/.claude/projects/<flattened-cwd>/` 目录
2. 找到最近修改的 `*.jsonl` 文件
3. 逐行解析 JSON，提取 `user` 和 `assistant` 消息文本
4. 过滤控制字符和 emoji 符号
5. 截取最后 `max_chars` 个字符

```
JSONL line → serde_json::Value
    │
    ▼
extract_claude_jsonl_entry()
    │
    ├── type="user"    → message.content / content
    ├── type="assistant" → message.content[].text (仅 text block)
    └── 其他 type       → 跳过
    │
    ▼
filter(!is_control && !is_emoji) → Vec<char>
```

支持的格式策略:
- `ClaudeJsonl` — 逐行 JSON，提取 user/assistant 文本
- `Plain` — 纯文本，过滤空白和控制符

### droplet.rs — 雨滴状态机

**职责:** 单列雨滴的完整生命周期管理。

**生命周期状态转换:**

```
       activate()
  [空闲] ──────────→ [Head 推进]
                          │
           head >= end_line?
                          │
                          ▼
                    [Head 停止]
                          │
              ┌─── ttl > 0? ───┐
              │ 是              │ 否
              ▼                 ▼
         [Tail 暂停]      [Tail 推进]
              │                 │
        linger 到期?            │
              │                 │
              ▼                 │
         [Tail 推进] ◄──────────┘
              │
        tail >= head?
              │
              ▼
           [死亡]
```

关键字段:
- `head_put_line` — Head 当前行（可见范围上限）
- `tail_put_line` — Tail 当前行（可见范围下限），`None` 表示尚未推进
- `end_line` — Head 停止行
- `time_to_linger` — Head 停止后 Tail 延迟推进的时长

`CharLoc` 枚举（`Tail | Head | Middle`）决定每个字符位置的渲染属性。

### cloud.rs — 模拟引擎

**职责:** 管理所有列、雨滴实例、字符池、故障效果和渲染状态的中央引擎。

```
Cloud
├── 列状态管理 (col_stat)
│   ├── max_speed_pct (异步滚动速度)
│   ├── num_droplets (当前活跃雨滴数)
│   └── can_spawn (是否允许新雨滴)
├── 字符池 (CHAR_POOL_SIZE=2048)
│   ├── char_pool (常规雨滴字符)
│   └── glitch_pool (故障替换字符)
├── 雨滴调度
│   ├── spawn_droplets() (按密度速率生成)
│   └── fill_droplet() (随机初始化参数)
├── 故障系统
│   ├── glitch_map (每像素故障标志)
│   ├── glitch 周期 (明/暗阶段)
│   └── do_glitch() (替换故障位置的字符)
└── 消息系统
    ├── set_message() (将消息分布在屏幕中央)
    └── calc_message() (只有雨滴经过时才显示)
```

**每帧 `rain()` 流程:**
1. 按 `droplets_per_sec` 速率生成新雨滴
2. 遍历所有活跃雨滴，调用 `droplet.advance(now)`
3. 检查 Tail 是否跨过 1/4 屏幕阈值，允许该列重新生成
4. 如果到了 glitch 时间点，执行 `do_glitch()`
5. 如果已设置消息，计算哪些消息字符被雨滴"揭示"

### render.rs — 终端渲染

**职责:** 将 Cloud 状态绘制到终端缓冲区。

`RainWidget` 实现 `ratatui::Widget`:
1. 清空缓冲区（`Color::Reset` 保留终端默认背景）
2. 遍历所有活跃雨滴，计算每行字符、颜色对、粗体标志
3. 写入对应 cell
4. 渲染被雨滴覆盖的消息字符

`StatusBar` 实现底部状态栏:
- 显示速度、密度、故障概率、字符源、颜色主题
- 暂停时显示 `[PAUSED]`

### color.rs — 颜色主题系统

**职责:** 颜色主题定义、加载和属性计算。

- 主题数据以 TOML 格式存储在 `colors.toml`，编译时 `include_str!` 嵌入二进制
- 运行时通过 `thread_local!` 缓存解析结果
- 支持 16 色（indexed）和 TrueColor 两种输出模式
- 颜色模式检测: `$COLORTERM` → `$TERM` → 默认 TrueColor

`ThemeColors::style(pair, is_bold)` 返回 `ratatui::Style`，pair 1 为最暗（尾部），pair 7 为最亮（头部）。

## 配置流

```
colors.toml ──(include_str!)──→ 嵌入二进制
                                    │
.neo-rainst.toml ──→ load_config() ──→ Config ──→ SimParams
XDG config.toml  ──→                                  │
CLI args ─────────────→ Args ────→ SimParams ────────┘
                                                       │
                                                       ▼
                                               Cloud::apply_params()
```

## 字符源选择决策树

```
--chars 参数是否提供?
├── 是 → 解析 Unicode 码点对 → BuiltinChars::from_unicode_pairs()
├── --charset-stdin? → StdinCharSource
├── --charset-file? → FileCharSource
└── config.charset.source?
    ├── "file:..." → FileCharSource
    ├── "stdin" → StdinCharSource
    ├── "claude" 或 claude.enabled=true
    │   └── ClaudeCharSource (→ TranscriptCharSource)
    │       ├── 扫描 transcript 目录找最新 .jsonl
    │       ├── 提取 user/assistant 文本
    │       ├── 过滤 control + emoji
    │       └── 截取 max_chars 尾字符
    └── 其他名称 → BuiltinChars::from_charset_name()
```

## 事件循环

```
while cloud.raining:
    poll input (non-blocking)
    ├── 按键 → handle_key() → 调整参数 / 退出
    └── Resize → reset_with_size()

    exit-after-secs? → 检查超时

    cloud.rain()
    ├── spawn_droplets()
    ├── droplet.advance() × N
    └── do_glitch()

    transcript_dir 变更? → cloud.reload_chars()  (每秒)

    terminal.draw(RainWidget)
    ├── 渲染雨滴字符
    └── 渲染消息 / 状态栏

    sleep(FPS 目标帧间隔, EMA 平滑)
```

## 关键设计决策

1. **字符源 trait 抽象** — 将内置字符集、文件、stdin、LLM 会话统一为同一个接口，Cloud 不关心字符来源。

2. **字符池预计算** — 启动时从字符源随机采样生成 2048 字符池和 1024 故障池，运行时仅做索引查找，避免每帧调用 RNG。

3. **EMA 帧率平滑** — `cur_delay = (prev_delay * 7 + calc_delay) / 8`，匹配原始 neo 的行为，避免帧时间抖动。

4. **glitch 明暗相位** — 故障周期分为 bright 阶段（0-25%）和 dim 阶段（75%+），通过时间比例计算，而非额外定时器。

5. **消息"揭示"机制** — 消息字符只有被雨滴覆盖时才显示，模拟原版 neo 的 `mvinnwstr` 屏幕读取行为。

6. **配置编译时嵌入** — `colors.toml` 通过 `include_str!` 嵌入二进制，无运行时文件依赖。

7. **emoji 过滤** — `TranscriptCharSource` 在构建字符向量时过滤 emoji 符号，确保终端雨渲染不受不可打印字符干扰。
