import { useCallback, useState } from "react";
import { formatContextWindow, getModelContextWindow, getModelLabel, getProviderDefinition } from "@/lib/providers";
import { useProviderCatalog } from "@/hooks/queries/useProviderCatalogQuery";
import { useStore } from "@/store";

export function useComposerModelMenu() {
  const [showModelMenu, setShowModelMenu] = useState(false);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const setSelectedProvider = useStore((s) => s.setSelectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const setSelectedModel = useStore((s) => s.setSelectedModel);
  const providers = useProviderCatalog();

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
    providers,
    selectedContextWindow: formatContextWindow(getModelContextWindow(selectedModel, providers)),
    selectedModel,
    selectedModelLabel: getModelLabel(selectedModel, providers),
    selectedProvider,
    selectedProviderLabel: getProviderDefinition(selectedProvider, providers).label,
    showModelMenu,
    toggleModelMenu,
  };
}
