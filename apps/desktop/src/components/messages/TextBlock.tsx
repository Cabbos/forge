import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { BlockState } from "@/lib/protocol";
import { CodeBlock } from "@/components/messages/CodeBlock";
import { WhaleSVG } from "./WhaleSVG";

export function TextBlock({ block }: { block: BlockState }) {
  if (!block.content && block.isComplete) return null;
  const hasContent = Boolean(block.content);

  return (
    <div className="mb-4">
      {hasContent ? (
        <div className="px-4 py-3 text-sm leading-relaxed break-words rounded-2xl rounded-bl-md border text-left min-w-0"
          style={{ background: "#0f0f0f", borderColor: "#181818", color: "#ccc", overflowWrap: "anywhere" }}>
          <div className="markdown-content">
            <ReactMarkdown remarkPlugins={[remarkGfm]}
              components={{
                code({ className, children }) {
                  const match = /language-(\w+)/.exec(className || "");
                  if (!className) return <code style={{ color: "#D4A853" }}>{children}</code>;
                  return <CodeBlock code={String(children).replace(/\n$/, "")} lang={match?.[1] || ""} />;
                },
                pre({ children }) { return <>{children}</>; },
                a({ href, children }) { return <a href={href} target="_blank" style={{ color: "#D4A853" }}>{children}</a>; },
              }}>
              {block.content}
            </ReactMarkdown>
          </div>
        </div>
      ) : (
        /* No content — just the swimming whale, no shimmer bars */
        <div className="py-1">
          <WhaleSVG animate size={14} />
        </div>
      )}

      {/* Tiny green whale when done */}
      {hasContent && block.isComplete && (
        <div className="flex items-center gap-1 mt-1.5 select-none">
          <WhaleSVG done size={12} />
          <span className="text-[9px] font-mono" style={{ color: "#4A9E6B", opacity: 0.3 }}>ok</span>
        </div>
      )}
    </div>
  );
}
