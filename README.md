# neo-rainst

用 Rust + ratatui 复刻 [neo](https://github.com/st3w/neo) 数字雨，支持自定义字符源和 Claude Code 会话渲染。

## 安装

```bash
# 通过 Cargo 安装（推荐）
cargo install --git https://github.com/dontreadthisline/vibe-neo-matrix.git

# 或一键安装脚本
curl -fsSL https://raw.githubusercontent.com/dontreadthisline/vibe-neo-matrix/master/install.sh | bash
```

## 基本用法

```bash
neo-rainst                           # 默认启动（片假名 + 经典绿色）
neo-rainst --charset ascii --color cyan
neo-rainst --message "Wake up, Neo..."
neo-rainst --screensaver             # 屏保模式
neo-rainst --exit-after-secs 30      # 30 秒后自动退出
```

## 键盘控制

| 按键 | 功能 |
|------|------|
| `q` / `Esc` | 退出 |
| `空格` | 重置数字雨 |
| `p` | 暂停 / 继续 |
| `↑` / `↓` | 加速 / 减速 |
| `←` / `→` | 减少 / 增加故障效果比例 |
| `Tab` | 切换着色模式（随机 / 距离渐变） |
| `a` | 切换异步滚动 |
| `-` / `+` | 减少 / 增加雨滴密度 |
| `1`-`0` | 切换颜色主题（green, green2, green3, gold, pink2, red, blue, cyan, purple, gray） |
| `!`-`%` | 更多颜色（rainbow, yellow, orange, pink, vaporwave） |

## CLI 参数

```
neo-rainst [OPTIONS]

字符源:
  --charset <NAME>        字符集: katakana, ascii, binary, hex, claude 等
  --charset-file <FILE>   从文件加载字符
  --charset-stdin         从 stdin 读取字符
  --chars <RANGE>         Unicode 码点范围，如 0x3040,0x309F

渲染:
  -c, --color <COLOR>     颜色主题: green, blue, cyan, rainbow, vaporwave 等
  -S, --speed <NUM>       下落速度 (默认 8.0)
  -d, --density <NUM>     雨滴密度 (默认 1.0)
  -f, --fps <NUM>         帧率 (默认 60)
  -m, --message <STR>     消息文字（雨滴揭示效果）
  --status                显示底部状态栏
  -F, --fullwidth         全宽字符模式
  -D, --defaultbg         使用终端默认背景色

样式:
  -M, --shadingmode <NUM> 着色模式: 0=随机, 1=距离头部渐变
  -b, --bold <NUM>        粗体模式: 0=关闭, 1=随机, 2=全部
  --colormode <NUM>       颜色模式: 0=单色, 16=16色, 256=256色, 32=truecolor

雨滴行为:
  -a, --async             异步滚动
  --maxdpc <NUM>          每列最大雨滴数
  --shortpct <NUM>        短雨滴百分比
  -r, --rippct <NUM>      提前消失百分比

故障效果:
  -g, --glitchms <LOW,HIGH>  故障持续时间范围（ms）
  -G, --glitchpct <NUM>      故障百分比
  --noglitch                 禁用故障效果
  -l, --lingerms <LOW,HIGH>  字符停留时间范围（ms）

模式:
  --inline <LINES>        内联模式（不占用全屏）
  --screensaver           屏保模式（任意键退出）
  --exit-on-key           任意键退出
  --exit-after-secs <N>   N 秒后自动退出

配置:
  --config <FILE>         指定配置文件路径
```

完整参数列表: `neo-rainst --help`

## 配置文件

优先级: CLI 参数 > `.neo-rainst.toml` (项目级) > `~/.config/neo-rainst/config.toml` (XDG)

示例 `.neo-rainst.toml`:

```toml
[charset]
source = "katakana"

[render]
fps = 60
color = "green"
shading_mode = 0   # 0=随机, 1=距离头部渐变
bold_mode = 1      # 0=关闭, 1=随机, 2=全部
full_width = false
default_bg = true
show_status = false

[rain]
speed = 8.0
density = 1.0
async_scroll = false
max_droplets_per_col = 3
short_pct = 50.0
die_early_pct = 33.3

[glitch]
enabled = true
pct = 10.0
low_ms = 300
high_ms = 400

[linger]
low_ms = 1
high_ms = 3000

[exit]
mode = "normal"

[claude]
enabled = false
transcript_dir = ""
max_chars = 10000
```

## Claude Code 集成

neo-rainst 可以自动读取 Claude Code 的对话 transcript 并渲染为数字雨。

### 自动模式（推荐）

配置文件中启用 claude 字符源即可，neo-rainst 会自动扫描 `~/.claude/projects/<project-dir>/` 目录下的 JSONL transcript 文件：

```toml
[charset]
source = "claude"

[claude]
enabled = true
transcript_dir = ""   # 留空则根据 CWD 自动推导
max_chars = 10000

[exit]
mode = "on-key"
```

```bash
neo-rainst --charset claude --exit-on-key
```

### bashrc 快捷函数

在 `~/.bashrc` 中添加，使每次 Claude Code 会话结束后自动渲染数字雨：

```bash
c() {
  cc_ds
  claude --model deepseek-v4-pro --dangerously-skip-permissions --allowedTools all "$@"
  neo-rainst --exit-on-key
}
```

### 字符源工作方式

neo-rainst 每 ~1 秒扫描一次 transcript 目录，找到最近修改的 `.jsonl` 文件，提取 user 和 assistant 消息中的纯文本，截取最后 `max_chars` 个字符作为雨滴内容。无需额外的 hook 脚本或中间文件。
