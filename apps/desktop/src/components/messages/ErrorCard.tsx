import { AlertCircle } from "lucide-react";
import type { BlockState } from "@/lib/protocol";
import { MessagePanel, MessagePanelHeader } from "@/components/messages/MessagePanel";

export function ErrorCard({ block }: { block: BlockState }) {
  const title = errorTitle(block);
  const message = cleanErrorMessage(block.content, title);

  return (
    <MessagePanel tone="danger" className="error-note" role="status" ariaLive="polite">
      <MessagePanelHeader
        icon={<AlertCircle className="size-4 forge-error-card-icon" />}
        title={title}
        meta="这次请求没有完成"
      />
      <div data-testid="error-card-body" className="forge-error-card-body">
        {message}
      </div>
    </MessagePanel>
  );
}

function errorTitle(block: BlockState) {
  if (block.metadata?.code === "send_failed") return "发送失败";
  if (block.metadata?.code === "stop_failed") return "停止失败";
  return "发生错误";
}

function cleanErrorMessage(content: string, title: string) {
  const message = content.trim();
  if (!message) return "操作没有完成，请稍后再试。";
  return message.replace(new RegExp(`^${escapeRegExp(title)}[:：]\\s*`), "");
}

function escapeRegExp(value: string) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
