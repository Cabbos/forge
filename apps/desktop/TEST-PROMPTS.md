# DeepSeek Agent 全场景测试

## 1. 基础工具链

```
在当前目录创建 hello.py，写一个函数检查回文字符串，用 shell 跑 python hello.py 验证。
如果通过就写一个 TEST-RESULT.md 记录结果。
```

**覆盖**: write_to_file → run_shell → write_to_file

---

## 2. 代码探索

```
先读 src-tauri/Cargo.toml 看用了哪些依赖，然后搜 src-tauri/src/ 下所有 .rs 文件中包含 "unwrap()" 的行，把结果写入 UNWRAP-AUDIT.md。
```

**覆盖**: read_file → search_content → write_to_file

---

## 3. 多文件编辑

```
在 src-tauri/src/executor/mod.rs 的注释里加一行 "// TEST: audit marker"，然后用 grep 确认添加成功。
```

**覆盖**: read_file → edit_file → run_shell(grep)

---

## 4. Web 搜索

```
用 web_search 搜索 "DeepSeek API tool calling 2026"，整理前三条结果的要点，写入 WEB-SEARCH-RESULT.md。
```

**覆盖**: web_search → write_to_file

---

## 5. 目录 + Shell复合

```
列出 src-tauri/src/harness/ 目录的所有文件，然后用 wc -l 统计每个 .rs 文件的行数，输出汇总表格。
```

**覆盖**: list_directory → run_shell(wc -l)

---

## 6. 思考过程

```
解析这道题：一个 5x5 迷宫，起点(0,0)终点(4,4)，中间有3堵墙。请先用 thinking 推理算法选择，再写代码求解。要求输出路径坐标。
```

**覆盖**: thinking（展开+点动画+shimmer）→ write_to_file → run_shell

---

## 7. 错误处理

```
用 web_fetch 抓取 https://this-domain-does-not-exist-12345.com，告诉我发生了什么。
```

**覆盖**: web_fetch 错误 → ToolCard 红色 XCircle 状态

---

## 8. 大文件读取

```
读取 src-tauri/Cargo.lock 的前 50 行，然后告诉我这个文件有多大。
```

**覆盖**: read_file offset/limit + run_shell(ls -lh)

---

## 9. 多轮串联

```
1. 列出 src/components/messages/ 目录
2. 读取每个 .tsx 文件的第 1 行
3. 统计里面用了哪些 Lucide 图标
4. 把统计结果写入 ICON-AUDIT.md
```

**覆盖**: list_directory → read_file × N → search_content → write_to_file

---

## 10. Shell 权限确认

```
执行这个 shell 命令：echo "test ok" > /tmp/agent-test.txt && cat /tmp/agent-test.txt
```

**覆盖**: run_shell → ConfirmCard 弹窗 → 用户 Allow/Deny

---

## UI 专项验证

**消息流**：用户消息右侧琥珀色气泡，AI 消息左侧暗色气泡，Tool/Shell/Thinking 左对齐 40px 缩进

**动画**：Thinking 三点脉冲 + 文字 shimmer，Tool running 橙色旋转

**滚动**：消息多到溢出时正常滚动，"↓" 按钮出现/消失

**HubPanel**：点右上角 PanelRightOpen 图标 → 右侧磨砂滑出

**Sidebar**：hover 展开/离开收起，session 圆点点击切换
