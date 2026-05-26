import { useCallback } from "react";
import { createProjectCheckpoint } from "@/lib/tauri";
import type { ComposerCapabilitySelection, McpContextSelection } from "@/lib/tauri";
import { buildFirstLoopAgentPrompt, deriveFirstLoopDraft } from "@/lib/first-loop";
import { useStore } from "@/store";
import type { ComposerChip } from "./composerTypes";

type SendComposerInput = (
  sessionId: string,
  text: string,
  mcpContext?: McpContextSelection[],
  capabilities?: ComposerCapabilitySelection[],
) => Promise<void>;

interface UseComposerSubmitOptions {
  chips: ComposerChip[];
  isRunning: boolean;
  onClearChips: () => void;
  onResetDraft: () => void;
  send: SendComposerInput;
  sessionId: string;
  value: string;
  workingDir?: string | null;
}

export function useComposerSubmit({
  chips,
  isRunning,
  onClearChips,
  onResetDraft,
  send,
  sessionId,
  value,
  workingDir,
}: UseComposerSubmitOptions) {
  const setFirstLoopDraft = useStore((s) => s.setFirstLoopDraft);
  const selectedMcpContext = useStore((s) => s.mcpContextBySession.get(sessionId) ?? []);

  return useCallback(async () => {
    const text = value.trim();
    if (!text && chips.length === 0) return;
    if (!isRunning) return;

    let message = text;
    const fileChips = chips.filter((chip) => chip.type === "file");
    const cmdChips = chips.filter((chip) => chip.type === "command");
    const capabilities: ComposerCapabilitySelection[] = [
      ...cmdChips.map((chip) => ({ kind: "slash_command" as const, command: chip.value })),
      ...fileChips.map((chip) => ({ kind: "file_reference" as const, path: chip.value })),
    ];

    if (fileChips.length > 0) {
      message = fileChips.map((chip) => `@${chip.value}`).join(" ") + (message ? "\n" + message : "");
    }
    if (!message.trim() && cmdChips.length > 0) {
      message = "请按所选动作继续。";
    }
    if (!message.trim()) return;

    const firstLoopDraft = deriveFirstLoopDraft(sessionId, message);
    if (firstLoopDraft) {
      setFirstLoopDraft(sessionId, firstLoopDraft);
    }

    await createProjectCheckpoint(sessionId, workingDir).catch(() => {});
    useStore.getState().addUserMessage(sessionId, message);
    await send(sessionId, buildFirstLoopAgentPrompt(message), selectedMcpContext, capabilities);
    onResetDraft();
    onClearChips();
  }, [
    chips,
    isRunning,
    onClearChips,
    onResetDraft,
    selectedMcpContext,
    send,
    sessionId,
    setFirstLoopDraft,
    value,
    workingDir,
  ]);
}
