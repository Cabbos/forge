import assert from "node:assert/strict";
import { describe, it } from "node:test";
import { createTerminalOutputSanitizer } from "./workPanelTerminalOutput.ts";

describe("createTerminalOutputSanitizer", () => {
  it("keeps plain text and normalizes carriage-return-only output", () => {
    const sanitizer = createTerminalOutputSanitizer();

    assert.equal(sanitizer.push("first\rsecond\r\nthird"), "first\nsecond\r\nthird");
  });

  it("removes CSI sequences split across chunks", () => {
    const sanitizer = createTerminalOutputSanitizer();

    assert.equal(sanitizer.push("ready\u001b[31"), "ready");
    assert.equal(sanitizer.push("m red\u001b[0m"), " red");
  });

  it("removes OSC BEL sequences split across chunks", () => {
    const sanitizer = createTerminalOutputSanitizer();

    assert.equal(sanitizer.push("before\u001b]0;title"), "before");
    assert.equal(sanitizer.push("\u0007after"), "after");
  });

  it("removes OSC ST sequences split across chunks", () => {
    const sanitizer = createTerminalOutputSanitizer();

    assert.equal(sanitizer.push("before\u001b]8;;https://forge"), "before");
    assert.equal(sanitizer.push(".local\u001b"), "");
    assert.equal(sanitizer.push("\\after"), "after");
  });

  it("forgets an unfinished control sequence when reset", () => {
    const sanitizer = createTerminalOutputSanitizer();

    assert.equal(sanitizer.push("before\u001b]0;title"), "before");
    sanitizer.reset();
    assert.equal(sanitizer.push("after"), "after");
  });
});
