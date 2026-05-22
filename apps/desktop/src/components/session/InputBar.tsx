import { useRef, useCallback, useEffect } from "react";
import { AlertCircle } from "lucide-react";
import { useSession } from "@/hooks/useSession";
import { useStore } from "@/store";
import { modeAwarePlaceholder } from "@/lib/task-mode";
import { ComposerChipTray } from "./ComposerChipTray";
import { ComposerModelMenu } from "./ComposerModelMenu";
import { ComposerSuggestionMenu } from "./ComposerSuggestionMenu";
import { ComposerToolbar } from "./ComposerToolbar";
import { useComposerChips } from "./useComposerChips";
import { useComposerDraft } from "./useComposerDraft";
import { useComposerKeyboard } from "./useComposerKeyboard";
import { useComposerModelMenu } from "./useComposerModelMenu";
import { useComposerResume } from "./useComposerResume";
import { useComposerSuggestions } from "./useComposerSuggestions";
import { useComposerSubmit } from "./useComposerSubmit";

interface InputBarProps { sessionId: string }

export function InputBar({ sessionId }: InputBarProps) {
  const composerRootRef = useRef<HTMLDivElement>(null);
  const pendingInput = useStore((s) => s.pendingInput);
  const setPendingInput = useStore((s) => s.setPendingInput);
  const workflow = useStore((s) => s.workflowBySession.get(sessionId) ?? null);
  const suggestionListId = `${sessionId}-composer-suggestions`;
  const modelMenuId = `${sessionId}-model-menu`;

  const { send, stop, resume } = useSession();
  const session = useStore((s) => s.sessions.get(sessionId));
  const {
    adjustHeight,
    composingRef,
    focusTextarea,
    resetDraft,
    setValue,
    textareaRef,
    value,
    valueRef,
  } = useComposerDraft({ pendingInput, setPendingInput });
  const {
    closeModelMenu,
    selectModel,
    selectedContextWindow,
    selectedModel,
    selectedModelLabel,
    selectedProvider,
    selectedProviderLabel,
    showModelMenu,
    toggleModelMenu,
  } = useComposerModelMenu();
  const {
    activeSuggestionIndex,
    atResults,
    closeSuggestions,
    setActiveSuggestionIndex,
    showSuggestions,
    syncSuggestionsForInput,
    toggleSuggestion,
  } = useComposerSuggestions({
    sessionId,
    workingDir: session?.workingDir,
    onCloseModelMenu: closeModelMenu,
    onFocusTextarea: focusTextarea,
  });
  const {
    addChip,
    chips,
    clearChips,
    removeChip,
    removeLastChip,
  } = useComposerChips({
    closeSuggestions,
    focusTextarea,
    setValue,
    textareaRef,
    valueRef,
  });
  const isRunning = session?.status === "running";
  const isStreaming = session?.streaming ?? false;
  const composerState = isStreaming ? "running" : isRunning ? "busy" : "paused";
  const {
    handleResume,
    isResuming,
    resumeError,
  } = useComposerResume({
    isRunning,
    onFocusTextarea: focusTextarea,
    resume,
    sessionId,
  });
  const canSend = isRunning && (value.trim().length > 0 || chips.length > 0);

  useEffect(() => { if (isRunning) focusTextarea(); }, [focusTextarea, sessionId, isRunning]);
  useEffect(() => {
    if (!showSuggestions && !showModelMenu) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && composerRootRef.current?.contains(target)) return;
      closeSuggestions();
      closeModelMenu();
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [closeModelMenu, closeSuggestions, showModelMenu, showSuggestions]);

  const handleChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const v = e.target.value;
    valueRef.current = v;
    setValue(v);
    adjustHeight();
    syncSuggestionsForInput(v, e.target.selectionStart);
  }, [adjustHeight, syncSuggestionsForInput]);
  const handleSend = useComposerSubmit({
    chips,
    isRunning,
    onClearChips: clearChips,
    onResetDraft: resetDraft,
    send,
    sessionId,
    value,
    workingDir: session?.workingDir,
  });

  const handleToggleModelMenu = useCallback(() => {
    closeSuggestions();
    toggleModelMenu();
  }, [closeSuggestions, toggleModelMenu]);
  const handleKeyDown = useComposerKeyboard({
    activeSuggestionIndex,
    addChip,
    atResults,
    chipsCount: chips.length,
    closeModelMenu,
    closeSuggestions,
    composingRef,
    onSend: handleSend,
    removeLastChip,
    setActiveSuggestionIndex,
    showSuggestions,
    valueRef,
  });

  return (
    <div data-testid="composer-frame" className="forge-composer-frame relative flex-shrink-0">
      <div ref={composerRootRef} data-testid="composer-lane" className="forge-conversation-lane relative">
      {!isRunning && resumeError && (
        <div data-testid="composer-error" role="status" aria-live="polite" className="forge-composer-error">
          <AlertCircle className="size-3.5 shrink-0" />
          <span className="min-w-0 truncate">{resumeError}</span>
        </div>
      )}
      {showSuggestions && (
        <ComposerSuggestionMenu
          id={suggestionListId}
          mode={showSuggestions}
          atResults={atResults}
          activeIndex={activeSuggestionIndex}
          onActiveIndexChange={setActiveSuggestionIndex}
          onAddChip={addChip}
        />
      )}

      {showModelMenu && (
        <ComposerModelMenu
          id={modelMenuId}
          labelledBy={`${modelMenuId}-button`}
          selectedModel={selectedModel}
          selectedProvider={selectedProvider}
          onSelect={selectModel}
        />
      )}

      <div
        data-testid="composer-surface"
        data-menu-open={showSuggestions || showModelMenu ? "true" : "false"}
        data-streaming={isStreaming ? "true" : "false"}
        data-state={composerState}
        className="forge-composer"
      >
        <ComposerChipTray chips={chips} onRemove={removeChip} />

        {/* Textarea */}
        <div data-testid="composer-textarea-wrap" className="forge-composer-textarea-wrap">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            onCompositionStart={() => {
              composingRef.current = true;
            }}
            onCompositionEnd={() => {
              composingRef.current = false;
              adjustHeight();
            }}
            placeholder={modeAwarePlaceholder(workflow, isRunning)}
            rows={1}
            disabled={!isRunning}
            className="forge-composer-textarea"
            style={{ paddingTop: chips.length > 0 ? "4px" : undefined }}
          />
        </div>

        <ComposerToolbar
          canSend={canSend}
          isResuming={isResuming}
          isRunning={isRunning}
          isStreaming={isStreaming}
          modelMenuId={modelMenuId}
          selectedContextWindow={selectedContextWindow}
          selectedModelLabel={selectedModelLabel}
          selectedProviderLabel={selectedProviderLabel}
          showModelMenu={showModelMenu}
          showSuggestions={showSuggestions}
          suggestionListId={suggestionListId}
          onResume={handleResume}
          onSend={handleSend}
          onStop={() => stop(sessionId)}
          onToggleModelMenu={handleToggleModelMenu}
          onToggleSuggestion={toggleSuggestion}
        />
      </div>
      </div>
    </div>
  );
}
