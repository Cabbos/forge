import { useCallback, useState } from "react";
import { formatContextWindow, getModelContextWindow, getModelLabel, getProviderDefinition } from "@/lib/providers";
import { useStore } from "@/store";

export function useComposerModelMenu() {
  const [showModelMenu, setShowModelMenu] = useState(false);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const setSelectedProvider = useStore((s) => s.setSelectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const setSelectedModel = useStore((s) => s.setSelectedModel);

  const closeModelMenu = useCallback(() => {
    setShowModelMenu(false);
  }, []);

  const toggleModelMenu = useCallback(() => {
    setShowModelMenu((current) => !current);
  }, []);

  const selectModel = useCallback((provider: string, model: string) => {
    setSelectedProvider(provider);
    setSelectedModel(model);
    closeModelMenu();
  }, [closeModelMenu, setSelectedModel, setSelectedProvider]);

  return {
    closeModelMenu,
    selectModel,
    selectedContextWindow: formatContextWindow(getModelContextWindow(selectedModel)),
    selectedModel,
    selectedModelLabel: getModelLabel(selectedModel),
    selectedProvider,
    selectedProviderLabel: getProviderDefinition(selectedProvider).label,
    showModelMenu,
    toggleModelMenu,
  };
}
