import { execFileSync } from "node:child_process";
import { chmodSync, existsSync } from "node:fs";
import { join } from "node:path";

function run() {
  let repoRoot;

  try {
    repoRoot = execFileSync("git", ["rev-parse", "--show-toplevel"], {
      encoding: "utf8",
    }).trim();
  } catch {
    console.log("[hooks] Not inside a git repository; skipping hook install.");
    return;
  }

  const hooksDir = join(repoRoot, ".githooks");
  if (!existsSync(hooksDir)) {
    console.log("[hooks] .githooks directory is missing; skipping hook install.");
    return;
  }

  execFileSync("git", ["config", "core.hooksPath", ".githooks"], {
    cwd: repoRoot,
    stdio: "inherit",
  });

  for (const hookName of ["pre-commit", "pre-push"]) {
    const hookPath = join(hooksDir, hookName);
    if (existsSync(hookPath)) {
      chmodSync(hookPath, 0o755);
    }
  }

  console.log("[hooks] Git hooks installed from .githooks.");
}

run();
