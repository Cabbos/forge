import { test, expect, type Page } from "@playwright/test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { simulateStream, fullConversation } from "../mock-ipc";
import type { WorkflowState } from "../../src/lib/protocol";

/** Setup: inject mock IPC before the app loads */
export async function setup(page: Page, options?: { workingDir?: string | null }) {
  const initialWorkingDir = options && "workingDir" in options ? options.workingDir : "/Users/cabbos/project/forge";
  await page.addInitScript(({ initialWorkingDir }) => {
    let callbackId = 0;
    const callbacks = new Map<number, (data: unknown) => void>();
    const workingDir = initialWorkingDir ?? "/Users/cabbos/project/forge";
    const sessionWorkingDirs = new Map<string, string>();
    if (initialWorkingDir === null) {
      window.localStorage.removeItem("forge-working-dir");
    } else {
      window.localStorage.setItem("forge-working-dir", workingDir);
    }
    const projectRuntimeStatus = {
      working_dir: workingDir,
      has_package_json: true,
      package_manager: "npm",
      dev_script: "dev",
      command: "npm run dev",
      port: 1420,
      url: "http://localhost:1420",
      running: false,
      managed: false,
      pid: null,
      can_start: true,
      can_stop: false,
      can_open: true,
      message: "Preview not running",
      logs: [],
    };
    const projectCheckpointStatus = {
      working_dir: workingDir,
      is_git_repo: true,
      dirty: false,
      last_checkpoint: null,
      message: "No checkpoint yet",
    };
    const diagnosticsReport = () => ({
      ok: true,
      generatedAtMs: Date.now(),
      checks: [
        {
          id: "config",
          label: "配置文件",
          status: "pass",
          message: "Forge config is readable.",
          detail: { path: "~/.forge/config.json" },
        },
        {
          id: "gateway_service",
          label: "Gateway service",
          status: "pass",
          message: "Gateway service is installed and running.",
          detail: { backend: "launchd", service_id: "com.forge.gateway" },
        },
      ],
    });
    const gatewayRuntimeStatus = () => ({
      ok: true,
      message: "Gateway runtime is healthy.",
      uptime_seconds: 125,
      active_sessions: 0,
      pending_triggers: 0,
      pending_session_inputs: 0,
      claimed_triggers: 0,
      dead_letter_runs: 0,
      recent_runs: [],
      recent_session_inputs: [],
      runtime_tasks: [
        {
          name: "webhook_listener",
          running: true,
          last_started_at_ms: Date.now() - 60_000,
          last_error: null,
        },
        {
          name: "scheduler_tick",
          running: true,
          last_started_at_ms: Date.now() - 30_000,
          last_error: null,
        },
      ],
    });
    const serviceStatus = () => {
      // @ts-expect-error acceptance mock
      if (window.__mockServiceStatus && typeof window.__mockServiceStatus === "object") {
        // @ts-expect-error acceptance mock
        return window.__mockServiceStatus as Record<string, unknown>;
      }
      return {
        installed: true,
        running: true,
        message: "Gateway service is installed and running.",
        supported: true,
        backend: "launchd",
        service_id: "com.forge.gateway",
        label: "com.forge.gateway",
        launch_domain: "gui/501",
        service_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist",
        plist_path: "/Users/test/Library/LaunchAgents/com.forge.gateway.plist",
        log_path: "/Users/test/.forge/logs/gateway.log",
        error_log_path: "/Users/test/.forge/logs/gateway-error.log",
        status_message: "Service 'com.forge.gateway' is running.",
      };
    };
    const mcpContextSources = {
      resources: [
        {
          server_id: "obsidian",
          uri: "file:///notes/forge.md",
          name: "Forge 研发记录",
          description: "Obsidian 中的项目研发记录。",
          mime_type: "text/markdown",
        },
      ],
      prompts: [
        {
          server_id: "linear",
          name: "summarize_issue",
          description: "整理当前任务风险。",
          arguments: [{ name: "focus", description: "关注点", required: false }],
        },
      ],
    };
    let forgeWikiExists = false;
    const forgeWikiPages = [
      {
        id: "index",
        project_path: workingDir,
        path: "index.md",
        title: "项目概览",
        kind: "index",
        summary: "项目目标、边界和当前结构。",
        updated_at: "2026-05-13T00:00:00.000Z",
        token_estimate: 120,
      },
      {
        id: "tasks",
        project_path: workingDir,
        path: "tasks.md",
        title: "当前任务",
        kind: "tasks",
        summary: "当前任务、验收步骤和后续事项。",
        updated_at: "2026-05-13T00:00:00.000Z",
        token_estimate: 120,
      },
      {
        id: "decisions",
        project_path: workingDir,
        path: "decisions.md",
        title: "决策记录",
        kind: "decisions",
        summary: "重要方案和取舍。",
        updated_at: "2026-05-13T00:00:00.000Z",
        token_estimate: 120,
      },
    ];
    const forgeWikiProposals = new Map<string, Record<string, unknown>>();
    const forgeWikiState = (projectPath: string, exists: boolean) => ({
      project_path: projectPath,
      exists,
      wiki_dir: `${projectPath}/.forge/wiki`,
      pages: exists ? forgeWikiPages.map((page) => ({ ...page, project_path: projectPath })) : [],
      message: exists ? "项目记录已就绪。" : "还没有项目记录",
    });
    const forgeWikiProposal = (projectPath: string, args: Record<string, unknown>) => ({
      id: String(args.proposalId ?? args.id ?? "forge-wiki-proposal"),
      project_path: projectPath,
      session_id: typeof args.sessionId === "string" ? args.sessionId : null,
      target_pages: Array.isArray(args.targetPages) ? args.targetPages.map(String) : ["tasks.md"],
      title: String(args.title ?? "记录项目进展"),
      summary: String(args.summary ?? "补充本轮任务产生的项目记录。"),
      patch_preview: typeof args.patchPreview === "string" ? args.patchPreview : null,
      status: "pending",
      created_at: "2026-05-13T00:00:00.000Z",
    });
    const permissionRules = () => {
      // @ts-expect-error mock
      if (!Array.isArray(window.__mockPermissionRules)) window.__mockPermissionRules = [];
      // @ts-expect-error mock
      return window.__mockPermissionRules as Array<Record<string, unknown>>;
    };
    const permissionRule = (toolName: string, decision: string) => ({
      tool_name: toolName,
      decision,
      created_at: "2026-06-16T00:00:00.000Z",
    });
    const capabilityEnabledState = () => {
      // @ts-expect-error acceptance mock
      if (!window.__mockCapabilityEnabled || typeof window.__mockCapabilityEnabled !== "object") {
        // @ts-expect-error acceptance mock
        window.__mockCapabilityEnabled = {
          read_file: true,
          "code-review": true,
          "mcp:obsidian": true,
          "hook:pre-commit": true,
        };
      }
      // @ts-expect-error acceptance mock
      return window.__mockCapabilityEnabled as Record<string, boolean>;
    };
    const mockCapabilities = () => {
      const enabled = capabilityEnabledState();
      return [
        { id: "read_file", name: "File Reader", description: "Read files", kind: "tool", source: "builtin", version: "1.0", enabled: enabled.read_file !== false },
        { id: "code-review", name: "Code Review", description: "Review code", kind: "skill", source: "github", version: "1.2", enabled: enabled["code-review"] !== false },
        { id: "mcp:obsidian", name: "Obsidian MCP", description: "Project notes", kind: "mcp_server", source: "~/.forge/mcp.json", version: "local", enabled: enabled["mcp:obsidian"] !== false },
        { id: "hook:pre-commit", name: "Pre-commit Hook", description: "Run checks before commit", kind: "hook", source: "local", version: "1", enabled: enabled["hook:pre-commit"] !== false },
      ];
    };
    const mockEcosystemItems = () => {
      const enabled = capabilityEnabledState();
      return [
        { id: "read_file", name: "File Reader", description: "Read files", kind: "tool", source: "builtin", version: "1.0", enabled: enabled.read_file !== false, status: "healthy", statusMessage: "Built-in tool is available.", configurable: false, configSummary: null },
        { id: "code-review", name: "Code Review", description: "Review code", kind: "skill", source: "github", version: "1.2", enabled: enabled["code-review"] !== false, status: "healthy", statusMessage: "Skill metadata loaded.", configurable: false, configSummary: null },
        { id: "mcp:obsidian", name: "Obsidian MCP", description: "Project notes", kind: "mcp_server", source: "~/.forge/mcp.json", version: "local", enabled: enabled["mcp:obsidian"] !== false, status: "warning", statusMessage: "Token is missing.", configurable: true, configSummary: "command: obsidian-mcp --stdio" },
        { id: "hook:pre-commit", name: "Pre-commit Hook", description: "Run checks before commit", kind: "hook", source: "local", version: "1", enabled: enabled["hook:pre-commit"] !== false, status: "healthy", statusMessage: "Hook is installed.", configurable: false, configSummary: null },
      ];
    };
    const mockToolInventory = () => [
      { id: "read_file", name: "Read File", description: "Read files", kind: "builtin", source: "forge", enabled: true },
      { id: "write_to_file", name: "Write File", description: "Write files", kind: "builtin", source: "forge", enabled: true },
      { id: "obsidian.search", name: "Search Notes", description: "Search Obsidian notes", kind: "mcp", source: "mcp:obsidian", enabled: false },
    ];
    const schedulerTasks = () => {
      // @ts-expect-error mock
      if (!Array.isArray(window.__mockScheduledTasks)) window.__mockScheduledTasks = [];
      // @ts-expect-error mock
      return window.__mockScheduledTasks as Array<Record<string, unknown>>;
    };
    const schedulerHistory = () => {
      // @ts-expect-error mock
      if (!Array.isArray(window.__mockSchedulerHistory)) window.__mockSchedulerHistory = [];
      // @ts-expect-error mock
      return window.__mockSchedulerHistory as Array<Record<string, unknown>>;
    };
    const schedulerTaskFromInput = (input: Record<string, unknown>, existing?: Record<string, unknown>) => {
      const now = Date.now();
      const intervalSeconds = Math.max(0, Number(input.interval_seconds ?? existing?.interval_seconds ?? 3600));
      const id = String(input.id ?? existing?.id ?? crypto.randomUUID());
      const createdAt = Number(existing?.created_at_ms ?? now);
      return {
        id,
        title: String(input.title ?? existing?.title ?? "Scheduled task"),
        text: String(input.text ?? existing?.text ?? ""),
        enabled: Boolean(existing?.enabled ?? true),
        interval_seconds: intervalSeconds,
        next_run_at_ms: intervalSeconds > 0 ? now + intervalSeconds * 1000 : 0,
        last_run_at_ms: existing?.last_run_at_ms ?? null,
        created_at_ms: createdAt,
        updated_at_ms: now,
        tags: Array.isArray(input.tags) ? input.tags.map(String) : Array.isArray(existing?.tags) ? existing.tags.map(String) : [],
        profile_id: typeof input.profile_id === "string" && input.profile_id ? input.profile_id : null,
        last_error: null,
      };
    };
    const a2aStates = () => {
      // @ts-expect-error mock
      if (!window.__mockA2AStates || typeof window.__mockA2AStates !== "object") window.__mockA2AStates = {};
      // @ts-expect-error mock
      return window.__mockA2AStates as Record<string, Record<string, unknown>>;
    };
    const reviewAgentA2AState = (args: Record<string, unknown>) => {
      const sessionId = String(args.sessionId ?? "");
      const state = a2aStates()[sessionId];
      if (!state || !Array.isArray(state.tasks)) throw new Error(`A2A state not found: ${sessionId}`);
      const decision = String(args.decision ?? "approve");
      const reviewedDecision = decision === "reject" ? "rejected" : "approved";
      const taskIds = new Set(Array.isArray(args.taskIds) ? args.taskIds.map(String) : []);
      const now = Date.now();
      const reviewMessage = decision === "reject" ? "Review rejected" : "Review approved";
      const tasks = state.tasks.map((rawTask) => {
        const task = rawTask as Record<string, unknown>;
        if (!taskIds.has(String(task.task_id))) return task;
        const messages = Array.isArray(task.messages) ? task.messages.slice() : [];
        messages.push({
          message_id: `mock-review-${String(task.task_id)}-${now}`,
          kind: decision === "reject" ? "failed" : "progress",
          content: reviewMessage,
          created_at_ms: now,
        });
        return {
          ...task,
          status: decision === "reject" ? "failed" : task.status,
          needs_human_review: false,
          review_decision: reviewedDecision,
          reviewed_at_ms: now,
          latest_message: reviewMessage,
          messages,
          failure_kind: decision === "reject" ? "review_rejection" : task.failure_kind ?? null,
          failure_message: decision === "reject" ? String(args.message ?? "Review rejected") : task.failure_message ?? null,
          retryable: decision === "reject" ? false : task.retryable ?? null,
          suggested_action: decision === "reject"
            ? "Review rejected by controller. Do not merge this worktree."
            : "Review approved by controller.",
        };
      });
      const nextState = {
        ...state,
        tasks,
        running_count: tasks.filter((task) => task.status === "running").length,
        completed_count: tasks.filter((task) => task.status === "completed").length,
        failed_count: tasks.filter((task) => task.status === "failed").length,
        interrupted_count: tasks.filter((task) => task.status === "interrupted").length,
      };
      a2aStates()[sessionId] = nextState;
      // @ts-expect-error mock
      window.__lastReviewAgentA2ATasksArgs = args;
      return {
        session_id: sessionId,
        source: "live",
        state: nextState,
      };
    };
    const openKeyvalDb = async () => {
      let db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store");
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
        request.onupgradeneeded = () => {
          const database = request.result;
          if (!database.objectStoreNames.contains("keyval")) database.createObjectStore("keyval");
        };
      });
      if (db.objectStoreNames.contains("keyval")) return db;

      const nextVersion = db.version + 1;
      db.close();
      db = await new Promise<IDBDatabase>((resolve, reject) => {
        const request = indexedDB.open("keyval-store", nextVersion);
        request.onerror = () => reject(request.error);
        request.onsuccess = () => resolve(request.result);
        request.onupgradeneeded = () => {
          const database = request.result;
          if (!database.objectStoreNames.contains("keyval")) database.createObjectStore("keyval");
        };
      });
      return db;
    };
    const readKeyval = async <T,>(key: string): Promise<T | null> => {
      try {
        const db = await openKeyvalDb();
        const value = await new Promise<T | null>((resolve, reject) => {
          const tx = db.transaction("keyval", "readonly");
          const request = tx.objectStore("keyval").get(key);
          request.onerror = () => reject(request.error);
          request.onsuccess = () => resolve((request.result ?? null) as T | null);
        });
        db.close();
        return value;
      } catch {
        return null;
      }
    };
    const writeKeyval = async (key: string, value: unknown) => {
      try {
        const db = await openKeyvalDb();
        await new Promise<void>((resolve, reject) => {
          const tx = db.transaction("keyval", "readwrite");
          tx.objectStore("keyval").put(value, key);
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        });
        db.close();
      } catch {
        // Tests that do not need persistence should not fail setup because of IndexedDB.
      }
    };
    const deleteKeyval = async (key: string) => {
      try {
        const db = await openKeyvalDb();
        await new Promise<void>((resolve, reject) => {
          const tx = db.transaction("keyval", "readwrite");
          tx.objectStore("keyval").delete(key);
          tx.oncomplete = () => resolve();
          tx.onerror = () => reject(tx.error);
        });
        db.close();
      } catch {
        // Tests that do not need persistence should not fail setup because of IndexedDB.
      }
    };
    const saveAppMetadataToIndexedDb = async (metadata: Record<string, unknown>) => {
      await writeKeyval("forge-workspaces", Array.isArray(metadata.workspaces) ? metadata.workspaces : []);
      if (typeof metadata.activeWorkspaceId === "string") await writeKeyval("forge-active-workspace", metadata.activeWorkspaceId);
      else await deleteKeyval("forge-active-workspace");
      if (typeof metadata.activeSessionId === "string") await writeKeyval("forge-active-session", metadata.activeSessionId);
      else await deleteKeyval("forge-active-session");
      if (typeof metadata.selectedProvider === "string") await writeKeyval("forge-provider", metadata.selectedProvider);
      if (typeof metadata.selectedModel === "string") await writeKeyval("forge-model", metadata.selectedModel);
    };
    const persistedSessionsForBackend = async () => {
      const sessions = await readKeyval<Array<Record<string, unknown>>>("forge-sessions");
      return (sessions ?? []).map((session) => {
        const createdAt = typeof session.createdAt === "number" ? session.createdAt : Date.now();
        const updatedAt = typeof session.updatedAt === "number" ? session.updatedAt : createdAt;
        return {
          id: String(session.id ?? crypto.randomUUID()),
          provider: String(session.agentType ?? session.provider ?? "deepseek"),
          model: String(session.model ?? "deepseek-v4-flash[1m]"),
          status: String(session.status ?? "stopped"),
          created_at: new Date(createdAt).toISOString(),
          working_dir: typeof session.workingDir === "string" ? session.workingDir : null,
          created_at_ms: createdAt,
          updated_at_ms: updatedAt,
          context_window_tokens: typeof session.contextWindowTokens === "number" ? session.contextWindowTokens : null,
          latest_workflow: session.workflowState ?? null,
          latest_delivery: session.deliverySummary ?? null,
        };
      });
    };
    const appMetadataFromIndexedDb = async () => ({
      workspaces: await readKeyval("forge-workspaces") ?? [],
      activeWorkspaceId: await readKeyval("forge-active-workspace"),
      activeSessionId: await readKeyval("forge-active-session"),
      selectedProvider: await readKeyval("forge-provider"),
      selectedModel: await readKeyval("forge-model"),
    });
    // @ts-expect-error mock
    window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
      const projectPath = String(args.projectPath ?? workingDir);
      switch (cmd) {
        case "create_session":
          {
            // @ts-expect-error mock
            if (window.__mockCreateSessionError) throw new Error(String(window.__mockCreateSessionError));
            // @ts-expect-error mock
            const sessionId = window.__mockSessionId ?? crypto.randomUUID();
            sessionWorkingDirs.set(sessionId, String(args.workingDir ?? workingDir));
            // @ts-expect-error mock
            window.__lastCreateSessionArgs = args;
            // @ts-expect-error mock
            return {
              session_id: sessionId,
              provider: "deepseek",
              model: "deepseek-v4-flash[1m]",
              // @ts-expect-error mock
              missing_api_key: Boolean(window.__mockMissingApiKey),
            };
          }
        case "resume_session":
          {
            const sessionId = String(args.sessionId ?? "");
            // @ts-expect-error mock
            window.__lastResumedSessionId = sessionId;
            // @ts-expect-error mock
            const deliverySummary = window.__mockResumeDeliverySummary;
            if (deliverySummary) {
              window.setTimeout(() => {
                // @ts-expect-error listeners
                for (const listener of window.__tauriListeners?.["session-output"] ?? []) {
                  listener({
                    payload: {
                      event_type: "delivery_summary",
                      session_id: sessionId,
                      block_id: "resume-delivery-summary",
                      summary: deliverySummary,
                    },
                  });
                }
                // @ts-expect-error mock
                window.__resumeDeliveryEmitted = true;
              }, 0);
            }
            return {
              session_id: sessionId,
              provider: "deepseek",
              model: "deepseek-v4-flash[1m]",
              missing_api_key: false,
            };
          }
        case "send_input":
          // @ts-expect-error mock
          window.__lastSendInputArgs = args;
          return undefined;
        case "compact_session_context":
          // @ts-expect-error mock
          window.__lastCompactSessionContextArgs = args;
          // @ts-expect-error mock
          const compactResult = window.__mockCompactSessionContextResult ?? {
            compacted: true,
            retained_messages: 32,
            compacted_messages: 8,
            estimated_tokens_before: 142000,
            estimated_tokens_after: 32000,
          };
          if (compactResult.compacted === false) {
            window.setTimeout(() => {
              // @ts-expect-error listeners
              for (const listener of window.__tauriListeners?.["session-output"] ?? []) {
                listener({
                  payload: {
                    event_type: "context_compact_skipped",
                    session_id: String(args.sessionId ?? ""),
                    block_id: crypto.randomUUID(),
                    reason: String(compactResult.skipped_reason ?? "history_too_short"),
                    retained_messages: Number(compactResult.retained_messages ?? 0),
                  },
                });
              }
            }, 0);
          }
          return compactResult;
        case "kill_session":
          // @ts-expect-error mock
          window.__lastKilledSessionId = args.sessionId;
          return undefined;
        case "delete_session":
          // @ts-expect-error mock
          window.__lastDeletedSessionId = args.sessionId;
          return undefined;
        case "confirm_response":
        case "set_api_key":
          return undefined;
        case "get_diagnostics_report":
          // @ts-expect-error acceptance mock
          window.__diagnosticsReportRequestCount = Number(window.__diagnosticsReportRequestCount ?? 0) + 1;
          // @ts-expect-error acceptance mock
          return window.__mockDiagnosticsReport ?? diagnosticsReport();
        case "get_gateway_runtime_status":
          // @ts-expect-error acceptance mock
          return window.__mockGatewayRuntimeStatus ?? gatewayRuntimeStatus();
        case "get_service_status":
          return serviceStatus();
        case "set_autostart": {
          const enabled = Boolean(args.enabled);
          const current = serviceStatus();
          const next = {
            ...current,
            installed: enabled,
            running: enabled,
            message: enabled
              ? "Gateway service is installed and running."
              : "Gateway service is not installed.",
            status_message: enabled
              ? `Service '${String(current.service_id ?? current.label ?? "forge-gateway")}' is running.`
              : `Service '${String(current.service_id ?? current.label ?? "forge-gateway")}' is not installed.`,
          };
          // @ts-expect-error acceptance mock
          window.__mockServiceStatus = next;
          // @ts-expect-error acceptance mock
          window.__lastSetAutostartArgs = args;
          return next;
        }
        case "list_gateway_triggers":
          // @ts-expect-error acceptance mock
          return Array.isArray(window.__mockGatewayTriggers) ? window.__mockGatewayTriggers : [];
        case "list_gateway_sessions":
          // @ts-expect-error acceptance mock
          return Array.isArray(window.__mockGatewaySessions) ? window.__mockGatewaySessions : [];
        case "run_repair_action":
          // @ts-expect-error acceptance mock
          window.__lastRepairActionArgs = args;
          return {
            action_id: String(args.actionId ?? "restart_gateway"),
            success: true,
            message: "Repair action completed.",
            verification: {
              label: "Gateway service",
              ok: true,
              message: "Gateway service is running.",
            },
          };
        case "list_permission_rules":
          return permissionRules();
        case "set_permission_rule": {
          const toolName = String(args.toolName ?? "");
          const decision = String(args.decision ?? "allow");
          const next = [
            ...permissionRules().filter((rule) => rule.tool_name !== toolName),
            permissionRule(toolName, decision),
          ].sort((a, b) => String(a.tool_name).localeCompare(String(b.tool_name)));
          // @ts-expect-error mock
          window.__mockPermissionRules = next;
          // @ts-expect-error mock
          window.__lastSetPermissionRuleArgs = args;
          return next;
        }
        case "reset_permission_rule": {
          const toolName = String(args.toolName ?? "");
          const next = permissionRules().filter((rule) => rule.tool_name !== toolName);
          // @ts-expect-error mock
          window.__mockPermissionRules = next;
          // @ts-expect-error mock
          window.__lastResetPermissionRuleArgs = args;
          return next;
        }
        case "list_scheduled_tasks":
          return {
            tasks: schedulerTasks(),
            recent_history: schedulerHistory(),
            load_error: null,
          };
        case "review_agent_a2a_tasks":
          return reviewAgentA2AState(args);
        case "upsert_scheduled_task": {
          const input = (args.input ?? {}) as Record<string, unknown>;
          const tasks = schedulerTasks();
          const id = typeof input.id === "string" && input.id ? input.id : null;
          const existingIndex = id ? tasks.findIndex((task) => task.id === id) : -1;
          const task = schedulerTaskFromInput(input, existingIndex >= 0 ? tasks[existingIndex] : undefined);
          if (existingIndex >= 0) tasks[existingIndex] = task;
          else tasks.push(task);
          // @ts-expect-error mock
          window.__lastUpsertScheduledTaskArgs = args;
          return task;
        }
        case "delete_scheduled_task": {
          const id = String(args.id ?? "");
          const tasks = schedulerTasks();
          const next = tasks.filter((task) => task.id !== id);
          // @ts-expect-error mock
          window.__mockScheduledTasks = next;
          // @ts-expect-error mock
          window.__lastDeleteScheduledTaskArgs = args;
          return true;
        }
        case "set_scheduled_task_enabled": {
          const id = String(args.id ?? "");
          const enabled = Boolean(args.enabled);
          const tasks = schedulerTasks();
          const task = tasks.find((item) => item.id === id);
          if (task) {
            task.enabled = enabled;
            task.updated_at_ms = Date.now();
          }
          // @ts-expect-error mock
          window.__lastSetScheduledTaskEnabledArgs = args;
          return true;
        }
        case "run_scheduled_task_now": {
          const id = String(args.id ?? "");
          const now = Date.now();
          const tasks = schedulerTasks();
          const task = tasks.find((item) => item.id === id);
          if (!task) throw new Error(`Scheduled task not found: ${id}`);
          task.last_run_at_ms = now;
          task.updated_at_ms = now;
          schedulerHistory().unshift({
            id: crypto.randomUUID(),
            task_id: id,
            started_at_ms: now,
            ended_at_ms: now + 1,
            status: "completed",
            message: `Ran ${String(task.title)}`,
          });
          // @ts-expect-error mock
          window.__lastRunScheduledTaskNowArgs = args;
          return task;
        }
        case "list_sessions":
          // @ts-expect-error mock
          if (Array.isArray(window.__mockListSessions)) return window.__mockListSessions;
          return persistedSessionsForBackend();
        case "get_session_store_stats":
          // @ts-expect-error mock
          if (window.__mockSessionStoreStats) return window.__mockSessionStoreStats;
          return {
            total_snapshots: 0,
            corrupted_snapshots: 0,
            total_bytes: 0,
            oldest_updated_at_ms: null,
            newest_updated_at_ms: null,
            by_provider: {},
            by_workspace: {},
          };
        case "search_session_store":
          // @ts-expect-error mock
          window.__lastSearchSessionStoreArgs = args;
          // @ts-expect-error mock
          if (Array.isArray(window.__mockSessionStoreSearchResults)) {
            const query = String(args.query ?? "").toLowerCase();
            // @ts-expect-error mock
            return window.__mockSessionStoreSearchResults.filter((snapshot) => {
              const haystack = [
                snapshot.session_id,
                snapshot.provider,
                snapshot.model,
                snapshot.working_dir,
                snapshot.summary,
              ].join(" ").toLowerCase();
              return !query || haystack.includes(query);
            });
          }
          return [];
        case "rename_session_snapshot":
          {
            const sessionId = String(args.sessionId ?? "");
            const summary = String(args.summary ?? "");
            // @ts-expect-error mock
            window.__lastRenamedSessionSnapshotArgs = { sessionId, summary };
            // @ts-expect-error mock
            const snapshots = Array.isArray(window.__mockSessionStoreSearchResults)
              // @ts-expect-error mock
              ? window.__mockSessionStoreSearchResults
              : [];
            const snapshot = snapshots.find((item) => item.session_id === sessionId);
            if (!snapshot) return null;
            snapshot.summary = summary;
            snapshot.updated_at_ms = Date.now();
            return snapshot;
          }
        case "export_session_store":
          {
            // @ts-expect-error mock
            window.__lastExportSessionStoreCalled = true;
            // @ts-expect-error mock
            const snapshots = Array.isArray(window.__mockSessionStoreSearchResults)
              // @ts-expect-error mock
              ? window.__mockSessionStoreSearchResults
              : [];
            return {
              schema_version: 1,
              exported_at_ms: Date.now(),
              snapshots,
            };
          }
        case "prune_session_store":
          {
            // @ts-expect-error mock
            window.__lastPruneSessionStoreArgs = args;
            // @ts-expect-error mock
            const snapshots = Array.isArray(window.__mockSessionStoreSearchResults)
              // @ts-expect-error mock
              ? window.__mockSessionStoreSearchResults
              : [];
            const keepRecent = Number(args.keepRecent ?? snapshots.length);
            const deleted = snapshots.splice(keepRecent).map((snapshot) => snapshot.session_id);
            // @ts-expect-error mock
            if (window.__mockSessionStoreStats) {
              // @ts-expect-error mock
              window.__mockSessionStoreStats.total_snapshots = snapshots.length;
            }
            return {
              deleted_session_ids: deleted,
              kept_session_ids: snapshots.map((snapshot) => snapshot.session_id),
              skipped_corrupted: 0,
            };
          }
        case "load_app_metadata":
          return appMetadataFromIndexedDb();
        case "save_app_metadata":
          await saveAppMetadataToIndexedDb(args.metadata as Record<string, unknown>);
          return undefined;
        case "load_session_transcript":
          return [];
        case "get_default_working_dir":
          return workingDir;
        case "list_capabilities":
          return mockCapabilities();
        case "list_ecosystem_items":
          return mockEcosystemItems();
        case "get_tool_inventory":
          return mockToolInventory();
        case "search_workspace_files":
          {
            // @ts-expect-error mock
            window.__lastSearchWorkspaceFilesArgs = args;
            const sessionWorkspace = sessionWorkingDirs.get(String(args.sessionId ?? ""));
            const searchWorkspace = String(args.workingDir ?? sessionWorkspace ?? workingDir);
            const files = searchWorkspace.includes("forge-test-app")
              ? ["src/DemoApp.tsx", "src/components/TimerPanel.tsx", "README.md"]
              : [
                  "src/App.tsx",
                  "src/components/session/InputBar.tsx",
                  "README.md",
                  "src/features/deep-context/adapters/anthropic-session-stream-router.ts",
                  "src/features/deep-context/adapters/openai-compatible-stream-router.ts",
                  "src/features/deep-context/components/RunEvidenceTimeline.tsx",
                  "src/features/deep-context/components/ProjectArchiveInspector.tsx",
                  "src/features/deep-context/lib/workspace-boundary-policy.ts",
                  "src/features/deep-context/lib/markdown-diagram-normalizer.ts",
                  "src/features/deep-context/tests/composer-chip-overflow.fixture.ts",
                  "src/features/deep-context/docs/long-path-reference-material.md",
                ];
            return files.filter((path) => path.toLowerCase().includes(String(args.query ?? "").toLowerCase()));
          }
        case "toggle_capability":
          // @ts-expect-error acceptance mock
          window.__lastToggleCapabilityArgs = args;
          capabilityEnabledState()[String(args.capabilityId ?? args.id)] = Boolean(args.enabled);
          return undefined;
        case "set_ecosystem_enabled":
          // @ts-expect-error acceptance mock
          window.__lastSetEcosystemEnabledArgs = args;
          capabilityEnabledState()[String(args.id)] = Boolean(args.enabled);
          return undefined;
        case "get_api_key_status":
          // @ts-expect-error mock
          if (window.__mockApiKeyStatus) return window.__mockApiKeyStatus;
          return [{ provider: "deepseek", set: true, preview: "sk-e0...23ef" }];
        case "get_project_runtime_status":
          // @ts-expect-error mock
          window.__lastProjectRuntimeStatusArgs = args;
          return {
            ...projectRuntimeStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "get_project_checkpoint_status":
          // @ts-expect-error mock
          window.__lastProjectCheckpointStatusArgs = args;
          return {
            ...projectCheckpointStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "start_project_dev_server":
          // @ts-expect-error mock
          window.__lastStartProjectDevServerArgs = args;
          return {
            ...projectRuntimeStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
            running: true,
            managed: true,
            can_start: false,
            can_stop: true,
            can_open: true,
          };
        case "stop_project_dev_server":
          // @ts-expect-error mock
          window.__lastStopProjectDevServerArgs = args;
          return {
            ...projectRuntimeStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "open_project_preview":
          // @ts-expect-error mock
          window.__lastOpenProjectPreviewArgs = args;
          return {
            ...projectRuntimeStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
            running: true,
            managed: true,
            can_start: false,
            can_stop: true,
            can_open: true,
          };
        case "create_project_checkpoint":
          // @ts-expect-error mock
          window.__lastCreateProjectCheckpointArgs = args;
          return {
            ...projectCheckpointStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "restore_project_checkpoint":
          // @ts-expect-error mock
          window.__lastRestoreProjectCheckpointArgs = args;
          return {
            ...projectCheckpointStatus,
            working_dir: String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir),
          };
        case "preview_file":
          // @ts-expect-error mock
          window.__lastPreviewFileArgs = args;
          return {
            path: `${String(args.workingDir ?? sessionWorkingDirs.get(String(args.sessionId ?? "")) ?? workingDir)}/${String(args.path ?? "src/App.tsx")}`,
            display_path: String(args.path ?? "src/App.tsx"),
            requested_line: args.line ?? null,
            start_line: 1,
            total_lines: 3,
            lines: [
              { number: 1, content: "export function Demo() {", is_target: args.line === 1 },
              { number: 2, content: "  return null;", is_target: args.line === 2 },
              { number: 3, content: "}", is_target: args.line === 3 },
            ],
          };
        case "open_file":
          // @ts-expect-error mock
          window.__lastOpenFileArgs = args;
          return undefined;
        case "list_mcp_context_sources":
          return mcpContextSources;
        case "list_memories":
          return [];
        case "get_workflow_state":
          return null;
        case "override_workflow_route":
          return {
            session_id: String(args.sessionId ?? "session"),
            route: args.action === "debug" ? "recovery" : args.action === "verify" ? "verification" : args.action === "plan_first" ? "workflow" : "direct",
            phase: args.action === "debug" ? "debugging" : args.action === "verify" ? "verifying" : args.action === "plan_first" ? "clarifying" : "idle",
            beginner_label: args.action === "debug" ? "遇到问题，正在排查" : args.action === "verify" ? "正在检查结果" : args.action === "plan_first" ? "先梳理想法" : "直接回答",
            developer_label: String(args.action ?? "direct"),
            matched_signals: ["manual override"],
            reason: "用户手动切换了当前工作方式。",
            gate: "none",
            override_actions: ["direct", "plan_first", "debug", "verify"],
            spec_path: null,
            plan_path: null,
            checkpoint_id: null,
            updated_at: Date.now(),
          };
        case "get_forge_wiki_state":
          return forgeWikiState(projectPath, forgeWikiExists);
        case "init_forge_wiki":
          forgeWikiExists = true;
          return forgeWikiState(projectPath, true);
        case "list_forge_wiki_pages":
          return forgeWikiExists ? forgeWikiPages.map((page) => ({ ...page, project_path: projectPath })) : [];
        case "read_forge_wiki_page":
          return args.pagePath === "tasks.md" ? "# 当前任务\n\n覆盖项目档案面板。" : "# 项目概览\n\n项目记录预览。";
        case "select_forge_wiki_context":
          return [
            {
              page_id: "tasks",
              title: "当前任务",
              path: "tasks.md",
              kind: "tasks",
              summary: "当前任务、验收步骤和后续事项。",
              score: 0.96,
              reason: "和当前请求最相关",
              injected: true,
            },
          ];
        case "create_forge_wiki_update_proposal": {
          const proposal = forgeWikiProposal(projectPath, args);
          forgeWikiProposals.set(String(proposal.id), proposal);
          return proposal;
        }
        case "accept_forge_wiki_update_proposal": {
          const proposal = forgeWikiProposals.get(String(args.proposalId)) ?? forgeWikiProposal(projectPath, args);
          const accepted = { ...proposal, status: "accepted" };
          forgeWikiProposals.set(String(accepted.id), accepted);
          return accepted;
        }
        case "discard_forge_wiki_update_proposal": {
          const proposal = forgeWikiProposals.get(String(args.proposalId)) ?? forgeWikiProposal(projectPath, args);
          const discarded = { ...proposal, status: "discarded" };
          forgeWikiProposals.set(String(discarded.id), discarded);
          return discarded;
        }
        default:
          return undefined;
      }
    };
    // @ts-expect-error mock
    window.__TAURI_INTERNALS__ = {
      invoke: (cmd: string, args: Record<string, unknown>) => {
        if (cmd === "plugin:event|listen") {
          // @ts-expect-error listeners
          if (!window.__tauriListeners[args.event as string]) window.__tauriListeners[args.event as string] = [];
          const callback = callbacks.get(args.handler as number);
          if (callback) {
            // @ts-expect-error listeners
            window.__tauriListeners[args.event as string].push(callback);
          }
          return args.handler;
        }
        if (cmd === "plugin:event|unlisten") {
          const event = args.event as string;
          const id = args.eventId as number;
          // @ts-expect-error listeners
          window.__tauriListeners[event] = (window.__tauriListeners[event] ?? []).filter((fn: unknown) => fn !== callbacks.get(id));
          callbacks.delete(id);
          return undefined;
        }
        return window.__tauriMockIPC?.(cmd, args);
      },
      transformCallback: (callback: (data: unknown) => void) => {
        callbackId += 1;
        callbacks.set(callbackId, callback);
        return callbackId;
      },
      unregisterCallback: (id: number) => {
        callbacks.delete(id);
      },
      callbacks,
    };
    // @ts-expect-error mock
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = {
      unregisterListener: (_event: string, id: number) => {
        callbacks.delete(id);
      },
    };
    // @ts-expect-error listeners
    window.__tauriListeners = {};
    // Mock Tauri listen()
    // @ts-expect-error
    window.__TAURI__ = {
      event: {
        listen: (event: string, fn: (data: unknown) => void) => {
          // @ts-expect-error
          if (!window.__tauriListeners[event]) window.__tauriListeners[event] = [];
          // @ts-expect-error
          window.__tauriListeners[event].push(fn);
          return () => {};
        },
      },
    };
  }, { initialWorkingDir });
}

