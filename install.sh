#!/usr/bin/env bash
set -euo pipefail

REPO_URL="https://github.com/dontreadthisline/vibe-neo-matrix.git"
CONFIG_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/neo-rainst"
CONFIG_FILE="$CONFIG_DIR/config.toml"

echo "==> 通过 cargo install --git 安装 neo-rainst ..."
cargo install --git "$REPO_URL"

echo ""
echo "==> 安装默认配置到 $CONFIG_FILE ..."
mkdir -p "$CONFIG_DIR"

cat > "$CONFIG_FILE" <<'TOML'
# neo-rainst 配置
# 优先级: CLI 参数 > 项目级 .neo-rainst.toml > 此文件

[charset]
# 字符源: katakana, ascii, binary, hex, claude 等
source = "claude"

[render]
fps = 60
color = "green"
shading_mode = 0   # 0=随机, 1=距离头部渐变
bold_mode = 1      # 0=关闭, 1=随机, 2=全部
full_width = false
default_bg = true  # 使用终端默认背景色
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

# Claude Code 集成配置
# transcript_dir 留空则根据 CWD 自动推导
[claude]
enabled = true
transcript_dir = ""
max_chars = 10000
TOML

echo "==> 完成"
echo ""
echo "用法:"
echo "  neo-rainst                     # 默认启动 (claude 模式)"
echo "  neo-rainst --exit-on-key       # 按任意键退出"
echo "  neo-rainst --charset katakana  # 片假名模式"
echo ""
echo "bashrc 函数示例:"
echo '  c() {'
echo '    cc_ds'
echo '    claude --model deepseek-v4-pro --dangerously-skip-permissions --allowedTools all "$@"'
echo '    neo-rainst --exit-on-key'
echo '  }'
echo ""
echo "配置文件: $CONFIG_FILE"
