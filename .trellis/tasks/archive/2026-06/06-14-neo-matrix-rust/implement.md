# neo-rainst 实现计划

实现顺序：核心渲染 → 颜色系统 → CLI/配置 → 扩展字符源 → Claude Code 集成

---

## Phase 1: 核心渲染引擎

### 1.1 项目脚手架
- `cargo init`，配置 `Cargo.toml` 依赖
- `main.rs`: `ratatui::run()` 骨架，空白 frame 渲染
- 验证: `cargo build && cargo run`，终端进入 alternate screen，按 q 退出

### 1.2 Droplet 状态机
- `droplet.rs`: `Droplet` struct + `new()`, `activate()`, `advance()`, `reset()`
- 完整 Head/Tail 推进逻辑、linger 暂停/恢复、死亡判定
- 单元测试：验证状态转换（head 停止、tail 暂停、tail 恢复、死亡）
- 验证: `cargo test droplet`

### 1.3 CharSource trait + Builtin
- `char_source.rs`: `CharSource` trait 定义
- `BuiltinChars::new(charset)` — 从 Unicode 范围构建字符向量
- 字符池预计算：`char_pool[2048]` + `glitch_pool[1024]`
- `get_char(line, cp_idx)` — `char_pool[(cp_idx + line) % 2048]` 索引
- 单元测试：验证字符池大小、索引确定性
- 验证: `cargo test char_source`

### 1.4 Cloud 核心 + Rain 循环
- `cloud.rs`: `Cloud` struct + `new()`, `reset()`, `init_chars()`
- `spawn_droplets()` — 基于时间积分、密度、列可用性生成雨滴
- `rain()` — 每帧主逻辑（推进 → glitch → 标记死亡 → 消息）
- `get_attr()` — 计算字符的 color pair 和 bold 属性
- 验证: 集成测试，Cloud::rain() 调用后 droplets 状态正确变化

### 1.5 颜色系统
- `color.rs`: `ColorTheme` 枚举 + 全部 16 个主题的 7 级渐变表
- `ThemeColors::style(pair_idx, bold, color_mode)` → `ratatui::Style`
- 颜色模式自动检测（truecolor/256/16/mono）
- 验证: 单元测试确认各主题各模式产生正确 Style

### 1.6 Glitch 系统
- `glitch_map` 布尔矩阵 + `glitch_pool` + `glitch_pool_idx`
- 时间驱动的 glitch 触发（`low_ms`~`high_ms` 随机间隔）
- bright/dim 周期（0~25% bright，75%+ dim）
- 验证: 单元测试 glitch_map 生成概率、glitch 触发时序

### 1.7 ratatui 自定义 Widget 渲染
- `render.rs`: `RainWidget` 实现 `Widget` trait
- 遍历 alive droplets，逐 cell 写入 `Buffer`
- 消息字符叠加渲染
- 验证: 启动 `cargo run`，目视确认终端有数字雨下落效果

### 1.8 主循环 + 输入处理
- `main.rs`: 完整主循环（input → rain → draw → sleep）
- 加权移动平均帧率控制
- 所有键盘快捷键（方向键、Tab、数字键、q/ESC、空格等）
- resize 事件处理
- 验证: 全键盘交互测试

---

## Phase 2: CLI + 配置

### 2.1 CLI 解析
- `clap` derive，全部参数（兼容原版 + 新增）
- 验证: `cargo run -- --help` 显示完整帮助
- 验证: `neo-rainst -c green -S 10 -m "MATRIX"` 生效

### 2.2 TOML 配置
- `config.rs`: 加载 XDG config、项目级 `.neo-rainst.toml`、`--config`
- Config struct 序列化/反序列化
- 优先级合并：CLI > .neo-rainst.toml > XDG config
- 验证: `cargo run -- --config ./test-config.toml`

### 2.3 全屏/内联模式
- `--fullscreen`（默认 alternate screen）
- `--inline N`（Viewport::Inline）
- 验证: `cargo run -- --inline 10` 在当前终端下方 10 行渲染

---

## Phase 3: 扩展字符源

### 3.1 FileCharSource
- 读取文本文件，收集非空白非控制字符
- 错误处理：文件不存在/权限错误
- 验证: `echo "ABC123" > /tmp/test-chars.txt && cargo run -- --charset-file /tmp/test-chars.txt`

### 3.2 StdinCharSource
- 读取 stdin，构建字符池
- 验证: `echo "こんにちは" | cargo run -- --charset-stdin`

### 3.3 运行时字符源切换
- 按键触发 `char_source.reload()` 或切换 source
- 切换后 `init_chars()` 重新构建字符池
- 验证: 运行时按键切换，字符立即变化

---

## Phase 4: Claude Code 集成

### 4.1 ClaudeCharSource
- 读取 `/tmp/neo-rainst-claude-session.txt`
- 截取最后 `max_chars` 字符
- 验证: 单元测试截取逻辑

### 4.2 SessionEnd Hook 脚本
- 脚本 `neo-rainst-hook.sh`:
  1. 从 Claude Code transcript 提取最近 N 字符
  2. 写入 `/tmp/neo-rainst-claude-session.txt`
  3. 启动 `neo-rainst --charset-file /tmp/neo-rainst-claude-session.txt --exit-after-secs 10`
- 说明文档写入 README
- 验证: 手动模拟 hook 环境变量测试

### 4.3 退出模式
- `--exit-on-key`：crossterm 任意按键 → break 主循环
- `--exit-after-secs N`：记录开始时间，超时后 break
- 验证: `cargo run -- --exit-after-secs 5` 5 秒后自动退出

---

## 检查清单

- [ ] `cargo build --release` 无错误无警告
- [ ] `cargo test` 全部通过
- [ ] `cargo clippy` 无警告
- [ ] 与原版 neo 并排对比视觉效果一致（片假名、绿色渐变、glitch 闪烁）
- [ ] 所有 CLI 参数可独立验证生效
- [ ] Resize 不崩溃
