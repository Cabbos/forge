import { ComposerModelMenu } from "./ComposerModelMenu";
import { ComposerSuggestionMenu } from "./ComposerSuggestionMenu";
import type { ProviderDefinition } from "@/lib/providers";
import type { ComposerChip, ComposerMenuMode } from "./composerTypes";

export interface ComposerMenuLayerProps {
  activeSuggestionIndex: number;
  atResults: string[];
  modelMenuId: string;
  selectedModel: string;
  selectedProvider: string;
  providers: ProviderDefinition[];
  showModelMenu: boolean;
  showSuggestions: ComposerMenuMode;
  suggestionListId: string;
  onActiveSuggestionIndexChange: (index: number) => void;
  onAddChip: (type: ComposerChip["type"], value: string) => void;
  onSelectModel: (provider: string, model: string) => void;
}

export function ComposerMenuLayer({
  activeSuggestionIndex,
  atResults,
  modelMenuId,
  onActiveSuggestionIndexChange,
  onAddChip,
  onSelectModel,
  providers,
  selectedModel,
  selectedProvider,
  showModelMenu,
  showSuggestions,
  suggestionListId,
}: ComposerMenuLayerProps) {
  return (
    <>
      {showSuggestions && (
        <ComposerSuggestionMenu
          id={suggestionListId}
          mode={showSuggestions}
          atResults={atResults}
          activeIndex={activeSuggestionIndex}
          onActiveIndexChange={onActiveSuggestionIndexChange}
          onAddChip={onAddChip}
        />
      )}

      {showModelMenu && (
        <ComposerModelMenu
          id={modelMenuId}
          labelledBy={`${modelMenuId}-button`}
          providers={providers}
          selectedModel={selectedModel}
          selectedProvider={selectedProvider}
          onSelect={onSelectModel}
        />
      )}
    </>
  );
}
