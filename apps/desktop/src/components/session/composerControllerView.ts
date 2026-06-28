import { modeAwarePlaceholder } from "@/lib/task-mode";
import type { WorkflowState } from "@/lib/protocol";
import type { ComposerMenuLayerProps } from "./ComposerMenuLayer";
import type { ComposerSurfaceProps } from "./ComposerSurface";

interface BuildComposerMenuLayerPropsOptions {
  activeSuggestionIndex: ComposerMenuLayerProps["activeSuggestionIndex"];
  atResults: ComposerMenuLayerProps["atResults"];
  modelMenuId: ComposerMenuLayerProps["modelMenuId"];
  providers: ComposerMenuLayerProps["providers"];
  selectedModel: ComposerMenuLayerProps["selectedModel"];
  selectedProvider: ComposerMenuLayerProps["selectedProvider"];
  showModelMenu: ComposerMenuLayerProps["showModelMenu"];
  showSuggestions: ComposerMenuLayerProps["showSuggestions"];
  suggestionListId: ComposerMenuLayerProps["suggestionListId"];
  onActiveSuggestionIndexChange: ComposerMenuLayerProps["onActiveSuggestionIndexChange"];
  onAddChip: ComposerMenuLayerProps["onAddChip"];
  onSelectModel: ComposerMenuLayerProps["onSelectModel"];
}

interface BuildComposerSurfacePropsOptions {
  canSend: ComposerSurfaceProps["canSend"];
  chips: ComposerSurfaceProps["chips"];
  composerState: ComposerSurfaceProps["composerState"];
  contextUsageView: ComposerSurfaceProps["contextUsageView"];
  isResuming: ComposerSurfaceProps["isResuming"];
  isRunning: ComposerSurfaceProps["isRunning"];
  isTurnInFlight: ComposerSurfaceProps["isStreaming"];
  modelMenuId: ComposerSurfaceProps["modelMenuId"];
  workflow: WorkflowState | null;
  selectedContextWindow: ComposerSurfaceProps["selectedContextWindow"];
  selectedModelLabel: ComposerSurfaceProps["selectedModelLabel"];
  selectedProviderLabel: ComposerSurfaceProps["selectedProviderLabel"];
  showModelMenu: ComposerSurfaceProps["showModelMenu"];
  showSuggestions: ComposerSurfaceProps["showSuggestions"];
  suggestionListId: ComposerSurfaceProps["suggestionListId"];
  value: ComposerSurfaceProps["value"];
  onCompositionStart: ComposerSurfaceProps["onCompositionStart"];
  onCompositionEnd: ComposerSurfaceProps["onCompositionEnd"];
  onRemoveChip: ComposerSurfaceProps["onRemoveChip"];
  onCompact: ComposerSurfaceProps["onCompact"];
  onResume: ComposerSurfaceProps["onResume"];
  onSend: ComposerSurfaceProps["onSend"];
  onStop: ComposerSurfaceProps["onStop"];
  onTextChange: ComposerSurfaceProps["onTextChange"];
  onTextKeyDown: ComposerSurfaceProps["onTextKeyDown"];
  onToggleModelMenu: ComposerSurfaceProps["onToggleModelMenu"];
  onToggleSuggestion: ComposerSurfaceProps["onToggleSuggestion"];
}

export function buildComposerMenuLayerProps({
  activeSuggestionIndex,
  atResults,
  modelMenuId,
  providers,
  onActiveSuggestionIndexChange,
  onAddChip,
  onSelectModel,
  selectedModel,
  selectedProvider,
  showModelMenu,
  showSuggestions,
  suggestionListId,
}: BuildComposerMenuLayerPropsOptions): ComposerMenuLayerProps {
  return {
    activeSuggestionIndex,
    atResults,
    modelMenuId,
    providers,
    selectedModel,
    selectedProvider,
    showModelMenu,
    showSuggestions,
    suggestionListId,
    onActiveSuggestionIndexChange,
    onAddChip,
    onSelectModel,
  };
}

export function buildComposerSurfaceProps({
  canSend,
  chips,
  composerState,
  contextUsageView,
  isResuming,
  isRunning,
  isTurnInFlight,
  modelMenuId,
  onCompositionEnd,
  onCompositionStart,
  onRemoveChip,
  onCompact,
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
}: BuildComposerSurfacePropsOptions): ComposerSurfaceProps {
  return {
    canSend,
    chips,
    composerState,
    contextUsageView,
    isResuming,
    isRunning,
    isStreaming: isTurnInFlight,
    modelMenuId,
    placeholder: modeAwarePlaceholder(workflow, isRunning),
    selectedContextWindow,
    selectedModelLabel,
    selectedProviderLabel,
    showModelMenu,
    showSuggestions,
    suggestionListId,
    value,
    onCompositionStart,
    onCompositionEnd,
    onRemoveChip,
    onCompact,
    onResume,
    onSend,
    onStop,
    onTextChange,
    onTextKeyDown,
    onToggleModelMenu,
    onToggleSuggestion,
  };
}
