import type { CliDeps } from "../cli.ts";

export async function runCommand(_argv: string[], deps: CliDeps = {}): Promise<number> {
  deps.io?.stdout("Forge run is not wired yet.\n");
  return 0;
}
