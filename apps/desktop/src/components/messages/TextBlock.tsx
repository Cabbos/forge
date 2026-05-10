import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { BlockState } from "@/lib/protocol";
import { CodeBlock } from "@/components/messages/CodeBlock";

export function TextBlock({ block }: { block: BlockState }) {
  if (!block.content && block.isComplete) return null;
  const isError = block.event_type === "error";

  return (
    <div className="flex gap-3">
      {/* Avatar */}
      <div className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0"
        style={{
          background: isError ? "rgba(200,80,80,0.12)" : "rgba(212,168,83,0.12)",
          color: isError ? "#D47777" : "#D4A853",
          fontSize: "0.65rem", fontWeight: 700,
        }}>
        A
      </div>
      <div className="flex-1 min-w-0">
        <div className="text-[10px] uppercase tracking-wider mb-1.5" style={{ color: "#555" }}>Assistant</div>
        <div className="text-sm leading-relaxed break-words" style={{ color: "#CCC" }}>
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={{
              code({ className, children }) {
                const match = /language-(\w+)/.exec(className || "");
                const lang = match?.[1] || "";
                const codeStr = String(children).replace(/\n$/, "");
                if (!className) return <code style={{ background: "var(--muted)", borderRadius: 4, padding: "0.12em 0.4em", fontSize: "0.85em", color: "#D4A853" }}>{children}</code>;
                return <CodeBlock code={codeStr} lang={lang} />;
              },
              pre({ children }) { return <>{children}</>; },
              a({ href, children }) { return <a href={href} target="_blank" rel="noopener noreferrer" style={{ color: "#D4A853", textDecoration: "underline" }}>{children}</a>; },
            }}>
            {block.content || (block.isComplete ? "" : "...")}
          </ReactMarkdown>
        </div>
        {/* Streaming shimmer bar */}
        {!block.isComplete && (
          <div className="h-px mt-2 overflow-hidden rounded-full" style={{ background: "#1c1c1c" }}>
            <div className="h-full w-1/3 rounded-full animate-[shimmer_1.5s_ease-in-out_infinite]"
              style={{ background: "linear-gradient(90deg, transparent, rgba(212,168,83,0.4), transparent)" }} />
          </div>
        )}
      </div>
    </div>
  );
}
