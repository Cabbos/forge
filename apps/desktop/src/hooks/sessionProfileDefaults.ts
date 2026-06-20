import {
  getDefaultModel,
  getProviderForModel,
  normalizeProviderId,
} from "../lib/providers.ts";
import type { ProviderDefinition } from "../lib/providers.ts";
import type { ForgeProfile, ProfileListPayload } from "../lib/ipc/types";

export interface ResolveProfileSessionDefaultsInput {
  workingDir: string;
  provider: string;
  model: string;
  profiles?: ProfileListPayload | null;
  providers?: ProviderDefinition[];
}

export interface ResolvedProfileSessionDefaults {
  workingDir: string;
  provider: string;
  model: string;
  profileId: string | null;
}

export interface ResolveProfileComposerDefaultsInput {
  currentProvider: string;
  currentModel: string;
  profile?: ForgeProfile | null;
  providers?: ProviderDefinition[];
}

export interface ResolvedProfileComposerDefaults {
  provider: string;
  model: string;
  changed: boolean;
}

export function resolveProfileSessionDefaults(
  input: ResolveProfileSessionDefaultsInput,
): ResolvedProfileSessionDefaults {
  const activeProfile = input.profiles?.profiles.find(
    (profile) => profile.id === input.profiles?.active_profile_id,
  );

  if (!activeProfile) {
    return {
      workingDir: input.workingDir,
      provider: input.provider,
      model: input.model,
      profileId: null,
    };
  }

  const profileProvider = cleanProfileDefault(activeProfile.default_provider);
  const profileModel = cleanProfileDefault(activeProfile.default_model);
  const provider = profileProvider
    ? normalizeProviderId(profileProvider, input.providers)
    : profileModel
      ? getProviderForModel(profileModel, input.providers) ?? input.provider
      : input.provider;
  const model = profileModel ?? (profileProvider ? getDefaultModel(provider, input.providers) : input.model);

  return {
    workingDir: cleanProfileDefault(activeProfile.default_workspace) ?? input.workingDir,
    provider,
    model,
    profileId: activeProfile.id,
  };
}

export function resolveProfileComposerDefaults(
  input: ResolveProfileComposerDefaultsInput,
): ResolvedProfileComposerDefaults {
  const profileProvider = cleanProfileDefault(input.profile?.default_provider);
  const profileModel = cleanProfileDefault(input.profile?.default_model);
  const providers = input.providers;

  if (!profileProvider && !profileModel) {
    return {
      provider: input.currentProvider,
      model: input.currentModel,
      changed: false,
    };
  }

  const provider = profileProvider
    ? normalizeProviderId(profileProvider, providers)
    : getProviderForModel(profileModel, providers) ?? normalizeProviderId(input.currentProvider, providers);
  const model = profileModel ?? getDefaultModel(provider, providers);

  return {
    provider,
    model,
    changed: provider !== input.currentProvider || model !== input.currentModel,
  };
}

function cleanProfileDefault(value: string | null | undefined): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}
