export function normalizeTaskTitle(value: string): string {
  return value.trim().replace(/\s{2}/u, " ");
}
