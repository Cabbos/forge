import { describe, it } from "node:test";
import assert from "node:assert";
import { deriveDiffView } from "./diffPresentation.ts";

describe("deriveDiffView", () => {
  it("derives a compact file tree from multi-file git diffs", () => {
    const diff = [
      "diff --git a/src/App.tsx b/src/App.tsx",
      "index 111..222 100644",
      "--- a/src/App.tsx",
      "+++ b/src/App.tsx",
      "@@ -1,2 +1,3 @@",
      " import React from 'react';",
      "-const label = 'Old';",
      "+const label = 'New';",
      "+const enabled = true;",
      "diff --git a/docs/runtime.md b/docs/runtime.md",
      "new file mode 100644",
      "--- /dev/null",
      "+++ b/docs/runtime.md",
      "@@ -0,0 +1,2 @@",
      "+# Runtime",
      "+Gateway status",
    ].join("\n");

    const view = deriveDiffView(diff, false);

    assert.strictEqual(view.fileCount, 2);
    assert.deepStrictEqual(view.files, [
      { path: "src/App.tsx", additions: 2, deletions: 1, status: "modified" },
      { path: "docs/runtime.md", additions: 2, deletions: 0, status: "added" },
    ]);
  });

  it("limits visible file tree entries while preserving the full file count", () => {
    const diff = Array.from({ length: 10 }, (_, index) => {
      const name = `src/file-${index}.ts`;
      return [
        `diff --git a/${name} b/${name}`,
        `--- a/${name}`,
        `+++ b/${name}`,
        "@@ -1 +1 @@",
        "-old",
        "+new",
      ].join("\n");
    }).join("\n");

    const view = deriveDiffView(diff, false);

    assert.strictEqual(view.fileCount, 10);
    assert.strictEqual(view.visibleFiles.length, 6);
    assert.strictEqual(view.hiddenFileCount, 4);
  });

  it("derives an image diff preview from image file contents", () => {
    const oldSvg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="red" /></svg>';
    const newSvg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><rect width="10" height="10" fill="green" /></svg>';

    const view = deriveDiffView(newSvg, false, {
      filePath: "assets/logo.svg",
      oldContent: oldSvg,
      newContent: newSvg,
    });

    assert.strictEqual(view.imageDiff?.filePath, "assets/logo.svg");
    assert.strictEqual(view.imageDiff?.beforeLabel, "之前");
    assert.strictEqual(view.imageDiff?.afterLabel, "之后");
    assert.ok(view.imageDiff?.beforeSrc?.startsWith("data:image/svg+xml;utf8,"));
    assert.ok(view.imageDiff?.afterSrc?.startsWith("data:image/svg+xml;utf8,"));
  });
});
