# Quality Guidelines

> Code quality standards for neo-rainst backend (Rust + ratatui).

---

## Gotchas

### Claude Code project directory naming

Claude Code 将 CWD 转换为 `~/.claude/projects/<slug>/` 目录名时，不仅把 `/` 转成 `-`，**也把 `_` 转成 `-`**。

```rust
// WRONG — 只替换 /，underscore 保留导致目录不存在
let dir_name = abs.to_string_lossy().replace('/', "-");
// /home/user/my_proj → -home-user-my_proj  (NOT FOUND)

// CORRECT — 同时归一化 / 和 _
let dir_name = abs.to_string_lossy().replace('/', "-").replace('_', "-");
// /home/user/my_proj → -home-user-my-proj  (FOUND)
```

**验证方式**：`ls ~/.claude/projects/` 列出实际目录名，对照推断结果。

### Shell alias 参数透传陷阱

Bash alias 把额外参数追加到展开文本的**末尾**，不是追加到中间命令：

```bash
# WRONG — --resume 参数漏给了 neo-rainst
alias c='cc_ds && claude ... ;neo-rainst'
# c --resume <id> → cc_ds && claude ... ;neo-rainst --resume <id>

# CORRECT — 用函数 + "$@" 透传
c() {
  cc_ds
  claude ... "$@"
  neo-rainst --config ... --exit-on-key
}
```

---

## Testing Requirements

### CharSource 实现必须覆盖的测试

每个 `CharSource` 实现至少需要：

| 测试点 | 说明 |
|--------|------|
| 路径归一化 | `transcript_dir_from_cwd()` 中 `/` 和 `_` → `-` |
| 文本提取 | `extract_text_from_entry()` 对 user/assistant 消息正确提取 |
| system 类型过滤 | attachment、file-history-snapshot 等不被纳入字符池 |
| 空目录/不存在目录 | `reload()` 返回空 chars 而非 panic |
| max_chars 截断 | 超长输入只保留尾部 |
| 重复 reload 跳过 | mtime 不变时不做无意义重解析 |

### 测试先行规则

对纯函数（路径转换、文本提取、状态转换）**必须先写测试再实现**：

```
写测试 → 测试失败(红) → 实现代码 → 测试通过(绿)
```

反模式：
```
实现代码 → 编译 → 用户发现问题 → 回头补测试
```

---

## Code Review Checklist

- [ ] `cargo test` 全部通过
- [ ] `cargo clippy` 零警告
- [ ] `cargo build --release` 无错误
- [ ] 新增 CharSource 或路径逻辑有对应单元测试
- [ ] shell 集成（alias/function）参数透传正确，不出现参数泄漏
- [ ] 配置文件路径使用绝对路径或明确推导逻辑，不依赖隐式 CWD
