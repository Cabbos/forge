import { readdir } from "node:fs/promises";
import { join, relative } from "node:path";
import { sha256File } from "./checksums";

const IGNORED = new Set([".git", ".venv", "node_modules", "dist", "coverage"]);

export interface ManifestEntry {
  path: string;
  sha256: string;
}

async function sourceFiles(root: string, directory = root): Promise<string[]> {
  const files: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (IGNORED.has(entry.name)) {
      continue;
    }
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await sourceFiles(root, path)));
    } else if (entry.isFile()) {
      files.push(path);
    }
  }
  return files;
}

export async function buildManifest(root: string): Promise<ManifestEntry[]> {
  const files = await sourceFiles(root);
  const entries = await Promise.all(
    files.map(async (path) => ({
      path: relative(root, path).replaceAll("\\", "/"),
      sha256: await sha256File(path),
    })),
  );
  return entries.sort((left, right) => left.path.localeCompare(right.path));
}
