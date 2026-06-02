import { Check, Copy } from "lucide-react";
import { useState } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ShellOutputSections } from "@/components/messages/ShellOutputSections";

interface ShellOutputSection {
  label: string;
  content: string;
}

interface ShellCardDetailProps {
  command: string;
  output: string;
  outputSections: ShellOutputSection[];
  tone: string;
}

export function ShellCardDetail({ command, output, outputSections, tone }: ShellCardDetailProps) {
  const [copied, setCopied] = useState(false);

  const copyOutput = async () => {
    await navigator.clipboard?.writeText(output);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1200);
  };

  return (
    <div data-testid="log-detail-surface" className="forge-log-detail" data-tone={tone}>
      <div data-testid="log-detail-header" className="forge-log-detail-header">
        <span className="min-w-0 truncate font-mono text-[10px] text-muted-foreground/75">{command}</span>
        <ButtonPrimitive
          type="button"
          aria-label={copied ? "已复制命令输出" : "复制命令输出"}
          title={copied ? "已复制" : "复制命令输出"}
          onClick={copyOutput}
          disabled={!output}
          className="forge-log-action disabled:cursor-default disabled:opacity-45"
        >
          {copied ? <Check className="size-3" /> : <Copy className="size-3" />}
        </ButtonPrimitive>
      </div>
      <ShellOutputSections sections={outputSections} />
    </div>
  );
}
