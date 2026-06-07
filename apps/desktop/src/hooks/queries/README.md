# Forge Query Layer 规范

本目录集中管理所有 TanStack Query（React Query）hooks。Forge 采用 **Zustand + TanStack Query 混合架构**：

- **Zustand** 负责本地状态、流式事件累积、会话 blocks、composer 输入、workspace 切换等高频/实时状态
- **TanStack Query** 负责 IPC 读取缓存、后台刷新、错误统一处理

## 边界规则

| 归 Query | 归 Zustand |
|---|---|
| 从 Rust 后端读取的配置/状态（API keys、capabilities、project runtime） | sessions、blocks、streaming event 累积 |
| 低频只读列表（continuity experiences、forge wiki state） | composer 输入、active workspace、UI 面板展开态 |
| 需要跨组件共享缓存的 IPC 读取 | 需要瞬时响应的本地交互状态 |
| 有明确 domain 生命周期、可被 invalidate 的数据 | transient UI state（loading spinner、modal open） |

**红线**：
- 不要把 streaming/session/transcript 累积链迁到 Query
- 不要把 Query 数据回写 Zustand store
- `memories` 等已被 Zustand store 消费的状态不要直接迁到 Query，否则需要同时重构 store 和所有消费者

**Store 级例外**：
部分 app 级配置读取（如 `loadAppMetadata`）虽在 Zustand `hydrate` 流程中被消费，但仍可迁到 Query，由 `queryClient.fetchQuery()` 提供缓存和错误一致性。这类调用不通过 React Hook，而是使用 QueryClient 的命令式 API，并保持原有的错误兜底行为。

## queryKey 规则

1. **所有 query key 必须集中到 `queryKeys.ts`**，禁止在组件里写硬编码数组
2. Key 结构：`["domain", ...params]`，params 用空字符串兜底 `undefined`/`null`
3. 需要批量 invalidate 时加前缀 key：
   ```ts
   continuityExperiences: (sessionId, projectPath, search?) => [...]
   continuityExperiencesAll: ["continuity-experiences"] // 用于 invalidateQueries 前缀匹配
   ```
4. 依赖变量变化时 Query 自动重取，不需要手动 `refetch`

## Error Handling 规则

1. **Query hooks 禁止吞错**：`queryFn` 里不要 `catch { return [] }` 或 `catch { return null }`
2. 让真实 IPC 异常抛给 React Query，组件通过 `isError` / `error` 消费
3. 组件层统一用 `getQueryErrorMessage(error)` 提取可读文案
4. `lib/tauri.ts` 里的函数已处理浏览器 fallback（`hasTauriRuntime()`），Query 层不需要再兜底

## Invalidation 规则

1. Mutation 后只 invalidate **对应 domain**，不要全局乱扫
2. 用 `queryKeys.xxx` 引用，不要手写数组：
   ```ts
   await queryClient.invalidateQueries({ queryKey: queryKeys.capabilities });
   ```
3. 需要级联更新时，invalidate 多个精确 key，而不是一个宽泛前缀

## 新增 Query Hook 模板

```ts
import { useQuery } from "@tanstack/react-query";
import { someIpcRead } from "@/lib/tauri";
import { queryKeys } from "./queryKeys";

export function useSomeQuery(arg: string, enabled = true) {
  return useQuery<ReturnType>({
    queryKey: queryKeys.someDomain(arg),
    queryFn: async () => {
      return await someIpcRead(arg);
    },
    enabled,
  });
}
```

## 组件消费模板

```ts
const {
  data = [],
  isLoading,
  isError,
  error,
} = useSomeQuery(arg, shouldFetch);

const queryError = getQueryErrorMessage(isError ? error : null);
```

## 目录结构

```
src/hooks/queries/
├── queryKeys.ts           # 集中 query key 定义
├── queryErrors.ts         # getQueryErrorMessage 辅助
├── useApiKeyStatusQuery.ts
├── useAppMetadataQuery.ts
├── useCapabilitiesQuery.ts
├── useContinuityExperiencesQuery.ts
├── useForgeWikiStateQuery.ts
├── useMcpContextSourcesQuery.ts
├── usePreviewFileQuery.ts
├── useProjectCheckpointStatusQuery.ts
├── useProjectRuntimeStatusQuery.ts
├── useSearchWorkspaceFilesQuery.ts
├── useSessionsQuery.ts
└── README.md              # 本文件
```

