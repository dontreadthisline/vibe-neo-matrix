# 实现计划

## 执行顺序

### P1: 基线保障
1. `cargo build && cargo test` — 确认 20 tests 通过
2. 保存 `cargo run -- --help` 输出为 golden file
3. 验证: build 成功 + 20 tests pass

### P2: droplet.rs — SENTINEL 消除
4. `head_cur_line`, `tail_cur_line`, `tail_put_line` 改为 `Option<u16>`
5. 更新 `Droplet::new()` / `activate()` / `advance()` / `char_loc()` 的哨兵判断
6. 更新 `cloud.rs` 和 `render.rs` 中所有对 `tail_put_line` 的引用
7. `cargo test` — 确认现有 droplet tests 通过
8. 验证: `cargo test` pass

### P3: dirs crate + 路径简化
9. `cargo add dirs`
10. 替换 `dirs_fallback()` → `dirs::home_dir()`
11. 替换 `xdg_config_path()` → `dirs::config_dir()`
12. 验证: `cargo build` pass

### P4: 参数模型收敛
13. 新建 `src/params.rs` — `SimParams` struct with clap + serde derive
14. 从 `main.rs` 迁移 clap 定义到 `params.rs`
15. 从 `config.rs` 迁移默认值到 `params.rs` 的 `Default` impl
16. 实现 `Params::merge()` — 分层合并 (两层 `Option<T>` 的覆盖语义)
17. 在 `cloud.rs` 实现 `Cloud::apply_params(&SimParams)` — 一次调用替代 15 个 setter
18. 重写 `main.rs` 使用新参数模型
19. `cargo test && cargo run -- --help` 对比 golden file
20. 验证: `--help` 输出一致 + tests pass

### P5: Agent 配置驱动化
21. 新建 `src/transcript/mod.rs` — `AgentConfig`, `TranscriptFormat`, BUILTIN_AGENTS
22. 重命名 `ClaudeCharSource` → `TranscriptCharSource`，接受 `AgentConfig`
23. 在 `config.rs` 新增 `[[agents]]` 解析
24. 保留 `[claude]` 向后兼容映射
25. 更新 `main.rs` 的字符源构建逻辑
26. 新增测试: 自定义 agent TOML 配置加载
27. 验证: 现有 char_source tests 全部通过

### P6: 最终验证
28. `cargo test` — 全部测试通过
29. `cargo build` — 无 warning
30. `cargo run -- --help` — 与 golden file 一致
31. `cargo run -- --charset claude --exit-on-key` — 功能正常
