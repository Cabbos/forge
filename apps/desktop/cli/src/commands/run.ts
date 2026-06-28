import type { CliDeps } from "../cli.ts";
import { buildHeadlessRequest, runHeadlessJson } from "../lib/headless.ts";

/** Parsed CLI arguments for the `run` command. */
export type ParsedRunArgs = {
  prompt: string;
  profileId?: string;
  provider?: string;
  model?: string;
  workspacePath?: string;
};

/**
 * Parse argv for the `forge run` subcommand.
 *
 * Supported flags:
 *   --profile, -p <id>    Named profile id
 *   --provider <name>     AI provider
 *   --model, -m <name>    Model name
 *   --workspace, -w <path> Working directory
 *   --prompt <text>       The prompt to send
 *
 * The first positional argument is treated as the prompt when `--prompt` is
 * absent.
 */
export function parseRunArgs(argv: string[]): ParsedRunArgs {
  const result: ParsedRunArgs = { prompt: "" };
  let i = 0;

  while (i < argv.length) {
    const arg = argv[i];

    if (arg === "--profile" || arg === "-p") {
      const val = argv[i + 1];
      if (!val || val.startsWith("--")) {
        throw new Error("--profile requires a value");
      }
      result.profileId = val;
      i += 2;
    } else if (arg === "--provider") {
      result.provider = argv[i + 1];
      i += 2;
    } else if (arg === "--model" || arg === "-m") {
      result.model = argv[i + 1];
      i += 2;
    } else if (arg === "--workspace" || arg === "-w") {
      result.workspacePath = argv[i + 1];
      i += 2;
    } else if (arg === "--prompt") {
      result.prompt = argv[i + 1] || "";
      i += 2;
    } else if (!arg.startsWith("-")) {
      // Positional argument — use as prompt if not already set.
      if (!result.prompt) {
        result.prompt = arg;
      }
      i += 1;
    } else {
      // Unknown flag — skip it and its potential value.
      i += 1;
    }
  }

  if (!result.prompt.trim()) {
    throw new Error("prompt is required");
  }

  return result;
}

/**
 * Execute `forge run` — parse args, build a headless request, run, and print
 * the JSON result.
 */
export async function runCommand(argv: string[], deps: CliDeps = {}): Promise<number> {
  try {
    const parsed = parseRunArgs(argv);
    const forgeRepoRoot = deps.cwd || process.cwd();

    const request = buildHeadlessRequest({
      prompt: parsed.prompt,
      provider: parsed.provider || "deepseek",
      model: parsed.model || "deepseek-chat",
      workspacePath: parsed.workspacePath || process.cwd(),
      profileId: parsed.profileId,
    });

    deps.io?.stdout(`Starting headless run with profile: ${parsed.profileId || "(none)"}\n`);

    const result = await runHeadlessJson({
      forgeRepoRoot,
      request,
      spawn: deps.spawn,
    });

    deps.io?.stdout(JSON.stringify(result, null, 2) + "\n");
    return 0;
  } catch (error) {
    deps.io?.stderr(`Error: ${error instanceof Error ? error.message : String(error)}\n`);
    return 1;
  }
}
