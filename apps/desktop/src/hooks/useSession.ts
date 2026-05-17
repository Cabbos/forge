import { useCallback } from "react";
import { useStore } from "../store";
import { createSession, getProjectCheckpointStatus, getProjectRuntimeStatus, resumeSession, sendInput, killSession } from "../lib/tauri";
import { getProviderLabel } from "../lib/providers";
import { getDeliveryConfidence } from "../lib/delivery-confidence";

export function useSession() {
  const addSession = useStore((s) => s.addSession);
  const removeSession = useStore((s) => s.removeSession);
  const updateSessionStatus = useStore((s) => s.updateSessionStatus);
  const dispatchOutputEvent = useStore((s) => s.dispatchOutputEvent);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);

  const create = useCallback(
    async (workingDir: string, provider = selectedProvider, model = selectedModel) => {
      try {
        const result = await createSession(workingDir, provider, model);
        addSession(result.session_id, result.provider ?? provider, result.model ?? model, workingDir);
        if (result.missing_api_key) {
          const providerLabel = getProviderLabel(result.provider ?? provider);
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
    [addSession, dispatchOutputEvent, selectedModel, selectedProvider]
  );

  const resume = useCallback(
    async (sessionId: string) => {
      try {
        const result = await resumeSession(sessionId);
        updateSessionStatus(result.session_id, "running");
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
    [dispatchOutputEvent, updateSessionStatus]
  );

  const send = useCallback(async (sessionId: string, text: string) => {
    try {
      await sendInput(sessionId, text);
      try {
        const [runtime, checkpoint] = await Promise.all([
          getProjectRuntimeStatus(sessionId),
          getProjectCheckpointStatus(sessionId),
        ]);
        const delivery = getDeliveryConfidence(runtime, checkpoint);
        dispatchOutputEvent({
          event_type: "delivery_summary",
          session_id: sessionId,
          block_id: crypto.randomUUID(),
          summary: {
            project_path: runtime.working_dir || checkpoint.working_dir || null,
            preview_label: delivery.preview.label,
            checkpoint_label: delivery.checkpoint.label,
            next_action: delivery.nextAction,
          },
        });
      } catch (summaryError) {
        console.warn("Failed to summarize delivery:", summaryError);
      }
    } catch (e) {
      console.error("Failed to send input:", e);
    }
  }, [dispatchOutputEvent]);

  const kill = useCallback(
    async (sessionId: string) => {
      try {
        await killSession(sessionId);
        removeSession(sessionId);
      } catch (e) {
        console.error("Failed to kill session:", e);
      }
    },
    [removeSession]
  );

  return { create, resume, send, kill };
}
