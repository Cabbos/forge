# Architecture

## 系统架构

```mermaid
flowchart TD
    subgraph Input
        T[tasks/sample_tasks.json<br/>EvaluationTask[]]
    end

    subgraph Core
        R[DeterministicMockRunner<br/>生成确定性 trace]
        M[calculate_metrics()<br/>纯函数，无副作用]
        S[InMemoryStorage<br/>进程内存储边界]
    end

    subgraph Output
        AT[AgentTrace<br/>tool_calls, shell_outputs,<br/>file_diffs, verification_result,<br/>failure_category]
        MS[MetricsSummary<br/>success_rate,<br/>verification_coverage,<br/>failure_categories]
    end

    subgraph API["FastAPI Service"]
        H["GET /health"]
        TL["GET /tasks"]
        CR["POST /runs"]
        GR["GET /runs/{id}"]
        GT["GET /runs/{id}/trace"]
        GM["GET /runs/{id}/metrics"]
    end

    subgraph Deliverables
        OD[OpenAPI 自动文档]
        DF[Dockerfile + docker-compose]
        PT[pytest 测试覆盖]
        RF[ruff 代码质量]
    end

    T --> S
    S --> R
    R --> AT
    AT --> M
    M --> MS

    S --> TL
    S --> CR
    S --> GR
    AT --> GT
    MS --> GM

    CR --> R
    R --> S

    API --> OD
    API --> DF
    API --> PT
    API --> RF
```

## Trace 数据流

```mermaid
flowchart LR
    subgraph Task["EvaluationTask"]
        P[prompt]
        CF[context_files]
        VC[verification_command]
        ES[expected_success]
    end

    subgraph Runner["DeterministicMockRunner"]
        TC[tool_calls<br/>read_context + edit_files]
        SO[shell_outputs<br/>verification command]
        FD[file_diffs<br/>mock patch]
    end

    subgraph Trace["AgentTrace"]
        VR[VerificationResult<br/>command, passed,<br/>exit_code, stdout, stderr]
        FR[failure_reason]
        FC[failure_category<br/>枚举：none / verification_failed /<br/>no_verification / runner_error /<br/>tool_error / timeout]
    end

    Task --> Runner
    Runner --> Trace
    VC --> VR
    ES --> VR
    VR --> FR
    FR --> FC
```

## Metrics 计算

```mermaid
flowchart TD
    AT[AgentTrace[]] --> CF{trace_passed?}
    CF -->|yes| P[passed = true]
    CF -->|no| F[passed = false<br/>记录 failure_category]

    P --> TM[TaskMetric]
    F --> TM

    TM --> AGG[聚合计算]
    AGG --> SR[success_rate<br/>passed / total]
    AGG --> VC[verification_coverage<br/>有验证的任务 / total]
    AGG --> ATC[average_tool_calls]
    AGG --> FC[failure_categories<br/>各类型计数]
    AGG --> PT[per-task pass/fail]
```

## Forge + forge-eval-runner 关系

```mermaid
flowchart TB
    subgraph Forge["Forge — Agent 产品工作台"]
        direction TB
        F1[用户意图] --> F2[项目上下文]
        F2 --> F3[Agent Loop]
        F3 --> F4[工具执行]
        F4 --> F5[权限确认]
        F5 --> F6[验证证据]
        F6 --> F7[项目档案]
    end

    subgraph Eval["forge-eval-runner — Agent 评测基础设施"]
        direction TB
        E1[任务定义] --> E2[Eval Run]
        E2 --> E3[AgentTrace]
        E3 --> E4[Metrics]
        E4 --> E5[Failure Analysis]
    end

    subgraph Proof["共同证明"]
        direction TB
        P1[可执行<br/>Agent 能做事]
        P2[可观测<br/>执行过程可追溯]
        P3[可验证<br/>结果有证据]
    end

    Forge --> Proof
    Eval --> Proof
```

## 为什么用 Mock Runner

```mermaid
flowchart LR
    subgraph Phase1["Phase 1（当前）"]
        M1[DeterministicMockRunner]
        M2[固定 trace shape]
        M3[稳定 API contract]
        M4[完整测试覆盖]
    end

    subgraph Phase2["Phase 2（后续）"]
        R1[RealProviderAdapter]
        R2[相同 trace shape]
        R3[相同 API contract]
        R4[真实模型执行]
    end

    Phase1 -->|替换 runner 实现| Phase2
```

## 技术栈

| 层 | 选型 | 说明 |
|---|---|---|
| API 框架 | FastAPI | 自动生成 OpenAPI 文档 |
| 数据校验 | Pydantic v2 | ConfigDict(extra="forbid") 严格模式 |
| 包管理 | uv | 快速、确定性依赖解析 |
| 测试 | pytest | test_api / test_runner / test_metrics |
| 代码质量 | ruff | lint + format |
| 容器化 | Dockerfile + docker-compose | 一键启动 |
| Runner | DeterministicMockRunner | 确定性输出，可复现 |
