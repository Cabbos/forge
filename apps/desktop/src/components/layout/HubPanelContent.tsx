import type { ComponentProps } from "react";
import { ForgeScrollArea } from "@/components/primitives/scroll-area";
import { ActiveContextSection } from "@/components/context/ActiveContextSection";
import { FirstLoopCard } from "@/components/context/FirstLoopCard";
import { ProjectOverviewCard } from "@/components/context/ProjectOverviewCard";
import { WikiSections } from "@/components/context/WikiSections";
import { AgentA2AWorkspace } from "@/components/messages/AgentA2ATimeline";
import { CurrentTaskCard } from "@/components/workflow/CurrentTaskCard";
import type { McpContextSelection } from "@/lib/tauri";
import { ProjectStatusCard } from "./ProjectStatusCard";
import { ArchiveDisclosure } from "./archive/ArchiveDisclosure";
import { ArchiveLayerHeader } from "./archive/ArchiveLayerHeader";
import { ArchiveSummaryStrip } from "./archive/ArchiveSummaryStrip";
import { ContextFilesSection } from "./archive/ArchiveContextMaterials";

type ArchiveSummaryProps = ComponentProps<typeof ArchiveSummaryStrip>;

export interface HubPanelContentProps {
  activeContextItems: ComponentProps<typeof ActiveContextSection>["items"];
  activeId: string | null;
  agentA2A: ComponentProps<typeof AgentA2AWorkspace>["state"];
  contextMaterials: ComponentProps<typeof ContextFilesSection>["files"];
  deliverySummary: ArchiveSummaryProps["deliverySummary"];
  firstLoopDraft: ComponentProps<typeof FirstLoopCard>["draft"];
  overview: ArchiveSummaryProps["overview"];
  projectPath: string | null;
  recordsRequestedOpen: boolean;
  workflow: ComponentProps<typeof CurrentTaskCard>["workflow"];
  onToggleContext: (selection: McpContextSelection) => void;
}

export function HubPanelContent({
  activeContextItems,
  activeId,
  agentA2A,
  contextMaterials,
  deliverySummary,
  firstLoopDraft,
  overview,
  projectPath,
  recordsRequestedOpen,
  workflow,
  onToggleContext,
}: HubPanelContentProps) {
  return (
    <ForgeScrollArea className="min-h-0 flex-1">
      <div data-testid="project-archive-body" className="forge-inspector-body">
        <div data-forge-motion="archive-section">
          <ArchiveSummaryStrip
            contextCount={activeContextItems.length}
            deliverySummary={deliverySummary}
            overview={overview}
          />
        </div>

        <div data-forge-motion="archive-section">
          <ProjectOverviewCard overview={overview} />
        </div>

        {agentA2A && agentA2A.tasks.length > 0 && (
          <div data-forge-motion="archive-section">
            <AgentA2AWorkspace state={agentA2A} />
          </div>
        )}

        <div data-forge-motion="archive-section">
          <CurrentTaskCard workflow={workflow} />
        </div>

        {firstLoopDraft && (
          <div data-forge-motion="archive-section">
            <FirstLoopCard draft={firstLoopDraft} />
          </div>
        )}

        {activeContextItems.length > 0 && (
          <div data-forge-motion="archive-section">
            <ActiveContextSection items={activeContextItems} />
          </div>
        )}

        <div data-forge-motion="archive-section">
          <ArchiveDisclosure
            testId="archive-disclosure-records"
            title="项目记录"
            meta="记录与建议"
            defaultOpen={recordsRequestedOpen || deliverySummary?.record_status === "pending"}
          >
            <WikiSections sessionId={activeId} projectPath={projectPath} />
          </ArchiveDisclosure>
        </div>

        <div data-forge-motion="archive-section">
          <ArchiveDisclosure
            testId="archive-disclosure-files"
            title="资料"
            meta={contextMaterials.length > 0 ? `${contextMaterials.length} 个资料` : "未添加"}
          >
            <ContextFilesSection
              files={contextMaterials}
              onToggle={onToggleContext}
            />
          </ArchiveDisclosure>
        </div>

        <div data-forge-motion="archive-section">
          <ArchiveLayerHeader title="交付" meta="最近状态" />
        </div>

        <div data-forge-motion="archive-section">
          {activeId ? (
            <ProjectStatusCard sessionId={activeId} />
          ) : (
            <div className="forge-empty">
              选择一个任务后查看预览和检查点。
            </div>
          )}
        </div>
      </div>
    </ForgeScrollArea>
  );
}
