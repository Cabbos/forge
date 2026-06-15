import {
  getDefaultModel,
  getProviderForModel,
  normalizeProviderId,
} from "../lib/providers.ts";
import type { ForgeProfile, ProfileListPayload } from "../lib/ipc/types";

export interface ResolveProfileSessionDefaultsInput {
  workingDir: string;
  provider: string;
  model: string;
  profiles?: ProfileListPayload | null;
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

  return {
    workingDir: cleanProfileDefault(activeProfile.default_workspace) ?? input.workingDir,
    provider: cleanProfileDefault(activeProfile.default_provider) ?? input.provider,
    model: cleanProfileDefault(activeProfile.default_model) ?? input.model,
    profileId: activeProfile.id,
  };
}

export function resolveProfileComposerDefaults(
  input: ResolveProfileComposerDefaultsInput,
): ResolvedProfileComposerDefaults {
  const profileProvider = cleanProfileDefault(input.profile?.default_provider);
  const profileModel = cleanProfileDefault(input.profile?.default_model);

  if (!profileProvider && !profileModel) {
    return {
      provider: input.currentProvider,
      model: input.currentModel,
      changed: false,
    };
  }

  const provider = profileProvider
    ? normalizeProviderId(profileProvider)
    : getProviderForModel(profileModel) ?? normalizeProviderId(input.currentProvider);
  const model = profileModel ?? getDefaultModel(provider);

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
