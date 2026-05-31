import { Activity, CheckCircle2, ClipboardList, FileText, FolderOpen, MessagesSquare, ShieldCheck, TerminalSquare, type LucideIcon } from "lucide-react";
import { ProjectStatusCard } from "./ProjectStatusCard";
import { useActiveWorkspace, useStore } from "@/store";
import { getProjectDisplay, getSessionStatus, getSessionTitle } from "@/lib/session-display";

interface ProjectCockpitProps {
  sessionId: string | null;
}

export function ProjectCockpit({ sessionId }: ProjectCockpitProps) {
  const activeWorkspace = useActiveWorkspace();
  const session = useStore((s) => sessionId ? s.sessions.get(sessionId) ?? null : null);
  const workflow = useStore((s) => sessionId ? s.workflowBySession.get(sessionId) ?? null : null);
  const deliverySummary = useStore((s) => sessionId ? s.deliverySummaryBySession.get(sessionId) ?? null : null);
  const selectedContext = useStore((s) => sessionId ? s.selectedContextBySession.get(sessionId) ?? [] : []);
  const wikiContext = useStore((s) => sessionId ? s.forgeWikiContextBySession.get(sessionId) ?? [] : []);
  const mcpContext = useStore((s) => sessionId ? s.mcpContextBySession.get(sessionId) ?? [] : []);
  const project = getProjectDisplay(session?.workingDir || activeWorkspace?.path);
  const status = getSessionStatus(session);
  const blocks = session?.blocks ?? [];
  const counts = summarizeBlocks(blocks);
  const contextCount = selectedContext.length + wikiContext.length + mcpContext.length;
  const activeTitle = session ? getSessionTitle(session) : "等待开始";
  const workflowLabel = workflow?.beginner_label ?? (session?.streaming ? "正在响应" : session ? "可以继续描述任务" : "选择任务后开始");
  const deliveryText = deliverySummary?.next_action ?? "交付状态会在预览、验证和检查点更新后收拢。";

  return (
    <aside
      data-testid="project-cockpit"
      className="forge-project-cockpit"
      role="complementary"
      aria-label="项目驾驶舱"
    >
      <div className="forge-cockpit-header">
        <div className="forge-cockpit-kicker">本地工作台</div>
        <h2 className="forge-cockpit-title">项目驾驶舱</h2>
        <div className="forge-cockpit-project" title={project.path}>
          <FolderOpen className="size-3.5" />
          <span>{project.name}</span>
        </div>
      </div>

      <div className="forge-cockpit-body">
        <section className="forge-cockpit-panel forge-cockpit-session-panel" aria-label="当前对话">
          <div className="forge-cockpit-panel-heading">
            <MessagesSquare className="size-3.5" />
            <span>当前对话</span>
          </div>
          <div className="forge-cockpit-session-title" title={activeTitle}>{activeTitle}</div>
          <div className="forge-cockpit-status-row">
            <span className="forge-cockpit-status-dot" aria-hidden="true" />
            <span>{status.label}</span>
          </div>
          <p className="forge-cockpit-muted">{workflowLabel}</p>
        </section>

        <ProjectStatusCard sessionId={sessionId} />

        <section className="forge-cockpit-panel" aria-label="操作态势">
          <div className="forge-cockpit-panel-heading">
            <Activity className="size-3.5" />
            <span>操作态势</span>
          </div>
          <div className="forge-cockpit-metrics">
            <CockpitMetric icon={MessagesSquare} label="消息" value={counts.messages} />
            <CockpitMetric icon={TerminalSquare} label="证据" value={counts.evidence} />
            <CockpitMetric icon={ShieldCheck} label="权限" value={counts.permissions} />
          </div>
        </section>

        <section className="forge-cockpit-panel" aria-label="上下文">
          <div className="forge-cockpit-panel-heading">
            <FileText className="size-3.5" />
            <span>上下文</span>
          </div>
          <div className="forge-cockpit-context-row">
            <span>{contextCount > 0 ? `${contextCount} 条材料已挂载` : "暂无挂载材料"}</span>
            <CheckCircle2 className="size-3.5" />
          </div>
          <p className="forge-cockpit-muted">
            资料、项目记录和连接器材料会留在当前项目边界内。
          </p>
        </section>

        <section className="forge-cockpit-panel" aria-label="交付">
          <div className="forge-cockpit-panel-heading">
            <ClipboardList className="size-3.5" />
            <span>交付</span>
          </div>
          <p className="forge-cockpit-delivery">{deliveryText}</p>
        </section>
      </div>
    </aside>
  );
}

function CockpitMetric({
  icon: Icon,
  label,
  value,
}: {
  icon: LucideIcon;
  label: string;
  value: number;
}) {
  return (
    <div className="forge-cockpit-metric">
      <Icon className="size-3.5" />
      <div>
        <div className="forge-cockpit-metric-value">{value}</div>
        <div className="forge-cockpit-metric-label">{label}</div>
      </div>
    </div>
  );
}

function summarizeBlocks(blocks: Array<{ event_type: string }>) {
  let messages = 0;
  let evidence = 0;
  let permissions = 0;

  for (const block of blocks) {
    if (block.event_type === "user_message" || block.event_type === "text") messages += 1;
    if (
      block.event_type === "tool_call" ||
      block.event_type === "shell" ||
      block.event_type === "diff_view"
    ) {
      evidence += 1;
    }
    if (block.event_type === "confirm_ask") permissions += 1;
  }

  return { messages, evidence, permissions };
}
