import { useState } from "react";
import { ChevronRight, Brain, MessageSquare, Search, FileText } from "lucide-react";
import { cn } from "@/lib/utils";

interface ToolStep {
  name: string;
  input: string;
  result: string;
}

interface RoundStep {
  round: number;
  thinking: string;
  text: string;
  tool_calls: ToolStep[];
}

interface SubAgentPayload {
  result: string;
  steps: RoundStep[];
}

function ToolIcon({ name }: { name: string }) {
  switch (name) {
    case "search_content": case "search_files": return <Search className="size-3" />;
    case "read_file": return <FileText className="size-3" />;
    case "list_directory": return <FileText className="size-3" />;
    default: return <Search className="size-3" />;
  }
}

function ToolStepRow({ step }: { step: ToolStep }) {
  const [open, setOpen] = useState(false);
  const shortResult = step.result.slice(0, 200);
  return (
    <div className="ml-2">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 text-[10px] py-0.5 cursor-pointer transition-colors w-full text-left"
        style={{ color: "#888" }}
      >
        <ChevronRight className={cn("size-2.5 transition-transform", open && "rotate-90")} />
        <ToolIcon name={step.name} />
        <span className="font-mono" style={{ color: "#5B9BD5" }}>{step.name}</span>
        <span className="truncate" style={{ color: "#666" }}>{step.input.slice(0, 40)}</span>
        <span style={{ color: "#555" }}>→</span>
        <span className="truncate" style={{ color: "#999" }}>{shortResult}</span>
      </button>
      {open && (
        <div className="ml-5 mt-0.5 mb-1 p-1.5 rounded text-[10px] font-mono whitespace-pre-wrap break-all"
          style={{ background: "#060606", border: "1px solid #181818", color: "#777", maxHeight: "120px", overflow: "auto" }}>
          {step.result}
        </div>
      )}
    </div>
  );
}

function RoundCard({ step }: { step: RoundStep }) {
  const [open, setOpen] = useState(false);
  const hasDetail = step.thinking || step.tool_calls.length > 0 || step.text;

  return (
    <div className="mb-1">
      <button
        onClick={() => setOpen(!open)}
        className="flex items-center gap-1.5 text-[10px] py-1 cursor-pointer transition-colors w-full text-left"
        style={{ color: "#999" }}
      >
        <ChevronRight className={cn("size-2.5 transition-transform", open && "rotate-90")} />
        <span style={{ color: "#D4A853" }}>Round {step.round + 1}</span>
        {step.thinking && <Brain className="size-2.5" style={{ color: "#888" }} />}
        {step.text && <MessageSquare className="size-2.5" style={{ color: "#888" }} />}
        <span style={{ color: "#666" }}>{step.tool_calls.length} tools</span>
      </button>
      {open && hasDetail && (
        <div className="ml-3 pl-2" style={{ borderLeft: "1px solid #1c1c1c" }}>
          {step.thinking && (
            <div className="text-[10px] py-0.5" style={{ color: "#666" }}>
              <span className="text-muted-foreground/40">thinking: </span>
              {step.thinking.slice(0, 300)}
            </div>
          )}
          {step.text && (
            <div className="text-[10px] py-0.5" style={{ color: "#999" }}>
              {step.text.slice(0, 300)}
            </div>
          )}
          {step.tool_calls.map((tc, i) => (
            <ToolStepRow key={i} step={tc} />
          ))}
        </div>
      )}
    </div>
  );
}

export function SubAgentTrace({ content }: { content: string }) {
  let payload: SubAgentPayload | null = null;
  try {
    const parsed = JSON.parse(content);
    if (parsed && typeof parsed === "object" && Array.isArray(parsed.steps)) {
      payload = parsed as SubAgentPayload;
    }
  } catch { return null; }

  if (!payload) return null;

  return (
    <div className="mt-1" style={{ borderLeft: "1px solid rgba(212,168,83,0.2)" }}>
      {/* Steps */}
      {payload.steps.length > 0 && (
        <div className="ml-2 py-1">
          {payload.steps.map((step, i) => (
            <RoundCard key={i} step={step} />
          ))}
        </div>
      )}
      {/* Final result */}
      <div className="ml-2 p-2 rounded text-[11px] font-mono whitespace-pre-wrap break-all"
        style={{ background: "#060606", border: "1px solid #181818", color: "#999", maxHeight: "200px", overflow: "auto" }}>
        {payload.result || "(empty)"}
      </div>
    </div>
  );
}
