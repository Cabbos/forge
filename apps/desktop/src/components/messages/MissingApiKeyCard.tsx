import { KeyRound, Settings } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";

interface MissingApiKeyCardProps {
  block: BlockState;
}

export function MissingApiKeyCard(_props: MissingApiKeyCardProps) {
  return (
    <MessagePanel tone="warning" className="forge-missing-api-key-card" role="status" ariaLive="polite">
      <MessagePanelHeader
        icon={<KeyRound className="size-4 forge-missing-api-key-icon" />}
        title="需要配置模型密钥"
        meta="模型服务暂不可用"
        actions={(
          <button
            type="button"
            data-testid="missing-api-key-action"
            onClick={() => window.dispatchEvent(new Event("forge:open-settings"))}
            className="forge-missing-api-key-action"
          >
            <Settings className="size-3.5" />
            打开设置
          </button>
        )}
      />
      <div data-testid="missing-api-key-card" className="forge-missing-api-key-body">
        当前模型服务还没有可用密钥。添加密钥后，就可以继续创建对话。
      </div>
    </MessagePanel>
  );
}
