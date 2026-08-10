import type { TaskQuery } from "../../../packages/domain/src";

export interface SavedFilter {
  name: string;
  query: TaskQuery;
}

export function serializeSavedFilters(filters: readonly SavedFilter[]): string {
  return JSON.stringify(
    [...filters]
      .map((filter) => ({
        name: filter.name.trim(),
        query: {
          ...(filter.query.search ? { search: filter.query.search } : {}),
          ...(filter.query.status ? { status: filter.query.status } : {}),
        },
      }))
      .sort((left, right) => left.name.localeCompare(right.name)),
  );
}

export function deserializeSavedFilters(value: string): SavedFilter[] {
  const parsed: unknown = JSON.parse(value);
  if (!Array.isArray(parsed)) {
    throw new Error("saved filters must be an array");
  }
  return parsed as SavedFilter[];
}
