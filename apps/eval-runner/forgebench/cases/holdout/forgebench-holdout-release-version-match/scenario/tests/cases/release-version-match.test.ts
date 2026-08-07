import { expect, test } from "bun:test";
import { assertMatchingVersions } from "../../tools/release/version-match";

test("version parity rejects a mismatched service", () => {
  expect(() =>
    assertMatchingVersions([
      { name: "root", version: "1.0.0" },
      { name: "task-api", version: "1.0.1" },
      { name: "event-worker", version: "1.0.0" },
    ]),
  ).toThrow(/task-api/u);
});
