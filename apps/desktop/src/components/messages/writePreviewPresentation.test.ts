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
