import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type {
  ForgeProfile,
  ProfileListPayload,
  UpsertProfileInput,
} from "./types";

export async function listProfiles(): Promise<ProfileListPayload> {
  if (!hasTauriRuntime()) {
    return { profiles: [], active_profile_id: null };
  }
  return invoke<ProfileListPayload>("list_profiles");
}

export async function upsertProfile(input: UpsertProfileInput): Promise<ForgeProfile> {
  return invoke<ForgeProfile>("upsert_profile", { input });
}

export async function deleteProfile(id: string): Promise<boolean> {
  return invoke<boolean>("delete_profile", { id });
}

export async function setActiveProfile(id: string): Promise<ProfileListPayload> {
  return invoke<ProfileListPayload>("set_active_profile", { id });
}
