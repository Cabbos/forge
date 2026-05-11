import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { BlockState } from "@/lib/protocol";
import { CodeBlock } from "@/components/messages/CodeBlock";

export function TextBlock({ block }: { block: BlockState }) {
  if (!block.content && block.isComplete) return null;
  const isError = block.event_type === "error";

  return (
    <div className="flex gap-3 mb-4">
      <div className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0"
        style={{
          background: isError ? "rgba(200,80,80,0.12)" : "rgba(212,168,83,0.12)",
          color: isError ? "#D47777" : "#D4A853",
          fontSize: "0.6rem", fontWeight: 700,
        }}>AI</div>
      <div className="flex-1 min-w-0">
        <div className="text-[9px] uppercase tracking-wider text-muted-foreground/50 mb-1.5">Assistant</div>
        <div className="px-4 py-3 text-sm leading-relaxed break-words rounded-2xl rounded-bl-md border"
          style={{ background: "#0f0f0f", borderColor: "#181818", color: "#ccc" }}>
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
            {block.content || "..."}
          </ReactMarkdown>
        </div>
        {!block.isComplete && (
          <div className="h-px mt-2 overflow-hidden rounded-full" style={{ background: "#181818" }}>
            <div className="h-full w-1/3 rounded-full animate-[shimmer_1.5s_ease-in-out_infinite]"
              style={{ background: "linear-gradient(90deg, transparent, rgba(212,168,83,0.25), transparent)" }} />
          </div>
        )}
      </div>
    </div>
  );
}
