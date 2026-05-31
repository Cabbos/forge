import { useCallback, useEffect, useRef } from "react";
import type { WorkflowState } from "@/lib/protocol";
import {
  buildComposerMenuLayerProps,
  buildComposerSurfaceProps,
} from "./composerControllerView";
import type { ComposerMenuLayerProps } from "./ComposerMenuLayer";
import type { ComposerSurfaceProps } from "./ComposerSurface";
import { useComposerMenuDismissal } from "./useComposerMenuDismissal";

interface UseComposerPresentationOptions {
  activeSuggestionIndex: ComposerMenuLayerProps["activeSuggestionIndex"];
  atResults: ComposerMenuLayerProps["atResults"];
  canSend: ComposerSurfaceProps["canSend"];
  chips: ComposerSurfaceProps["chips"];
  closeModelMenu: () => void;
  closeSuggestions: () => void;
  composerState: ComposerSurfaceProps["composerState"];
  focusTextarea: () => void;
  isResuming: ComposerSurfaceProps["isResuming"];
  isRunning: ComposerSurfaceProps["isRunning"];
  isTurnInFlight: ComposerSurfaceProps["isStreaming"];
  resumeError: string;
  selectedContextWindow: ComposerSurfaceProps["selectedContextWindow"];
  selectedModel: ComposerMenuLayerProps["selectedModel"];
  selectedModelLabel: ComposerSurfaceProps["selectedModelLabel"];
  selectedProvider: ComposerMenuLayerProps["selectedProvider"];
  selectedProviderLabel: ComposerSurfaceProps["selectedProviderLabel"];
  sessionId: string;
  showModelMenu: ComposerMenuLayerProps["showModelMenu"];
  showSuggestions: ComposerMenuLayerProps["showSuggestions"];
  value: ComposerSurfaceProps["value"];
  workflow: WorkflowState | null;
  onActiveSuggestionIndexChange: ComposerMenuLayerProps["onActiveSuggestionIndexChange"];
  onAddChip: ComposerMenuLayerProps["onAddChip"];
  onCompositionEnd: ComposerSurfaceProps["onCompositionEnd"];
  onCompositionStart: ComposerSurfaceProps["onCompositionStart"];
  onRemoveChip: ComposerSurfaceProps["onRemoveChip"];
  onResume: ComposerSurfaceProps["onResume"];
  onSelectModel: ComposerMenuLayerProps["onSelectModel"];
  onSend: ComposerSurfaceProps["onSend"];
  onStop: ComposerSurfaceProps["onStop"];
  onTextChange: ComposerSurfaceProps["onTextChange"];
  onTextKeyDown: ComposerSurfaceProps["onTextKeyDown"];
  onToggleModelMenu: ComposerSurfaceProps["onToggleModelMenu"];
  onToggleSuggestion: ComposerSurfaceProps["onToggleSuggestion"];
}

export function useComposerPresentation({
  activeSuggestionIndex,
  atResults,
  canSend,
  chips,
  closeModelMenu,
  closeSuggestions,
  composerState,
  focusTextarea,
  isResuming,
  isRunning,
  isTurnInFlight,
  onActiveSuggestionIndexChange,
  onAddChip,
  onCompositionEnd,
  onCompositionStart,
  onRemoveChip,
  onResume,
  onSelectModel,
  onSend,
  onStop,
  onTextChange,
  onTextKeyDown,
  onToggleModelMenu,
  onToggleSuggestion,
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
}: UseComposerPresentationOptions) {
  const composerRootRef = useRef<HTMLDivElement>(null);
  const suggestionListId = `${sessionId}-composer-suggestions`;
  const modelMenuId = `${sessionId}-model-menu`;

  useEffect(() => { if (isRunning) focusTextarea(); }, [focusTextarea, isRunning, sessionId]);

  const dismissMenus = useCallback(() => {
    closeSuggestions();
    closeModelMenu();
  }, [closeModelMenu, closeSuggestions]);
  useComposerMenuDismissal({
    isMenuOpen: Boolean(showSuggestions || showModelMenu),
    onDismiss: dismissMenus,
    rootRef: composerRootRef,
  });

  const menuLayerProps = buildComposerMenuLayerProps({
    activeSuggestionIndex,
    atResults,
    modelMenuId,
    onActiveSuggestionIndexChange,
    onAddChip,
    onSelectModel,
    selectedModel,
    selectedProvider,
    showModelMenu,
    showSuggestions,
    suggestionListId,
  });
  const surfaceProps = buildComposerSurfaceProps({
    canSend,
    chips,
    composerState,
    isResuming,
    isRunning,
    isTurnInFlight,
    modelMenuId,
    onCompositionStart,
    onCompositionEnd,
    onRemoveChip,
    onResume,
    onSend,
    onStop,
    onTextChange,
    onTextKeyDown,
    onToggleModelMenu,
    onToggleSuggestion,
    selectedContextWindow,
    selectedModelLabel,
    selectedProviderLabel,
    showModelMenu,
    showSuggestions,
    suggestionListId,
    value,
    workflow,
  });

  return {
    composerRootRef,
    menuLayerProps,
    resumeErrorMessage: isRunning ? "" : resumeError,
    surfaceProps,
  };
}
