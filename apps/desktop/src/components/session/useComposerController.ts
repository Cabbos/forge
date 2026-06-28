import { useComposerActions } from "./useComposerActions";
import { useComposerChips } from "./useComposerChips";
import { useComposerDraft } from "./useComposerDraft";
import { useComposerModelMenu } from "./useComposerModelMenu";
import { useComposerPresentation } from "./useComposerPresentation";
import { useComposerSessionState } from "./useComposerSessionState";
import { useComposerSuggestions } from "./useComposerSuggestions";
import { buildComposerContextUsageView } from "./contextUsageView";

export function useComposerController(sessionId: string) {
  const {
    composerState,
    contextUsage,
    contextWindowTokens,
    isRunning,
    isTurnInFlight,
    workflow,
    workingDir,
  } = useComposerSessionState(sessionId);
  const {
    adjustHeight,
    composingRef,
    focusTextarea,
    resetDraft,
    setValue,
    textareaRef,
    value,
    valueRef,
  } = useComposerDraft();
  const {
    closeModelMenu,
    providers,
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
    workingDir,
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
  const {
    handleChange,
    handleCompositionEnd,
    handleCompositionStart,
    handleKeyDown,
    handleCompact,
    handleResume,
    handleSend,
    handleStop,
    handleToggleModelMenu,
    isCompacting,
    isResuming,
    resumeError,
  } = useComposerActions({
    activeSuggestionIndex,
    addChip,
    adjustHeight,
    atResults,
    chips,
    clearChips,
    closeModelMenu,
    closeSuggestions,
    composingRef,
    focusTextarea,
    isRunning,
    removeLastChip,
    resetDraft,
    sessionId,
    setActiveSuggestionIndex,
    setValue,
    showSuggestions,
    syncSuggestionsForInput,
    toggleModelMenu,
    value,
    valueRef,
    workingDir,
  });
  const canSend = isRunning && !isTurnInFlight && (value.trim().length > 0 || chips.length > 0);
  const contextUsageView = buildComposerContextUsageView({
    fallbackContextWindowTokens: contextWindowTokens,
    isCompacting,
    isStreaming: isTurnInFlight,
    usage: contextUsage,
  });

  const {
    composerRootRef,
    menuLayerProps,
    resumeErrorMessage,
    surfaceProps,
  } = useComposerPresentation({
    activeSuggestionIndex,
    atResults,
    canSend,
    chips,
    closeModelMenu,
    closeSuggestions,
    composerState,
    contextUsageView,
    focusTextarea,
    isResuming,
    isRunning,
    isTurnInFlight,
    onActiveSuggestionIndexChange: setActiveSuggestionIndex,
    onAddChip: addChip,
    onCompositionStart: handleCompositionStart,
    onCompositionEnd: handleCompositionEnd,
    onRemoveChip: removeChip,
    onResume: handleResume,
    onCompact: handleCompact,
    onSelectModel: selectModel,
    providers,
    onSend: handleSend,
    onStop: handleStop,
    onTextChange: handleChange,
    onTextKeyDown: handleKeyDown,
    onToggleModelMenu: handleToggleModelMenu,
    onToggleSuggestion: toggleSuggestion,
    resumeError,
    selectedContextWindow,
    selectedModel,
    selectedModelLabel,
    selectedProvider,
    selectedProviderLabel,
    sessionId,
    showModelMenu,
    showSuggestions,
    value,
    workflow,
  });

  return {
    composerRootRef,
    menuLayerProps,
    resumeErrorMessage,
    surfaceProps,
    textareaRef,
  };
}
