import test from "node:test";
import assert from "node:assert/strict";
import { existsSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  classifyDoctorReport,
  exitCodeForDoctorReport,
  runDoctorIsolated,
} from "./eval-doctor-precheck.mjs";

test("doctor precheck passes clean reports", () => {
  const report = {
    checks: [
      { name: "bun", ok: true, message: "bun 1.0.0" },
      { name: "forge_repo_root", ok: true, message: "/repo" },
    ],
  };

  assert.equal(exitCodeForDoctorReport(report), 0);
  assert.equal(classifyDoctorReport(report).hardFails.length, 0);
});

test("doctor precheck treats missing forge data as a soft fresh-install failure", () => {
  const report = {
    checks: [
      {
        name: "forge_logs",
        ok: false,
        message: "Data directory /Users/example/.forge does not exist.",
      },
    ],
  };

  const classified = classifyDoctorReport(report);
  assert.equal(classified.softFails.length, 1);
  assert.equal(classified.hardFails.length, 0);
  assert.equal(exitCodeForDoctorReport(report), 0);
});

test("doctor precheck fails hard doctor failures", () => {
  const report = {
    checks: [
      {
        name: "forge_config",
        ok: false,
        message: "Config corrupted: expected value",
      },
      {
        name: "forge_logs",
        ok: false,
        message: "Data directory /Users/example/.forge is not writable.",
      },
    ],
  };

  const classified = classifyDoctorReport(report);
  assert.equal(classified.softFails.length, 0);
  assert.equal(classified.hardFails.length, 2);
  assert.equal(exitCodeForDoctorReport(report), 1);
});

test("doctor precheck runs against a disposable Forge home", () => {
  let captured;
  const result = runDoctorIsolated({
    spawn(command, args, options) {
      captured = { command, args, options };
      assert.equal(existsSync(options.env.HOME), true);
      assert.equal(existsSync(join(options.env.HOME, ".forge")), true);
      return { stdout: '{"ok":true,"checks":[]}', stderr: "", status: 0 };
    },
  });

  assert.equal(captured.command, "bun");
  assert.deepEqual(captured.args.slice(-2), ["doctor", "--json"]);
  assert.equal(captured.options.env.HOME, captured.options.env.USERPROFILE);
  assert.equal(existsSync(result.isolatedHome), false);
});

test("a corrupted user snapshot cannot enter the isolated doctor report", () => {
  const userHome = mkdtempSync(join(tmpdir(), "forge-user-home-"));
  mkdirSync(join(userHome, ".forge", "sessions"), { recursive: true });
  writeFileSync(join(userHome, ".forge", "sessions", "broken.json"), "{");
  let evaluatedHome;

  try {
    runDoctorIsolated({
      baseEnv: { ...process.env, HOME: userHome, USERPROFILE: userHome },
      spawn(_command, _args, options) {
        evaluatedHome = options.env.HOME;
        assert.notEqual(evaluatedHome, userHome);
        assert.equal(
          existsSync(join(evaluatedHome, ".forge", "sessions", "broken.json")),
          false,
        );
        return { stdout: '{"ok":true,"checks":[]}', stderr: "", status: 0 };
      },
    });
  } finally {
    rmSync(userHome, { recursive: true, force: true });
  }
});

test("isolated doctor home preserves the original Rust toolchain roots", () => {
  const originalHome = join(tmpdir(), "forge-original-home");
  let childEnv;

  runDoctorIsolated({
    baseEnv: { HOME: originalHome, PATH: process.env.PATH },
    spawn(_command, _args, options) {
      childEnv = options.env;
      return { stdout: '{"ok":true,"checks":[]}', stderr: "", status: 0 };
    },
  });

  assert.equal(childEnv.RUSTUP_HOME, join(originalHome, ".rustup"));
  assert.equal(childEnv.CARGO_HOME, join(originalHome, ".cargo"));
  assert.notEqual(childEnv.HOME, originalHome);
});
