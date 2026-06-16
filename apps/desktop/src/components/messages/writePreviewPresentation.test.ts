import { describe, it } from "node:test";
import assert from "node:assert";
import { deriveWritePreview } from "./writePreviewPresentation.ts";

describe("deriveWritePreview", () => {
  it("builds a markdown write preview from write_file content", () => {
    const preview = deriveWritePreview("write_file", {
      path: "docs/runtime.md",
      content: "# Runtime\n\n- Gateway\n- Sessions\n",
    });

    assert.notStrictEqual(preview, null);
    assert.strictEqual(preview?.filePath, "docs/runtime.md");
    assert.strictEqual(preview?.mode, "markdown");
    assert.strictEqual(preview?.language, "markdown");
    assert.strictEqual(preview?.lineCount, 4);
    assert.strictEqual(preview?.content, "# Runtime\n\n- Gateway\n- Sessions\n");
  });

  it("builds an image write preview from inline SVG content", () => {
    const preview = deriveWritePreview("write_file", {
      path: "assets/logo.svg",
      content: '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><circle cx="5" cy="5" r="4" /></svg>',
    });

    assert.notStrictEqual(preview, null);
    assert.strictEqual(preview?.mode, "image");
    assert.strictEqual(preview?.language, "svg");
    assert.strictEqual(preview?.languageLabel, "SVG");
    assert.ok(preview?.imageSrc?.startsWith("data:image/svg+xml;utf8,"));
  });

  it("builds an image write preview from image data URLs", () => {
    const preview = deriveWritePreview("write_file", {
      path: "assets/check.png",
      content: "data:image/png;base64,iVBORw0KGgo=",
    });

    assert.notStrictEqual(preview, null);
    assert.strictEqual(preview?.mode, "image");
    assert.strictEqual(preview?.language, "png");
    assert.strictEqual(preview?.languageLabel, "PNG");
    assert.strictEqual(preview?.imageSrc, "data:image/png;base64,iVBORw0KGgo=");
  });

  it("builds an edit preview from new_string when content is absent", () => {
    const preview = deriveWritePreview("edit", {
      file_path: "src/App.tsx",
      old_string: "const title = 'old';",
      new_string: "const title = 'new';",
    });

    assert.notStrictEqual(preview, null);
    assert.strictEqual(preview?.filePath, "src/App.tsx");
    assert.strictEqual(preview?.mode, "code");
    assert.strictEqual(preview?.language, "tsx");
    assert.strictEqual(preview?.content, "const title = 'new';");
  });

  it("returns null for read-only tools or empty write payloads", () => {
    assert.strictEqual(deriveWritePreview("read_file", { path: "README.md" }), null);
    assert.strictEqual(deriveWritePreview("write_file", { path: "README.md", content: "" }), null);
  });
});
