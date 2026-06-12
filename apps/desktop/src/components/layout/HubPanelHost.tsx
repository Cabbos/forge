import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { useStore } from "@/store";

type HubPanelSection = "agents" | "records";

const LazyHubPanel = lazy(() => import("./HubPanel").then((module) => ({ default: module.HubPanel })));

export function HubPanelHost() {
  const [loaded, setLoaded] = useState(false);
  const [open, setOpen] = useState(false);
  const [section, setSection] = useState<HubPanelSection | null>(null);
  const [dismissedA2ASignature, setDismissedA2ASignature] = useState<string | null>(null);
  const activeSessionId = useStore((s) => s.activeSessionId);
  const agentA2A = useStore((s) =>
    activeSessionId ? s.agentA2ABySession.get(activeSessionId) ?? null : null,
  );
  const a2aSignature = activeSessionId && agentA2A && agentA2A.tasks.length > 0
    ? `${activeSessionId}:${agentA2A.tasks.map((task) => task.task_id).join(",")}`
    : null;

  const openPanel = useCallback((targetSection?: HubPanelSection) => {
    setLoaded(true);
    setOpen(true);
    if (targetSection) setSection(targetSection);
  }, []);

  const setPanelOpen = useCallback((nextOpen: boolean) => {
    if (!nextOpen && a2aSignature) setDismissedA2ASignature(a2aSignature);
    setOpen(nextOpen);
  }, [a2aSignature]);

  useEffect(() => {
    const toggleHandler = () => {
      setLoaded(true);
      setPanelOpen(!open);
    };
    const openHandler = (event: Event) => {
      const detail = (event as CustomEvent<{ section?: HubPanelSection }>).detail;
      openPanel(detail?.section);
    };
    const shortcutHandler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "i") return;
      event.preventDefault();
      setLoaded(true);
      setPanelOpen(!open);
    };

    window.addEventListener("toggle-hub", toggleHandler);
    window.addEventListener("open-hub", openHandler);
    window.addEventListener("keydown", shortcutHandler);
    return () => {
      window.removeEventListener("toggle-hub", toggleHandler);
      window.removeEventListener("open-hub", openHandler);
      window.removeEventListener("keydown", shortcutHandler);
    };
  }, [open, openPanel, setPanelOpen]);

  useEffect(() => {
    if (!a2aSignature || open || dismissedA2ASignature === a2aSignature) return;
    setLoaded(true);
    setSection("agents");
    setOpen(true);
  }, [a2aSignature, dismissedA2ASignature, open]);

  useEffect(() => {
    document.documentElement.dataset.projectArchiveOpen = open ? "true" : "false";
    return () => {
      delete document.documentElement.dataset.projectArchiveOpen;
    };
  }, [open]);

  if (!loaded) return null;

  return (
    <Suspense fallback={null}>
      <LazyHubPanel
        open={open}
        initialSection={section}
        onOpenChange={setPanelOpen}
      />
    </Suspense>
  );
}