## IPC Read 调用盘点

基于 `src/lib/tauri.ts` 导出的 read-like IPC，按消费位置分类：

| 函数 | 消费位置 | 状态 | 理由 |
|---|---|---|---|
| `getApiKeyStatus` | `useApiKeyStatusQuery` | **已迁 Query** | 纯配置读取 |
| `loadAppMetadata` | `useAppMetadataQuery` + `hydration.ts` fetchQuery | **已迁 Query** | Store 级例外，命令式缓存 |
| `listSessions` | `useSessionsQuery` + `hydration.ts` fetchQuery | **已迁 Query** | Store 级例外，命令式缓存 |
| `listCapabilities` | `useCapabilitiesQuery` | **已迁 Query** | 纯配置读取 |
| `getProjectRuntimeStatus` | `useProjectRuntimeStatusQuery` | **已迁 Query** | 状态读取 |
| `getProjectCheckpointStatus` | `useProjectCheckpointStatusQuery` | **已迁 Query** | 状态读取 |
| `listMcpContextSources` | `useMcpContextSourcesQuery` | **已迁 Query** | 低频只读列表 |
| `listContinuityExperiences` | `useContinuityExperiencesQuery` | **已迁 Query** | 低频只读列表 |
| `searchContinuityExperiences` | `useContinuityExperiencesQuery` | **已迁 Query** | 低频只读搜索 |
| `searchWorkspaceFiles` | `useSearchWorkspaceFilesQuery` | **已迁 Query** | 搜索类读取 |
| `previewFile` | `usePreviewFileQuery` | **已迁 Query** | 文件预览读取 |
| `getForgeWikiState` | `useForgeWikiStateQuery` | **已迁 Query** | 低频状态读取 |
| `loadSessionTranscript` | `src/store/persistence.ts` | **保留 Zustand** | 红线：session/transcript 累积链 |
| `listMemories` | `src/components/context/WikiSections.tsx` → `setMemories` | **保留 Zustand** | 写入 store，迁 Query 需重构全链路 |
| `getWorkflowState` | 无调用点 | **死代码** | 未使用，可清理 |
| `getDefaultWorkingDir` | 无调用点 | **死代码** | 未使用，可清理 |
| `listPlugins` | 无调用点 | **死代码** | 未使用，可清理 |
| `discoverPlugins` | 无调用点 | **死代码** | 未使用，可清理 |
| `listForgeWikiPages` | 无调用点 | **死代码** | 未使用，可清理 |
| `readForgeWikiPage` | 无调用点 | **死代码** | 未使用，可清理 |
| `selectForgeWikiContext` | 无调用点 | **死代码** | 未使用，可清理 |
| `selectContextMemories` | 无调用点 | **死代码** | 未使用，可清理 |

## 状态机（XState）评估备忘

以下前端状态流未来可能适合 XState，但本阶段不落地：

| 状态流 | 当前位置 | 复杂度 | XState 收益 | 迁移风险 |
|---|---|---|---|---|
| **composer submit / confirm / cancel** | `useComposerSubmit.ts` + `ConfirmCard.tsx` | 中 | 显式化 confirm 生命周期，防止竞态 | 高 — 涉及 streaming、pending confirm 跨组件协调 |
| **session streaming lifecycle** | `store/index.ts` `dispatchOutputEvent` | 高 | 统一 stream_start → chunk → end → error 状态 | 极高 —  backbone 协议，牵一发而动全身 |
| **command palette / modal flow** | `CommandPalette.tsx` + `ForgeCommandDialog` | 低 | 过度设计，当前 useState 足够 | 低 — 但收益不明显 |
| **wiki memory proposal flow** | `WikiSections.tsx` + `useWikiSectionsActions.ts` | 中 | pending / accepted / discarded / busy 状态显式化 | 中 — 局部状态，边界清晰 |
| **file preview loading** | `FilePreviewSheet.tsx` | 低 | loading / loaded / error / action-error 子状态 | 低 — 但当前 Query + useState 已足够 |

**结论**：短期内 none 值得立即引入 XState。wiki proposal flow 是最有潜力的候选，但应在后续独立迭代中评估。
