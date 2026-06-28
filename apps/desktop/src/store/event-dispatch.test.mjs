import { build } from "esbuild";

const result = await build({
  entryPoints: [new URL("./event-dispatch.test.ts", import.meta.url).pathname],
  bundle: true,
  write: false,
  platform: "node",
  format: "esm",
  external: ["node:assert", "node:test"],
  logLevel: "silent",
});

const code = result.outputFiles[0].text;
const dataUrl = `data:text/javascript;base64,${Buffer.from(code).toString("base64")}`;
await import(dataUrl);
