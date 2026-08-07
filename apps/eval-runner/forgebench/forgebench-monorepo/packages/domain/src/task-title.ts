export function normalizeTaskTitle(value: string): string {
  return value.normalize("NFKC").trim().replace(/\s+/gu, " ");
}
