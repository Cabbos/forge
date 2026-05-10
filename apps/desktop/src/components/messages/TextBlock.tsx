import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { BlockState } from "@/lib/protocol";
import { CodeBlock } from "@/components/messages/CodeBlock";

export function TextBlock({ block }: { block: BlockState }) {
  if (!block.content && block.isComplete) return null;

  return (
    <div className="mb-6">
      <div className="bg-card rounded-2xl rounded-bl-lg px-6 py-4 max-w-[85%] shadow-sm">
        <div className="prose prose-sm dark:prose-invert max-w-none text-card-foreground/90 leading-relaxed">
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={{
              code({ className, children }) {
                const match = /language-(\w+)/.exec(className || "");
                const lang = match?.[1] || "";
                const codeStr = String(children).replace(/\n$/, "");
                if (!className) return <code className="bg-muted rounded-md px-1.5 py-0.5 text-xs font-mono">{children}</code>;
                return <CodeBlock code={codeStr} lang={lang} />;
              },
              pre({ children }) { return <>{children}</>; },
              a({ href, children }) { return <a href={href} target="_blank" rel="noopener noreferrer" className="text-primary underline underline-offset-2 decoration-primary/30 hover:decoration-primary transition-all">{children}</a>; },
              table({ children }) { return <div className="overflow-x-auto border border-border/50 rounded-xl my-3"><table className="min-w-full text-sm">{children}</table></div>; },
              th({ children }) { return <th className="bg-muted/50 px-3 py-2 text-left font-medium text-muted-foreground text-xs border-b border-border/50">{children}</th>; },
              td({ children }) { return <td className="px-3 py-1.5 text-sm border-b border-border/30">{children}</td>; },
            }}>
            {block.content || (block.isComplete ? "" : "...")}
          </ReactMarkdown>
        </div>
        {!block.isComplete && (
          <div className="flex items-center gap-1.5 mt-2.5 pt-2.5 border-t border-border/20">
            <span className="flex gap-1">
              <span className="w-1 h-3 rounded-full bg-primary/50 animate-pulse" />
              <span className="w-1 h-3 rounded-full bg-primary/50 animate-pulse [animation-delay:150ms]" />
              <span className="w-1 h-3 rounded-full bg-primary/50 animate-pulse [animation-delay:300ms]" />
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
