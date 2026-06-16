# neo-rainst: 用 ratatui 复刻 neo 数字雨，支持自定义字符源

## Goal

用 Rust + ratatui 复刻 [neo](https://github.com/st3w/neo) 的核心数字雨渲染效果，补充其缺失的自定义字符源能力，并额外提供 Claude Code 会话历史渲染。

## 原始 neo 核心机制（必须复刻）

以下内容基于对原始 C++ 源码（`neo.cpp`、`cloud.h/cpp`、`droplet.h/cpp`）的完整分析。

### 渲染架构
- **Cloud**：管理所有列状态和雨滴生命周期，每帧调用 `Rain()` 推进渲染
- **Droplet**：单列雨滴，追踪 head（头部/最下方）和 tail（尾部/最上方）的位置
- **每帧流程**：`SpawnDroplets()` → 推进每个 Droplet → `DoGlitch()` → `Draw()` → 消息渲染

### Droplet 生命周期（关键算法）
```
激活 → Head 向下推进 → Head 到达 endLine 停止
     → Tail 暂停（linger 期间）
     → Linger 时间到，Tail 恢复推进
     → Tail 追上 Head → Droplet 死亡
```
- Head 位置 `_headPutLine`，Tail 位置 `_tailPutLine`
- 当 Head 到达 `endLine`，Head 停止，`_headStopTime` 记录停止时间
- `_timeToLinger` 毫秒后 Tail 恢复推进
- `_dieEarlyPct` 概率让 endLine 提前（随机行），模拟雨滴提前消逝
- `_shortPct` 概率缩短雨滴长度

### 字符选择机制
- 预计算 `_charPool[2048]`，从当前字符集随机填充
- 每个 Droplet 获得一个 `_charPoolIdx`
- 屏幕上 `(col, line)` 位置的字符 = `_charPool[(charPoolIdx + line) % 2048]`
- 效果：同一列中每个位置字符固定，不同列不同

### Glitch 效果
- 预计算 `_glitchPool[1024]` 随机字符
- 预计算 `_glitchMap[cols * lines]` 布尔矩阵（`_glitchPct` 概率为 true，默认 10%）
- 时间驱动：`_glitchLowMs`~`_glitchHighMs` 间隔触发（默认 300~400ms）
- 触发时：对该 Droplet 覆盖范围内 `GlitchMap=true` 的位置，替换 charPool 中对应字符为 glitchPool 中字符
- 触发后的 25% 时间内画面变亮（bright），75% 后变暗（dim），强化闪烁感

### 颜色系统
- 每个颜色主题定义 7 个 color pair（1=最暗尾部 → 7=最亮头部）
- 头部字符：pair 7 + bold
- 尾部字符：pair 1 + 非 bold
- 中间字符：pair 2~5 随机（每个屏幕位置预先随机分配）
- Distance-from-head 模式下，颜色按 head 距离线性插值
- 对 GlitchMap=true 的位置：bright 时段升一档，dim 时段降一档

### 消息功能（`-m`）
- 消息字符预排列在屏幕中央（1/4 到 3/4 列宽，垂直居中）
- **不是直接覆盖渲染**，而是当雨滴经过消息位置时，该位置已画出字符 → `DrawMessage()` 用消息字符替换对应位置
- 视觉效果：雨滴"揭示"隐藏的消息（类似电影片头爬行效果）

### 运行时交互
- `q`/ESC：退出
- 空格：暂停/继续
- 方向键上下：调整速度
- 方向键左右：调整 Glitch 概率 ±5%
- Tab：切换 ShadingMode（RANDOM / DISTANCE_FROM_HEAD）
- 数字键 1-0 和特殊字符：切换颜色主题
- `-`/`+`：调整雨滴密度
- `a`：切换异步滚动

### CLI 参数（完整列表）
- `-a` async 异步滚动、`-b` bold 模式、`-c` 颜色、`-C` 颜色文件
- `-D` defaultbg、`-d` 密度、`-f` FPS、`-F` 全宽、`-g` glitch 时间
- `-G` glitch 概率、`-h` help、`-l` linger 时间、`-m` 消息
- `-M` shading mode、`-p` profile、`-r` die early 概率
- `-s` screensaver、`-S` 速度、`-V` version
- `--chars` 自定义 Unicode 范围、`--charset` 字符集
- `--colormode` 颜色模式、`--maxdpc` 每列最大雨滴数
- `--noglitch`、`--shortpct`

---

## Requirements

### R1: 核心渲染复刻（首要目标）
- 完整复刻 Droplet 生命周期：head/tail 推进、linger、die-early、short
- 字符池预计算 + charPoolIdx 索引机制
- Glitch 效果：时间驱动 + GlitchMap + bright/dim 周期
- 消息功能：`-m` 参数，雨滴揭示效果
- 颜色渐变：7 级 pair，head/tail/middle 区分，支持 RANDOM 和 DISTANCE_FROM_HEAD 两种 shading
- 终端 resize 自适应，复用 `Reset()` 机制
- 帧率控制：加权移动平均延迟 `curDelay = (7*prevDelay + calcDelay) / 8`
- FPS 目标默认 60，可配置

### R2: 扩展字符源（核心拓展）
neo 原有 `--chars` 只支持偶数个十六进制 Unicode 码点定义范围。拓展为：
- **内置字符集**（继承原版全部）：katakana（默认）、ascii、extended、digits、punc、binary、hex、greek、cyrillic、arabic、hebrew、devanagari、braille、runic
- **文件字符源**：`--charset-file /path/to/chars.txt`，纯文本文件中所有非空白非控制字符构成候选池
- **stdin 字符源**：`--charset-stdin`，从管道读取字符流构建候选池
- **Unicode 范围**：兼容原版 `--chars 0x3040,0x309F` 偶数对格式
- 字符源 trait 抽象，统一接口

### R3: Claude Code 集成（拓展功能）
- SessionEnd hook → `/tmp/neo-rainst-claude-session.txt` → 自动启动 neo-rainst
- 可配置渲染字符数上限（默认 10000）
- `--exit-on-key`（默认）/ `--exit-after-secs N`
- 回退监听模式：`inotify`/`kqueue` 监听文件变化

### R4: CLI 兼容性
- Short option 和 long option 完全兼容原版
- 新增：`--charset-file`、`--charset-stdin`、`--exit-on-key`、`--exit-after-secs`、`--config`、`--fullscreen`/`--inline`
- 所有参数可通过 TOML 配置文件预设
- 优先级：CLI > 环境变量 > 项目级 `.neo-rainst.toml` > XDG config

### R5: 颜色主题
- 继承原版全部 16 个颜色主题：GREEN, GREEN2, GREEN3, YELLOW, ORANGE, RED, BLUE, CYAN, GOLD, RAINBOW, PURPLE, PINK, PINK2, VAPORWAVE, GRAY, USER
- 自动检测终端颜色能力（truecolor/256/16/mono）
- 支持 `--colorfile` 颜色文件（兼容原版格式）

### R6: 跨平台与分发
- Linux + macOS
- 配置 XDG：`~/.config/neo-rainst/config.toml`
- `cargo install neo-rainst`

---

## Acceptance Criteria

- [ ] `cargo install neo-rainst` 可安装运行
- [ ] 与原版 neo 视觉效果一致：片假名下落的绿色渐变、glitch 闪烁、亮度波动
- [ ] `neo-rainst -m "HELLO"` 消息被雨滴逐步揭示
- [ ] `neo-rainst --charset binary` 切换为二进制字符集
- [ ] `neo-rainst --charset-file ./my-chars.txt` 从文件加载自定义字符
- [ ] `echo "CUSTOM" | neo-rainst --charset-stdin` 管道输入
- [ ] 终端 resize 自适应
- [ ] 所有原版键盘快捷键可用（方向键、Tab、数字键等）

## Out of Scope

- Windows 支持
- 原始 neo 的 autotools 构建兼容

## 项目元信息

- 包名/二进制名：`neo-rainst`
- License：MIT OR Apache-2.0

## Open Questions

1. Claude Code SessionEnd hook 的 stdin JSON 结构需实际验证（不影响 Phase 1 核心实现）
