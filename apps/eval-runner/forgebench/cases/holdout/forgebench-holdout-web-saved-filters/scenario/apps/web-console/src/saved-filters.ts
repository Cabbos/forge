import type { TaskQuery } from "../../../packages/domain/src";

export interface SavedFilter {
  name: string;
  query: TaskQuery;
}

export function serializeSavedFilters(filters: readonly SavedFilter[]): string {
  return JSON.stringify(filters);
}

export function deserializeSavedFilters(value: string): SavedFilter[] {
  return JSON.parse(value) as SavedFilter[];
}
