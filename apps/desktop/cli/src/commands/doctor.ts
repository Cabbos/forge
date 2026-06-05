import type { CliDeps } from "../cli.ts";

export async function runDoctor(_argv: string[], deps: CliDeps = {}): Promise<number> {
  deps.io?.stdout("Forge doctor is not wired yet.\n");
  return 0;
}
