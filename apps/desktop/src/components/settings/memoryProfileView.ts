import type {
  ForgeProfile,
  MemoryFact,
  ProfileListPayload,
  UpsertMemoryFactInput,
} from "../../lib/ipc/types";

export function resolveActiveMemoryProfile(
  profiles?: ProfileListPayload | null,
): ForgeProfile | null {
  const activeProfileId = resolveMemoryProfileId(profiles?.active_profile_id);
  if (!activeProfileId) return null;
  return profiles?.profiles.find((profile) => profile.id === activeProfileId) ?? null;
}

export function resolveMemoryProfileId(
  profileId: string | null | undefined,
): string | null {
  const trimmed = profileId?.trim();
  return trimmed ? trimmed : null;
}

export function buildMemoryFactUpsertInput({
  fact,
  text,
  tags,
  activeProfileId,
}: {
  fact?: MemoryFact | null;
  text: string;
  tags: string[];
  activeProfileId?: string | null;
}): UpsertMemoryFactInput {
  const profileId = resolveMemoryProfileId(fact?.profile_id) ?? resolveMemoryProfileId(activeProfileId);
  const input: UpsertMemoryFactInput = {
    ...(fact ? { id: fact.id } : {}),
    text,
    tags,
  };

  if (profileId) {
    input.profile_id = profileId;
  }

  return input;
}
