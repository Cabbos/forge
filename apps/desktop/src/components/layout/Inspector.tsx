import type { ReactNode } from "react";
import { useStore } from "@/store";

interface InspectorProps {
  sessionId: string | null;
}

export function Inspector({ sessionId }: InspectorProps) {
  const session = useStore((s) => sessionId ? s.sessions.get(sessionId) ?? null : null);
  const blocks = session?.blocks ?? [];
  const eventCount = blocks.length;
  const toolCount = new Set(
    blocks.filter(b => b.event_type === "tool_call").map(b => b.metadata.tool_name as string)
  ).size;

  return (
    <aside
      data-testid="project-cockpit"
      className="forge-project-cockpit"
      role="complementary"
      aria-label="Inspector"
    >
      <div className="forge-inspector-content">
        <div className="forge-inspector-title">Inspector</div>

        <div className="forge-inspector-stats">
          <StatBox value={eventCount} label="events" />
          <StatBox value={toolCount} label="tools" />
        </div>

        <InfoCard title="AI Adapter" tags={["stream_message()", "async_trait"]}>
          Provider streams are folded into shared protocol events before the store sees them.
        </InfoCard>

        <InfoCard title="StreamEvent Contract" tags={[]}>
          <div className="forge-inspector-code">
            <div>*_start → *_chunk → *_end</div>
            <div className="forge-inspector-muted">Rust: protocol/events.rs</div>
            <div className="forge-inspector-muted">TS: src/lib/protocol.ts</div>
            <div className="forge-inspector-muted">Store: dispatchOutputEvent</div>
          </div>
        </InfoCard>

        <InfoCard title="Plugin Surface" tags={["MCP", "Hook", "Skill"]}>
          <span className="forge-inspector-muted">
            Install, scan, and target plugins per agent.
          </span>
        </InfoCard>
      </div>
    </aside>
  );
}

function StatBox({ value, label }: { value: number; label: string }) {
  return (
    <div className="forge-inspector-stat">
      <div className="forge-inspector-stat-value">
        {String(value).padStart(2, "0")}
      </div>
      <div className="forge-inspector-stat-label">
        {label}
      </div>
    </div>
  );
}

function InfoCard({ title, tags, children }: { title: string; tags: string[]; children: ReactNode }) {
  return (
    <div className="forge-inspector-card">
      <div className="forge-inspector-card-title">
        {title}
      </div>
      {tags.length > 0 && (
        <div className="forge-inspector-tags">
          {tags.map(tag => (
            <span key={tag} className="forge-inspector-tag">
              {tag}
            </span>
          ))}
        </div>
      )}
      <div className="forge-inspector-card-body">
        {children}
      </div>
    </div>
  );
}
