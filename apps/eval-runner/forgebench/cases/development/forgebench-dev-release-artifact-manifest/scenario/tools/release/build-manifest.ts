import { readdir } from "node:fs/promises";
import { join, relative } from "node:path";

export interface ManifestEntry {
  path: string;
  sha256: string;
}

async function sourceFiles(root: string, directory = root): Promise<string[]> {
  const files: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await sourceFiles(root, path)));
    } else {
      files.push(path);
    }
  }
  return files;
}

export async function buildManifest(root: string): Promise<ManifestEntry[]> {
  return (await sourceFiles(root)).map((path) => ({
    path: relative(root, path),
    sha256: String(path.length),
  }));
}
