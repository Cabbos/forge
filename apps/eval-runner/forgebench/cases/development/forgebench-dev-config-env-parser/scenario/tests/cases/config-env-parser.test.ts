import { expect, test } from "bun:test";
import { parsePort } from "../../packages/config/src/env";

test("configured ports must be valid TCP integers", () => {
  for (const value of [
    "",
    " ",
    "+80",
    "080",
    "abc",
    "3.5",
    "0",
    "65536",
    "9007199254740992",
  ]) {
    expect(() => parsePort(value)).toThrow();
  }
  expect(parsePort(undefined, 4100)).toBe(4100);
});
