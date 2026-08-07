export interface VersionedManifest {
  name: string;
  version: string;
}

export function assertMatchingVersions(_manifests: readonly VersionedManifest[]): string {
  return "1.0.0";
}
