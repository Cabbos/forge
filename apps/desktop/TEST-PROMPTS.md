# Harness 工具系统测试提示词

## Level 1 — 基础工具链

```
在当前目录下创建一个 hello.py，写入一个斐波那契函数。然后搜索项目中所有引用 "read_file" 的代码行，最后用 shell 跑一下这个 python 文件看看输出。
```

**验证点**: write_to_file → search_content → run_shell 三个工具连续调用，结果回填 API。

---

```
读取 src-tauri/Cargo.toml，告诉我这个项目用到了哪些 Rust 依赖。
```

**验证点**: read_file 单工具，返回内容被正确格式化给 API。

---

```
搜索 src-tauri/src/ 下所有 .rs 文件，找找有没有包含 "unwrap" 的代码行。
```

**验证点**: search_files (glob) + search_content (grep)。

---

## Level 2 — 权限 + 多轮工具

```
帮我做三件事：
1. 在 src-tauri/src/harness/ 下创建一个 AUDIT.md
2. 写一段内容描述你看到的 harness 模块结构
3. 确认写入前弹出权限确认
```

**验证点**: write_to_file 触发 PermissionGate → ConfirmAsk 弹窗 → 用户 Allow → 写入成功。

---

```
执行命令 ls -la /tmp，然后告诉我输出里有没有以 "claude" 开头的文件。
```

**验证点**: run_shell 权限确认，stdout 返回给 API。

---

```
用 web_search 搜索 "DeepSeek V4 Flash API rate limit 2026"，把前三条结果整理输出。
```

**验证点**: web_search (DuckDuckGo)，结构化结果解析。

---

## Level 3 — 全量串联

```
做一个完整的代码审计：
1. 读取 src-tauri/src/harness/mod.rs 和 src-tauri/src/agent/session.rs
2. 搜索项目里所有的 "unwrap()" 调用
3. 搜索项目里所有的 ".lock().unwrap()" 持有模式
4. 把审计结果写入 AUDIT-REPORT.md
5. 用 shell 跑 cargo build --manifest-path src-tauri/Cargo.toml 验证项目还能编译
```

**验证点**: 5 工具串联，多轮 tool call，权限弹窗，Hook 日志记录，耗时统计。

---

```
用 web_search 搜索 "rust harness agent framework 2026"，取前三条结果的 URL，用 web_fetch 抓取每篇的正文前 200 字，总结共性，把总结写入 WEB-AUDIT.md。
```

**验证点**: web_search → web_fetch × 3 → write_to_file。多轮网络 IO + 文件写入。

---

## Level 4 — 权限边界

```
执行这个命令：echo "hello" > /tmp/test.txt && cat /tmp/test.txt
```

**验证点**: run_shell 权限确认，复合命令通过。

---

```
执行命令：rm -rf /tmp/nonexistent-test-dir
```

**验证点**: run_shell 不匹配危险模式时应允许执行，返回 stderr。

---

```
尝试写入 /etc/test-agent-file（应该被拒绝）
```

**验证点**: write_to_file 到系统目录应触发 PermissionGate → Deny。

---

## Level 5 — Thinking + Streaming

```
解释一下 Rust 的 async/await 和 Tokio 运行时之间的关系。先思考，再给出结构化答案。
```

**验证点**: thinking block 出现 + 展开/折叠 + text 流式渲染。

---

```
在当前项目里搜索所有 Rust 文件，数一数一共有多少个 impl 块，统计每个模块的 impl 数量，输出到一个表格。
```

**验证点**: search_files → read_file × N → write_to_file。多文件读取 + 聚合输出。