export async function holdSendInput(page: Page) {
  await page.evaluate(() => {
    // @ts-expect-error mock
    const original = window.__tauriMockIPC;
    const calls: Record<string, unknown>[] = [];
    const resolvers: Array<() => void> = [];

    // @ts-expect-error mock
    window.__heldSendInput = {
      calls,
      releaseNext: () => {
        const resolve = resolvers.shift();
        resolve?.();
      },
    };

    // @ts-expect-error mock
    window.__tauriMockIPC = async (cmd: string, args: Record<string, unknown>) => {
      if (cmd === "send_input") {
        calls.push(args);
        await new Promise<void>((resolve) => {
          resolvers.push(resolve);
        });
        return undefined;
      }
      return original?.(cmd, args);
    };
  });
}

export async function expectHeldSendInput(page: Page, textIncludes: string) {
  await expect.poll(async () => page.evaluate(() => {
    // @ts-expect-error mock
    return window.__heldSendInput?.calls.length ?? 0;
  })).toBe(1);

  const [call] = await page.evaluate(() => {
    // @ts-expect-error mock
    return window.__heldSendInput?.calls ?? [];
  });
  expect(String(call.text)).toContain(textIncludes);
  return call;
}

export async function getLastSendInputArgs(page: Page): Promise<Record<string, unknown> | undefined> {
  return page.evaluate(() => {
    // @ts-expect-error mock
    return window.__lastSendInputArgs;
  });
}

