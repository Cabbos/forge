import { KeyRound, Settings } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { MessagePanel } from "@/components/messages/MessagePanel";

interface MissingApiKeyCardProps {
  block: BlockState;
}

export function MissingApiKeyCard(_props: MissingApiKeyCardProps) {
  return (
    <MessagePanel tone="warning" className="max-w-[620px] px-3 py-3">
      <div className="flex items-start gap-3">
        <div
          className="mt-0.5 flex size-7 shrink-0 items-center justify-center rounded-md"
          style={{ background: "rgba(212,168,83,0.14)", color: "#D4A853" }}
        >
          <KeyRound className="size-4" />
        </div>
        <div className="min-w-0">
          <div className="text-sm font-medium text-foreground">需要配置模型密钥</div>
          <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
            当前模型服务还没有可用密钥。添加密钥后，就可以继续创建对话。
          </p>
          <button
            type="button"
            onClick={() => window.dispatchEvent(new Event("forge:open-settings"))}
            className="mt-3 inline-flex h-7 items-center gap-1.5 rounded-md px-2.5 text-[11px] font-medium transition-colors"
            style={{ background: "#D4A853", color: "#111216" }}
          >
            <Settings className="size-3.5" />
            打开设置
          </button>
        </div>
      </div>
    </MessagePanel>
  );
}
