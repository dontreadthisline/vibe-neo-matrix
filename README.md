# neo-rainst

用 Rust + ratatui 复刻 [neo](https://github.com/st3w/neo) 数字雨，支持自定义字符源和 Claude Code 会话渲染。

## 构建与运行

```bash
cargo build --release
cargo run
```

按 `q` 或 `Esc` 退出。空格暂停/继续，方向键调整速度，数字键 1-0 切换颜色主题。

## CLI 参数

```
neo-rainst [OPTIONS]

字符源:
  --charset <NAME>        字符集: katakana, ascii, binary, hex, claude 等
  --charset-file <FILE>   从文件加载字符
  --charset-stdin         从 stdin 读取字符

渲染:
  -c, --color <COLOR>     颜色主题: green, blue, cyan, rainbow 等
  -S, --speed <NUM>       下落速度 (默认 8.0)
  -d, --density <NUM>     雨滴密度 (默认 1.0)
  -f, --fps <NUM>         帧率 (默认 60)
  -m, --message <STR>     消息文字（雨滴揭示效果）
  --status                显示底部状态栏

模式:
  --inline <LINES>        内联模式（不占用全屏）
  --screensaver           屏保模式（任意键退出）
  --exit-on-key           任意键退出
  --exit-after-secs <N>   N 秒后自动退出

配置:
  --config <FILE>         指定配置文件
```

完整参数列表: `cargo run -- --help`

## 配置文件

优先级: CLI 参数 > `.neo-rainst.toml` (项目级) > `~/.config/neo-rainst/config.toml` (XDG)

示例 `.neo-rainst.toml`:

```toml
[charset]
source = "katakana"

[render]
fps = 60
color = "green"
default_bg = true
show_status = false

[claude]
enabled = false
session_file = "/tmp/neo-rainst-claude-session.txt"
max_chars = 10000
```

## Claude Code 集成

让 neo-rainst 在 Claude Code 会话结束时自动提取对话内容并渲染为数字雨。

### 配置步骤

**1. 复制 hook 脚本到项目：**

```bash
cp scripts/session-end.py .claude/hooks/
chmod +x .claude/hooks/session-end.py
```

**2. 在 `.claude/settings.json` 中添加 SessionEnd hook：**

```json
{
  "hooks": {
    "SessionEnd": [
      {
        "matcher": "*",
        "hooks": [
          {
            "type": "command",
            "command": "python3 .claude/hooks/session-end.py",
            "timeout": 30
          }
        ]
      }
    ]
  }
}
```

**3. 在 `.neo-rainst.toml` 中启用 claude 字符源：**

```toml
[charset]
source = "claude"

[claude]
session_file = "/tmp/neo-rainst-claude-session.txt"
max_chars = 10000

[exit]
mode = "on-key"
```

**4. 重启 Claude Code 使配置生效。**

### 使用流程

1. 正常使用 Claude Code 进行对话
2. 输入 `/exit` 或 Ctrl+C 结束会话
3. SessionEnd hook 自动提取会话文字并启动 neo-rainst
4. 数字雨展示当前会话内容，按任意键退出
