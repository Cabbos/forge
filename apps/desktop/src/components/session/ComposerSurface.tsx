import * as React from "react";
import { ComposerChipTray } from "./ComposerChipTray";
import { ComposerTextarea } from "./ComposerTextarea";
import { ComposerToolbar } from "./ComposerToolbar";
import type { ComposerSurfaceState } from "./composerTurnState";
import type { ComposerChip, ComposerMenuMode } from "./composerTypes";

export interface ComposerSurfaceProps {
  canSend: boolean;
  chips: ComposerChip[];
  composerState: ComposerSurfaceState;
  isResuming: boolean;
  isRunning: boolean;
  isStreaming: boolean;
  modelMenuId: string;
  placeholder: string;
  selectedContextWindow: string;
  selectedModelLabel: string;
  selectedProviderLabel: string;
  showModelMenu: boolean;
  showSuggestions: ComposerMenuMode;
  suggestionListId: string;
  value: string;
  onCompositionEnd: React.CompositionEventHandler<HTMLTextAreaElement>;
  onCompositionStart: React.CompositionEventHandler<HTMLTextAreaElement>;
  onRemoveChip: (chipId: string) => void;
  onResume: () => void;
  onSend: () => void;
  onStop: () => void;
  onTextChange: React.ChangeEventHandler<HTMLTextAreaElement>;
  onTextKeyDown: React.KeyboardEventHandler<HTMLTextAreaElement>;
  onToggleModelMenu: () => void;
  onToggleSuggestion: (mode: Exclude<ComposerMenuMode, null>) => void;
}

const ComposerSurface = React.forwardRef<HTMLTextAreaElement, ComposerSurfaceProps>(function ComposerSurface({
  canSend,
  chips,
  composerState,
  isResuming,
  isRunning,
  isStreaming,
  modelMenuId,
  onCompositionEnd,
  onCompositionStart,
  onRemoveChip,
  onResume,
  onSend,
  onStop,
  onTextChange,
  onTextKeyDown,
  onToggleModelMenu,
  onToggleSuggestion,
  placeholder,
  selectedContextWindow,
  selectedModelLabel,
  selectedProviderLabel,
  showModelMenu,
  showSuggestions,
  suggestionListId,
  value,
}, ref) {
  return (
    <div
      data-testid="composer-surface"
      data-menu-open={showSuggestions || showModelMenu ? "true" : "false"}
      data-streaming={isStreaming ? "true" : "false"}
      data-state={composerState}
      className="forge-composer"
    >
      <ComposerChipTray chips={chips} onRemove={onRemoveChip} />

      <ComposerTextarea
        ref={ref}
        value={value}
        onChange={onTextChange}
        onKeyDown={onTextKeyDown}
        onCompositionStart={onCompositionStart}
        onCompositionEnd={onCompositionEnd}
        placeholder={placeholder}
        disabled={!isRunning}
        hasChips={chips.length > 0}
      />

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
        onResume={onResume}
        onSend={onSend}
        onStop={onStop}
        onToggleModelMenu={onToggleModelMenu}
        onToggleSuggestion={onToggleSuggestion}
      />
    </div>
  );
});

ComposerSurface.displayName = "ComposerSurface";

export { ComposerSurface };
