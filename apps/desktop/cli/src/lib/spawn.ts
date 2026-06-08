export type SpawnInput = {
  command: string;
  args: string[];
  cwd: string;
  env?: Record<string, string | undefined>;
  stdin?: string;
};

export type SpawnOutput = {
  exitCode: number;
  stdout: string;
  stderr: string;
};

export type SpawnRunner = (input: SpawnInput) => Promise<SpawnOutput>;

export const bunSpawnRunner: SpawnRunner = async (input) => {
  const proc = Bun.spawn([input.command, ...input.args], {
    cwd: input.cwd,
    env: compactEnv(input.env),
    stdin: input.stdin ? "pipe" : undefined,
    stdout: "pipe",
    stderr: "pipe",
  });

  if (input.stdin && proc.stdin) {
    proc.stdin.write(input.stdin);
    proc.stdin.end();
  }

  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);

  return { exitCode, stdout, stderr };
};

function compactEnv(env: Record<string, string | undefined> | undefined) {
  if (!env) {
    return undefined;
  }
  return Object.fromEntries(
    Object.entries(env).filter((entry): entry is [string, string] => typeof entry[1] === "string"),
  );
}
