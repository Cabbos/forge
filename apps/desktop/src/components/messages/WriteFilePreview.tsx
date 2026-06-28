import { FileImage, FileText } from "lucide-react";
import { CodeBlock } from "@/components/messages/CodeBlock";
import { MarkdownRenderer } from "@/components/messages/MarkdownRenderer";
import type { WritePreviewView } from "@/components/messages/writePreviewPresentation";

interface WriteFilePreviewProps {
  preview: WritePreviewView | null;
}

export function WriteFilePreview({ preview }: WriteFilePreviewProps) {
  if (!preview) return null;

  const Icon = preview.mode === "image" ? FileImage : FileText;
  const metaLabel = preview.mode === "image" ? `${preview.languageLabel} · 图片` : `${preview.languageLabel} · ${preview.lineCount} 行`;

  return (
    <section data-testid="write-file-preview" className="forge-write-preview" aria-label="写入预览">
      <div className="forge-write-preview-header">
        <Icon className="size-3.5" />
        <span className="forge-write-preview-title">写入预览</span>
        <span className="forge-write-preview-path">{preview.filePath}</span>
        <span className="forge-write-preview-meta">{metaLabel}</span>
      </div>
      <div className="forge-write-preview-body" data-mode={preview.mode}>
        {preview.mode === "image" && preview.imageSrc ? (
          <figure className="forge-write-preview-image-frame">
            <img data-testid="write-file-image-preview" src={preview.imageSrc} alt={preview.filePath} />
          </figure>
        ) : preview.mode === "markdown" ? (
          <div className="markdown-content forge-write-preview-markdown">
            <MarkdownRenderer content={preview.content} onOpenFileRef={() => {}} />
          </div>
        ) : preview.mode === "code" ? (
          <CodeBlock code={preview.content} lang={preview.language} />
        ) : (
          <pre className="forge-write-preview-text">{preview.content}</pre>
        )}
      </div>
    </section>
  );
}
