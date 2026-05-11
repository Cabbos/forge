# DeepSeek API Tool Calling 搜索结果整理

> 搜索关键词：`DeepSeek API tool calling 2026`  
> 搜索时间：2026-04  
> 数据来源：官方文档 + 社区技术文章

---

## 结果一：DeepSeek 官方 API 文档 — Tool Calls

**来源**：<https://api-docs.deepseek.com/guides/tool_calls>  
**类型**：官方文档

### 要点整理

1. **Tool Calls 功能定义**：允许模型调用外部工具来增强自身能力，模型本身不执行具体函数，仅输出结构化的调用请求，由开发者自行执行并返回结果。

2. **非思考模式（Non-thinking Mode）**：
   - 使用 OpenAI 兼容的 `tools` 参数定义工具列表。
   - 工具定义包含 `type: "function"`、`name`、`description` 和 `parameters`（JSON Schema 格式）。
   - 示例代码（Python）展示了查询天气的完整流程：用户提问 → 模型返回函数调用 → 开发者执行函数并回传结果 → 模型用自然语言回复。

3. **思考模式（Thinking Mode）**：从 DeepSeek-V3.2 开始，API 支持在思考模式下使用工具调用（详见过渡到 Thinking Mode 文档）。

4. **Strict Mode（Beta）**：
   - 使用 `base_url="https://api.deepseek.com/beta"` 启用。
   - 工具需设置 `strict: true`，模型将严格遵循 Function 的 JSON Schema 格式输出。
   - 支持丰富的 JSON Schema 类型：`object`、`string`、`number`、`integer`、`boolean`、`array`、`enum`、`anyOf`。
   - `object` 类型的 `additionalProperties` 必须设为 `false`，所有属性必须为 `required`。

---

## 结果二：SegmentFault — Function Calling 完整指南（2026）

**来源**：<https://segmentfault.com/a/1190000047732712>  
**作者**：七牛云行业应用  
**发布日期**：2026-04-27

### 要点整理

1. **核心定义**：Function Calling 是 LLM 与外部工具之间的"契约"——开发者定义可用的操作及其参数结构，模型决定何时调用并输出结构化请求，开发者执行真实操作并回传结果。模型**永远不直接执行任何代码**。

2. **Agentic Loop 原理**：
   - 用户输入 → 模型决定是否调用工具 → 返回结构化调用请求（含函数名和参数）
   - 开发者执行工具 → 将结果回传给模型 → 模型生成最终回复
   - 支持 **并行函数调用（Parallel Function Calling）**，一次返回多个工具调用。

3. **tool_choice 四种取值**：
   - `auto`：模型自主决定是否调用工具
   - `none`：不调用任何工具
   - `required`：强制调用某个工具
   - `{ "type": "function", "function": { "name": "xxx" } }`：指定调用特定工具

4. **三大平台对比**（OpenAI / Claude / DeepSeek）：
   - **OpenAI**：使用 `tools` 参数，`tool_choice` 控制行为，返回 `tool_calls`。
   - **Claude（Anthropic）**：使用 `tools` 参数，但服务端工具与客户端工具拆分方式不同。
   - **DeepSeek**：**与 OpenAI 完全兼容**，可用 OpenAI SDK 直接接入，仅需修改 `base_url` 为 `https://api.deepseek.com`。支持 `strict mode`。

5. **最佳实践**：工具描述（description）要清晰准确，参数名要有语义，避免歧义；合理设置 `required` 字段；优先使用 `strict mode` 确保输出格式合规。

---

## 结果三：掘金 — Spring AI 框架升级：Function Calling 废弃，被 Tool Calling 取代

**来源**：<https://juejin.cn/post/7470423971310436390>  
**作者**：房杰  
**发布日期**：2025-02-12

### 要点整理

1. **趋势变化**：Spring AI 框架从 Function Calling 升级为 Tool Calling，反映了 AI 行业从"函数调用"到"工具调用"的概念演进。"Tool Calling" 是比 "Function Calling" 更广义的概念，不仅仅支持函数，还支持各种类型的工具。

2. **DeepSeek 在 Spring AI 中的集成**：Spring AI 框架深度支持多个大模型平台的 Tool Calling，包括 OpenAI、Anthropic、DeepSeek 等，DeepSeek 凭借其 OpenAI 兼容 API 可无缝接入。

3. **Tool Calling 相比 Function Calling 的优势**：
   - 更灵活的工具定义和组合方式
   - 更好的并行工具调用支持
   - 更丰富的返回值处理机制

4. **实际应用场景**：文章展示了 Tool Calling 在 Spring AI 中的完整代码实现，包括工具定义、注册、调用链路和结果处理，为 Java 生态的 AI Agent 开发提供了参考。

---

## 总结

| 维度 | 结论 |
|------|------|
| **API 兼容性** | DeepSeek API 与 OpenAI API 完全兼容，支持 `tools` / `tool_calls` 参数 |
| **支持模式** | 非思考模式 + 思考模式（V3.2+）均支持 Tool Calls |
| **特色功能** | Strict Mode（Beta）确保严格遵循 JSON Schema，支持丰富的数据类型校验 |
| **生态集成** | 可被 OpenAI SDK 直接调用，也被 Spring AI 等框架深度集成 |
| **2026 年进展** | DeepSeek V4 发布后，Agent 和代码能力大幅提升，Tool Calling 在实际 Agent 场景中表现优异 |
