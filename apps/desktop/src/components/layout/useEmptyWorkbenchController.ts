import { useCallback, useRef, useState, type KeyboardEvent as ReactKeyboardEvent } from "react";
import { useActiveWorkspace, useStore } from "@/store";
import { useSession } from "@/hooks/useSession";
import { buildFirstLoopAgentPrompt, deriveFirstLoopDraft } from "@/lib/first-loop";
import { createProjectCheckpoint, pickWorkspaceFolder } from "@/lib/tauri";
import { isBroadWorkspacePath, workspaceFromPath } from "@/lib/workspaces";
import type { EmptyStartMode, EmptyWorkbenchProps } from "./EmptyWorkbench";

type EmptyWorkbenchControllerProps = Omit<EmptyWorkbenchProps, "project">;

export function useEmptyWorkbenchController() {
  const [emptyPrompt, setEmptyPrompt] = useState("");
  const [emptyStartMode, setEmptyStartMode] = useState<EmptyStartMode | null>(null);
  const [emptyWorkspaceNotice, setEmptyWorkspaceNotice] = useState<string | null>(null);
  const [emptyPromptStarting, setEmptyPromptStarting] = useState(false);
  const emptyPromptRef = useRef<HTMLTextAreaElement>(null);
  const activeWorkspace = useActiveWorkspace();
  const selectedProvider = useStore((s) => s.selectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const setFirstLoopDraft = useStore((s) => s.setFirstLoopDraft);
  const addUserMessage = useStore((s) => s.addUserMessage);
  const upsertWorkspace = useStore((s) => s.upsertWorkspace);
  const { create, send } = useSession();

  const emptyComposerPlaceholder = emptyStartMode === "existing-project"
    ? "描述当前项目里要改的地方，Forge 会保持在当前项目内处理"
    : "描述你想做的小工具，例如：记录喝水次数、客户跟进、番茄钟";
  const emptyComposerContext = emptyStartMode === "existing-project"
    ? "打开已有项目 · Enter 发送 · Shift+Enter 换行"
    : "做个新工具 · Enter 发送 · Shift+Enter 换行";

  const focusEmptyComposer = useCallback(() => {
    requestAnimationFrame(() => {
      emptyPromptRef.current?.focus();
    });
  }, []);

  const activateWorkspaceFromPath = useCallback((path: string): boolean => {
    if (!path) {
      setEmptyWorkspaceNotice("先选择一个保存位置或已有项目文件夹。");
      return false;
    }
    if (isBroadWorkspacePath(path)) {
      setEmptyWorkspaceNotice("请选择更具体的文件夹，不要直接使用用户主目录。");
      return false;
    }
    const workspace = workspaceFromPath(path);
    if (!workspace) {
      setEmptyWorkspaceNotice("这个路径暂时不能作为本地工作空间。");
      return false;
    }
    upsertWorkspace(workspace);
    setEmptyWorkspaceNotice(null);
    return true;
  }, [upsertWorkspace]);

  const chooseWorkspaceForEmptyState = useCallback(async (): Promise<boolean> => {
    setEmptyWorkspaceNotice(null);
    try {
      const selectedPath = await pickWorkspaceFolder();
      if (!selectedPath) {
        setEmptyWorkspaceNotice("先选择保存位置或已有项目文件夹，再开始对话。");
        return false;
      }
      return activateWorkspaceFromPath(selectedPath);
    } catch (error) {
      console.error("Failed to choose workspace from empty state:", error);
      setEmptyWorkspaceNotice("没有打开文件夹选择器，请从左侧选择项目。");
      return false;
    }
  }, [activateWorkspaceFromPath]);

  const selectNewToolEntry = useCallback(async () => {
    setEmptyStartMode("new-tool");
    if (!activeWorkspace) {
      const selected = await chooseWorkspaceForEmptyState();
      if (!selected) return;
    }
    focusEmptyComposer();
  }, [activeWorkspace, chooseWorkspaceForEmptyState, focusEmptyComposer]);

  const selectExistingProjectEntry = useCallback(async () => {
    setEmptyStartMode("existing-project");
    if (!activeWorkspace) {
      const selected = await chooseWorkspaceForEmptyState();
      if (!selected) return;
    }
    focusEmptyComposer();
  }, [activeWorkspace, chooseWorkspaceForEmptyState, focusEmptyComposer]);

  const startConversation = useCallback(() => {
    if (!activeWorkspace) return;
    create(activeWorkspace.path, selectedProvider, selectedModel).catch((error) => {
      console.error("Failed to create session:", error);
    });
  }, [activeWorkspace, create, selectedModel, selectedProvider]);

  const startConversationWithPrompt = useCallback(async () => {
    const text = emptyPrompt.trim();
    if (!activeWorkspace || !text || emptyPromptStarting) return;

    setEmptyPromptStarting(true);
    try {
      const sessionId = await create(activeWorkspace.path, selectedProvider, selectedModel);
      const firstLoopDraft = deriveFirstLoopDraft(sessionId, text);
      if (firstLoopDraft) {
        setFirstLoopDraft(sessionId, firstLoopDraft);
      }
      await createProjectCheckpoint(sessionId, activeWorkspace.path).catch(() => {});
      addUserMessage(sessionId, text);
      await send(sessionId, buildFirstLoopAgentPrompt(text, { workingDir: activeWorkspace.path }), []);
      setEmptyPrompt("");
    } catch (error) {
      console.error("Failed to start conversation from prompt:", error);
    } finally {
      setEmptyPromptStarting(false);
    }
  }, [
    activeWorkspace,
    addUserMessage,
    create,
    emptyPrompt,
    emptyPromptStarting,
    selectedModel,
    selectedProvider,
    send,
    setFirstLoopDraft,
  ]);

  const handleEmptyPromptKeyDown = useCallback((event: ReactKeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key !== "Enter" || event.shiftKey || event.nativeEvent.isComposing) return;
    event.preventDefault();
    startConversationWithPrompt();
  }, [startConversationWithPrompt]);

  const useEmptyHint = useCallback((hint: string) => {
    setEmptyStartMode(hint.includes("检查") || hint.includes("优化") ? "existing-project" : "new-tool");
    setEmptyPrompt(hint);
    requestAnimationFrame(() => {
      emptyPromptRef.current?.focus();
      emptyPromptRef.current?.setSelectionRange(hint.length, hint.length);
    });
  }, []);

  const emptyWorkbenchProps: EmptyWorkbenchControllerProps = {
    emptyComposerContext,
    emptyComposerPlaceholder,
    emptyPrompt,
    emptyPromptRef,
    emptyPromptStarting,
    emptyStartMode,
    emptyWorkspaceNotice,
    hasActiveWorkspace: Boolean(activeWorkspace),
    onEmptyPromptChange: setEmptyPrompt,
    onEmptyPromptKeyDown: handleEmptyPromptKeyDown,
    onSelectExistingProjectEntry: selectExistingProjectEntry,
    onSelectNewToolEntry: selectNewToolEntry,
    onStartConversation: startConversation,
    onStartConversationWithPrompt: startConversationWithPrompt,
    onUseEmptyHint: useEmptyHint,
  };

  return { emptyWorkbenchProps, startConversation };
}
