import { expect, test } from "bun:test";
import { parsePort } from "../../packages/config/src/env";

test("configured ports must be valid TCP integers", () => {
  for (const value of ["", "abc", "3.5", "0", "65536"]) {
    expect(() => parsePort(value)).toThrow();
  }
  expect(parsePort(undefined, 4100)).toBe(4100);
});
