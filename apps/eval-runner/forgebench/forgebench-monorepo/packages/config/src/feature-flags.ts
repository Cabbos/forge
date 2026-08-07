export function parseFeatureFlag(value: string | undefined, fallback = false): boolean {
  if (value === undefined) {
    return fallback;
  }
  if (value === "true") {
    return true;
  }
  if (value === "false") {
    return false;
  }
  throw new Error("feature flag must be true or false");
}
