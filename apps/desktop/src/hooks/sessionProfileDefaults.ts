import type { ProfileListPayload } from "../lib/ipc/types";

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

function cleanProfileDefault(value: string | null | undefined): string | null {
  const trimmed = value?.trim();
  return trimmed ? trimmed : null;
}
