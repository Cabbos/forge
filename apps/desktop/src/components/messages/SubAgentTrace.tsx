import { useState } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
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
    <div className="forge-sub-agent-tool">
      <ButtonPrimitive
        type="button"
        data-testid="sub-agent-tool-trigger"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="forge-sub-agent-tool-trigger"
      >
        <ChevronRight className={cn("size-2.5 transition-transform", open && "rotate-90")} />
        <ToolIcon name={step.name} />
        <span className="forge-sub-agent-tool-name">{step.name}</span>
        <span className="forge-sub-agent-muted">{step.input.slice(0, 40)}</span>
        <span className="forge-sub-agent-muted">→</span>
        <span className="forge-sub-agent-preview">{shortResult}</span>
      </ButtonPrimitive>
      {open && (
        <div data-testid="sub-agent-tool-result" className="forge-sub-agent-output forge-sub-agent-output--tool">
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
    <div className="forge-sub-agent-round">
      <ButtonPrimitive
        type="button"
        data-testid="sub-agent-round-trigger"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="forge-sub-agent-round-trigger"
      >
        <ChevronRight className={cn("size-2.5 transition-transform", open && "rotate-90")} />
        <span className="forge-sub-agent-round-index">第 {step.round + 1} 轮</span>
        {step.thinking && <Brain className="size-2.5" />}
        {step.text && <MessageSquare className="size-2.5" />}
        <span className="forge-sub-agent-muted">{step.tool_calls.length} 个工具</span>
      </ButtonPrimitive>
      {open && hasDetail && (
        <div data-testid="sub-agent-round-detail" className="forge-sub-agent-round-detail">
          {step.thinking && (
            <div className="forge-sub-agent-prose">
              <span className="forge-sub-agent-muted">思考：</span>
              {step.thinking.slice(0, 300)}
            </div>
          )}
          {step.text && (
            <div className="forge-sub-agent-prose forge-sub-agent-prose--answer">
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
    <div data-testid="sub-agent-trace" className="forge-sub-agent-trace">
      {payload.steps.length > 0 && (
        <div className="forge-sub-agent-rounds">
          {payload.steps.map((step, i) => (
            <RoundCard key={i} step={step} />
          ))}
        </div>
      )}
      <div data-testid="sub-agent-result" className="forge-sub-agent-output forge-sub-agent-output--final">
        {payload.result || "暂无结果"}
      </div>
    </div>
  );
}
