import { describe, expect, test } from "bun:test";
import { parseFeatureFlag, parsePort } from "../../packages/config/src";

describe("configuration", () => {
  test("parses valid values", () => {
    expect(parsePort("8080")).toBe(8080);
    expect(parsePort(undefined, 4000)).toBe(4000);
    expect(parseFeatureFlag("true")).toBe(true);
    expect(parseFeatureFlag("false")).toBe(false);
  });

  test("rejects ambiguous values", () => {
    expect(() => parsePort("0")).toThrow();
    expect(() => parsePort("65536")).toThrow();
    expect(() => parsePort("3.5")).toThrow();
    expect(() => parseFeatureFlag("yes")).toThrow();
  });
});
