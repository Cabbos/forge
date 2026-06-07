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
└── README.md              # 本文件
```
