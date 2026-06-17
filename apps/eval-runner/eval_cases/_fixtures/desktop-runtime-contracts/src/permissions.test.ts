import assert from "node:assert/strict";
import test from "node:test";

import { decidePermission, type PermissionRule } from "./permissions.ts";

test("deny rule takes precedence over broader allow rule", () => {
  const rules: PermissionRule[] = [
    { pattern: "/workspace", action: "allow" },
    { pattern: "/workspace/.env", action: "deny" }
  ];

  assert.equal(decidePermission("/workspace/.env", rules), "deny");
});

test("specific allow rule still permits normal workspace file", () => {
  const rules: PermissionRule[] = [{ pattern: "/workspace", action: "allow" }];

  assert.equal(decidePermission("/workspace/src/app.ts", rules), "allow");
});