export async function expectLastSendInputArgs(page: Page, expected: Record<string, unknown>) {
  await expect.poll(async () => getLastSendInputArgs(page)).toMatchObject(expected);
  const args = await getLastSendInputArgs(page);
  expect(args).toBeDefined();
  return args!;
}

export async function expectNoSendInput(page: Page) {
  await expect(await getLastSendInputArgs(page)).toBeUndefined();
}

export async function releaseHeldSendInput(page: Page) {
  await page.evaluate(() => {
    // @ts-expect-error mock
    window.__heldSendInput?.releaseNext();
  });
}

export function projectArchive(page: Page) {
  return page.getByTestId("project-archive-panel");
}

export async function openProjectArchive(page: Page, section?: "records") {
  const archive = projectArchive(page);
  if (await archive.isVisible().catch(() => false)) return archive;

  await page.getByRole("button", { name: "打开项目档案" }).click();
  if (await archive.isVisible({ timeout: 750 }).catch(() => false)) return archive;

  await page.evaluate((targetSection) => {
    window.dispatchEvent(new CustomEvent("open-hub", {
      detail: targetSection ? { section: targetSection } : undefined,
    }));
  }, section);
  await expect(archive).toBeVisible();
  return archive;
}

export async function expandArchiveRecords(page: Page) {
  const archive = projectArchive(page);
  const records = archive.getByTestId("archive-disclosure-records");
  const trigger = records.getByRole("button", { name: /项目记录/ }).first();
  if (await trigger.getAttribute("aria-expanded") !== "true") {
    await trigger.click();
  }
  return records;
}

export async function expandArchiveFiles(page: Page) {
  const archive = projectArchive(page);
  const files = archive.getByTestId("archive-disclosure-files");
  const trigger = files.getByRole("button", { name: /资料/ }).first();
  if (await trigger.getAttribute("aria-expanded") !== "true") {
    await trigger.click();
  }
  return files;
}
