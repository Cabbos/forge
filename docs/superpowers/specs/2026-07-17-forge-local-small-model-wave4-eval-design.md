# Forge 本地小模型第四波分层评测设计

## 目标

继续使用 CC Switch 当前 `remote-vllm` 配置中的
`qwen3.6-35b-a3b-nvfp4-fast` 评测 Forge Agent。新增六个未参与前 36
次观测的任务，每题独立运行三次，共增加 18 次观测。结果必须区分：

- 模型功能行为；
- Agent 文件范围与 20 轮收敛纪律；
- continuity 与外层评测器问题；
- 原始完整链路结果。

本轮不修改 Forge 产品代码，只生成评测产物、独立复验记录和增量报告。

## 已选方案

采用“六个全新 Continuity 编码任务 × 三次独立批次”。相较于重跑旧题，
该方案增加功能多样性；相较于混合 Gateway/A2A 离线契约用例，它保留真实
远程模型、工具循环、补丁、验证、continuity 和报告链路的可比性。

## 任务矩阵

| 难度 | 任务 ID | 主要能力 | 独立隐藏边界 |
|---|---|---|---|
| 简单 | `continuity-pipeline-normalize-input` | 字符串规范化 | 制表符、换行、中文、数字、纯空白 |
| 简单 | `continuity-pipeline-task-summary` | 状态计数与不变性 | 空列表、混合状态、输入数组不被修改 |
| 中等 | `continuity-pipeline-storage-validation` | 类型守卫与标题规范化 | 大小写非法状态、Unicode/连续空白 |
| 中等 | `continuity-pipeline-priority-labels` | 多语言别名解析 | `p1→high`、`p2→medium`、`p3→low`、未知值 |
| 困难 | `continuity-pipeline-due-date-labels` | 日期分类与格式化 | 带时间戳 ISO、月末、年末与跨年“明天” |
| 困难 | `continuity-pipeline-csv-export` | CSV 序列化 | 逗号、双引号、换行、中文、空列表 |

每层两个不同任务，每题三个重复，故每层六次观测。

## 执行架构

1. 运行前从 CC Switch 数据库读取当前 provider、env base、认证令牌和
   `ANTHROPIC_DEFAULT_OPUS_MODEL`。报告只记录 provider/model 与安全的
   endpoint 元数据，不保存令牌。
2. 调用 `/v1/models` 确认目标模型仍可用。使用已连通的 env base，
   不使用与其冲突的顶层 `apiBase`。
3. 在系统临时目录生成只包含六个任务的临时 case pack；fixture 使用绝对路径，
   不写入仓库。
4. 用 Forge eval runner 顺序执行 R1、R2、R3 三个独立批次。每个批次包含六题，
   单题超时 120 秒，模型预算 20 轮。
5. 每个批次立即检查 JSON 可解析性、trace 数量、任务集合、patch replay、
   sandbox scrub 和敏感信息。
6. 对每条 trace 在全新临时 fixture 中重新执行 setup、应用补丁、依赖水合、
   `npm test`、`npx tsc --noEmit` 与独立隐藏 oracle。
7. 汇总本轮 18 次，并与既有 36 次描述性合并为 18 个不同任务、54 次观测。

## 环境映射

运行时显式设置：

- `FORGE_HEADLESS_PROVIDER=custom_openai`
- `FORGE_HEADLESS_MODEL=<CC Switch 目标模型>`
- `FORGE_CUSTOM_OPENAI_BASE_URL=<CC Switch env base>/v1`
- `FORGE_CUSTOM_OPENAI_API_KEY=<CC Switch 认证令牌>`
- `FORGE_CUSTOM_OPENAI_MODEL=<CC Switch 目标模型>`

令牌只存在于子进程环境。命令输出、JSON 产物和报告都必须通过敏感信息扫描。
CC Switch 的 `temperature=0.7` 若仍未由 Forge 请求显式传播，继续作为配置
缺口记录，不在本轮暗中改变运行时。

## 判定口径

- **功能通过**：补丁可重放、项目测试通过、类型检查通过、隐藏 oracle 通过。
- **范围通过**：Rust `forge_run_evidence.changed_files` 全部属于任务
  `expected_files_changed`，且不命中 `forbidden_files_changed`。
- **预算通过**：`model_rounds <= 20`。
- **Agent 纪律通过**：功能、范围、预算同时通过。
- **Continuity 通过**：任务的最终 continuity assertion 通过。
- **原始链路通过**：`trace.error IS NULL`。

Python workspace observer 中的依赖目录噪声只作为数据质量信号，不替代 Rust
运行时 changed-files 判定。

## 失败归因

每条失败按以下顺序标注，可保留“混合失败”：

1. 隐藏 oracle 或项目验证失败：模型功能问题。
2. 功能通过但范围越界或超过 20 轮：模型/Agent 纪律问题。
3. 出现 `no such column: project_path`：continuity 数据库迁移问题。
4. 没有迁移错误但缺少要求的 event/experience/reflection：continuity 证据形成问题。
5. Agent 纪律通过但原始链路失败：评测器误报。
6. Continuity 通过但模型超预算：系统链路可用，同时保留模型收敛失败。

## 产物

三个批次分别写入：

- `apps/desktop/artifacts/eval-runs/2026-07-18-wave4-fresh-continuity-r1-forge.json`
- `apps/desktop/artifacts/eval-runs/2026-07-18-wave4-fresh-continuity-r2-forge.json`
- `apps/desktop/artifacts/eval-runs/2026-07-18-wave4-fresh-continuity-r3-forge.json`

报告增量保留既有全部图表与表格，并新增：

- 本轮五层质量漏斗；
- 六题三次稳定性表；
- 18 次运行审计表；
- 54 次合并难度图；
- 模型问题、评测器问题和混合失败的数量与证据。

## 错误处理

- endpoint 或模型不可用：停止远程调用，不生成伪结果。
- 单个批次退出非零但产物完整：保留产物并继续独立复验；退出原因进入报告。
- 产物缺 trace、任务集合错误或 JSON 损坏：该批次重跑一次，不合并不完整数据。
- continuity 故障不阻止补丁功能复验；两类结论分别报告。
- 独立复验无法执行：对应功能结果标为“未验证”，不推断通过。

## 验收标准

- 三个批次各包含恰好六个目标任务，合计 18 条新 trace。
- 18 条 trace 均完成 patch replay、sandbox scrub 和独立复验，或明确记录不可
  复验原因。
- 所有百分比可由保存的 JSON 与独立 oracle 记录重新计算。
- 报告明确区分模型功能、Agent 纪律、continuity、评测器误报和原始链路。
- 报告 Artifact 通过来源 SQL、敏感信息和结构校验后只渲染一次。
- 不覆盖或提交用户已有的无关工作区修改。

## 非目标

- 不修复 continuity 迁移、Agent 提示或 provider 实现。
- 不调优模型参数以提高本轮分数。
- 不把基线组与隐藏-oracle 组视为同质统计样本。
