import test from "node:test";
import assert from "node:assert/strict";
import {
  classifyDoctorReport,
  exitCodeForDoctorReport,
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
