import { useCallback } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useStore } from "../store";
import {
  compactSessionContext,
  createSession,
  deleteSession,
  resumeSession,
  sendInput,
  killSession,
} from "../lib/tauri";
import type { ComposerCapabilitySelection, McpContextSelection } from "../lib/tauri";
import { getProviderLabel } from "../lib/providers";
import { queryKeys } from "@/hooks/queries/queryKeys";
import { useProfilesQuery } from "@/hooks/queries/useProfilesQuery";
import { resolveProfileSessionDefaults } from "./sessionProfileDefaults";

export function useSession() {
  const addSession = useStore((s) => s.addSession);
  const removeSession = useStore((s) => s.removeSession);
  const updateSessionStatus = useStore((s) => s.updateSessionStatus);
  const dispatchOutputEvent = useStore((s) => s.dispatchOutputEvent);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const { data: profiles } = useProfilesQuery();
  const queryClient = useQueryClient();

  const create = useCallback(
    async (workingDir: string, provider = selectedProvider, model = selectedModel) => {
      try {
        const sessionDefaults = resolveProfileSessionDefaults({
          workingDir,
          provider,
          model,
          profiles,
        });
        const result = await createSession(
          sessionDefaults.workingDir,
          sessionDefaults.provider,
          sessionDefaults.model,
          "",
          sessionDefaults.profileId,
        );
        addSession(
          result.session_id,
          result.provider ?? sessionDefaults.provider,
          result.model ?? sessionDefaults.model,
          sessionDefaults.workingDir,
        );
        await queryClient.invalidateQueries({ queryKey: queryKeys.sessions });
        if (result.missing_api_key) {
          const providerLabel = getProviderLabel(result.provider ?? sessionDefaults.provider);
          dispatchOutputEvent({
            event_type: "error",
            session_id: result.session_id,
            block_id: crypto.randomUUID(),
            message: `还没有配置 ${providerLabel} 模型密钥。请打开设置，粘贴密钥后就可以开始发送。`,
            code: "missing_api_key",
          });
        }
        return result.session_id;
      } catch (e) {
        console.error("Failed to create session:", e);
        throw e;
      }
    },
    [addSession, dispatchOutputEvent, profiles, queryClient, selectedModel, selectedProvider]
  );

  const resume = useCallback(
    async (sessionId: string) => {
      try {
        const result = await resumeSession(sessionId);
        updateSessionStatus(result.session_id, "running");
        await queryClient.invalidateQueries({ queryKey: queryKeys.sessions });
        if (result.missing_api_key) {
          const providerLabel = getProviderLabel(result.provider);
          dispatchOutputEvent({
            event_type: "error",
            session_id: result.session_id,
            block_id: crypto.randomUUID(),
            message: `还没有配置 ${providerLabel} 模型密钥。请打开设置，粘贴密钥后就可以开始发送。`,
            code: "missing_api_key",
          });
        }
        return result.session_id;
      } catch (e) {
        console.error("Failed to resume session:", e);
        throw e;
      }
    },
    [dispatchOutputEvent, queryClient, updateSessionStatus]
  );

  const send = useCallback(async (
    sessionId: string,
    text: string,
    mcpContext: McpContextSelection[] = [],
    capabilities: ComposerCapabilitySelection[] = [],
  ) => {
    try {
      await sendInput(sessionId, text, mcpContext, capabilities);
    } catch (e) {
      console.error("Failed to send input:", e);
      dispatchOutputEvent({
        event_type: "error",
        session_id: sessionId,
        block_id: crypto.randomUUID(),
        message: userFacingSendError(e),
        code: "send_failed",
      });
    }
  }, [dispatchOutputEvent]);

  const stop = useCallback(
    async (sessionId: string) => {
      try {
        await killSession(sessionId);
        dispatchOutputEvent({
          event_type: "session_stopped",
          session_id: sessionId,
          reason: "stopped",
        });
      } catch (e) {
        console.error("Failed to stop session:", e);
        dispatchOutputEvent({
          event_type: "error",
          session_id: sessionId,
          block_id: crypto.randomUUID(),
          message: userFacingSendError(e, "停止失败"),
          code: "stop_failed",
        });
      }
    },
    [dispatchOutputEvent]
  );

  const compact = useCallback(
    async (sessionId: string) => {
      try {
        await compactSessionContext(sessionId);
      } catch (e) {
        console.error("Failed to compact session context:", e);
        dispatchOutputEvent({
          event_type: "error",
          session_id: sessionId,
          block_id: crypto.randomUUID(),
          message: userFacingSendError(e, "压缩上下文失败"),
          code: "compact_failed",
        });
      }
    },
    [dispatchOutputEvent]
  );

  const deleteConversation = useCallback(
    async (sessionId: string) => {
      try {
        await deleteSession(sessionId);
      } catch (e) {
        console.error("Failed to delete session:", e);
      } finally {
        removeSession(sessionId);
        await queryClient.invalidateQueries({ queryKey: queryKeys.sessions });
      }
    },
    [removeSession, queryClient]
  );

  return { create, resume, send, stop, compact, deleteConversation };
}

function userFacingSendError(error: unknown, prefix = "发送失败") {
  const raw = error instanceof Error ? error.message : String(error);
  if (/api key|密钥|key configured/i.test(raw)) {
    return `${prefix}：模型服务还没有可用密钥，请打开设置后再试。`;
  }
  if (/session not found|not running|会话/i.test(raw)) {
    return `${prefix}：当前会话暂时不可用，可以继续会话或新建对话后再试。`;
  }
  return `${prefix}：这次请求没有发出去，请稍后重试。`;
}
