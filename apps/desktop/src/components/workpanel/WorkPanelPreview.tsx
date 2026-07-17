import { useState } from "react";
import { ExternalLink, Monitor, RefreshCw, Smartphone, Tablet } from "lucide-react";
import { ForgeIconButton } from "@/components/primitives/icon-button";
import { normalizePreviewUrl } from "./workPanelSelectors";
import { WorkPanelFileDocument } from "./WorkPanelFiles";
import type { WorkPanelTab } from "./workPanelTypes";

type PreviewTab = Extract<WorkPanelTab, { kind: "preview" }>;
type PreviewWidth = "desktop" | "tablet" | "mobile";

export function WorkPanelPreview({ tab }: { tab: PreviewTab }) {
  if (tab.target.type === "file") {
    return <WorkPanelFileDocument path={tab.target.path} />;
  }

  return <WorkPanelWebPreview url={tab.target.url} label={tab.label} />;
}

function WorkPanelWebPreview({ url: inputUrl, label }: { url: string; label: string }) {
  const [reloadKey, setReloadKey] = useState(0);
  const [width, setWidth] = useState<PreviewWidth>("desktop");
  const url = normalizePreviewUrl(inputUrl);

  if (!url) {
    return (
      <div className="forge-work-panel-placeholder" role="alert">
        <strong>无法打开这个预览</strong>
        <span>工作面板只允许本机 HTTP 或 HTTPS 地址。</span>
      </div>
    );
  }

  return (
    <section className="forge-work-panel-web-preview" data-testid="work-panel-web-preview">
      <header className="forge-work-panel-content-toolbar">
        <span className="forge-work-panel-preview-url" title={url}>{url}</span>
        <div className="forge-work-panel-preview-actions" role="group" aria-label="预览控制">
          <PreviewWidthButton label="桌面宽度" active={width === "desktop"} onClick={() => setWidth("desktop")}>
            <Monitor className="size-3.5" />
          </PreviewWidthButton>
          <PreviewWidthButton label="平板宽度" active={width === "tablet"} onClick={() => setWidth("tablet")}>
            <Tablet className="size-3.5" />
          </PreviewWidthButton>
          <PreviewWidthButton label="手机宽度" active={width === "mobile"} onClick={() => setWidth("mobile")}>
            <Smartphone className="size-3.5" />
          </PreviewWidthButton>
          <ForgeIconButton aria-label="刷新预览" title="刷新预览" onClick={() => setReloadKey((value) => value + 1)}>
            <RefreshCw className="size-3.5" />
          </ForgeIconButton>
          <ForgeIconButton
            aria-label="在外部打开预览"
            title="在外部打开预览"
            onClick={() => window.open(url, "_blank", "noopener,noreferrer")}
          >
            <ExternalLink className="size-3.5" />
          </ForgeIconButton>
        </div>
      </header>
      <div className="forge-work-panel-preview-stage">
        <div className="forge-work-panel-preview-viewport" data-width={width}>
          <iframe
            key={reloadKey}
            src={url}
            title={label}
            referrerPolicy="no-referrer"
            sandbox="allow-forms allow-modals allow-popups allow-same-origin allow-scripts"
          />
        </div>
      </div>
    </section>
  );
}

function PreviewWidthButton({
  active,
  children,
  label,
  onClick,
}: {
  active: boolean;
  children: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <ForgeIconButton
      aria-label={label}
      aria-pressed={active}
      title={label}
      data-active={active ? "true" : "false"}
      onClick={onClick}
    >
      {children}
    </ForgeIconButton>
  );
}
