import { set as idbSet } from "idb-keyval";
import {
  getDefaultModel,
  modelBelongsToProvider,
  normalizeProviderId,
} from "../lib/providers";
import { hasTauriRuntime } from "../lib/tauri";
import {
  MODEL_KEY,
  PROVIDER_KEY,
  persistBackendAppMetadata,
} from "./persistence";
import type { AppStore } from "./types";

type StoreSet = (partial: Partial<AppStore>) => void;
type StoreGet = () => AppStore;
type PreferencesActions = Pick<
  AppStore,
  "setSelectedProvider" | "setSelectedModel" | "setTheme"
>;

export function createPreferencesActions(
  set: StoreSet,
  get: StoreGet,
): PreferencesActions {
  return {
    setSelectedProvider: (p) => {
      const selectedProvider = normalizeProviderId(p);
      const currentModel = get().selectedModel;
      const selectedModel = modelBelongsToProvider(selectedProvider, currentModel)
        ? currentModel
        : getDefaultModel(selectedProvider);
      set({ selectedProvider, selectedModel });
      if (hasTauriRuntime()) {
        persistBackendAppMetadata({
          workspaces: get().workspaces,
          activeWorkspaceId: get().activeWorkspaceId,
          activeSessionId: get().activeSessionId,
          selectedProvider,
          selectedModel,
        });
      } else {
        idbSet(PROVIDER_KEY, selectedProvider).catch(() => {});
        idbSet(MODEL_KEY, selectedModel).catch(() => {});
      }
    },

    setSelectedModel: (m) => {
      set({ selectedModel: m });
      if (hasTauriRuntime()) {
        persistBackendAppMetadata({
          workspaces: get().workspaces,
          activeWorkspaceId: get().activeWorkspaceId,
          activeSessionId: get().activeSessionId,
          selectedProvider: get().selectedProvider,
          selectedModel: m,
        });
      } else {
        idbSet(MODEL_KEY, m).catch(() => {});
      }
    },

    setTheme: (theme) => {
      set({ theme });
      idbSet("tui-theme", theme).catch(() => {});
    },
  };
}
