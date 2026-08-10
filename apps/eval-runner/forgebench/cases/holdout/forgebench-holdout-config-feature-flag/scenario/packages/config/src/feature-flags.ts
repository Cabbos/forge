export function parseFeatureFlag(value: string | undefined, fallback = false): boolean {
  if (value === undefined) {
    return fallback;
  }
  return ["true", "yes", "1"].includes(value.toLowerCase());
}
