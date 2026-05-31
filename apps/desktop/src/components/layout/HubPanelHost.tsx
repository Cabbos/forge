import { lazy, Suspense, useCallback, useEffect, useState } from "react";

type HubPanelSection = "records";

const LazyHubPanel = lazy(() => import("./HubPanel").then((module) => ({ default: module.HubPanel })));

export function HubPanelHost() {
  const [loaded, setLoaded] = useState(false);
  const [open, setOpen] = useState(false);
  const [section, setSection] = useState<HubPanelSection | null>(null);

  const openPanel = useCallback((targetSection?: HubPanelSection) => {
    setLoaded(true);
    setOpen(true);
    if (targetSection) setSection(targetSection);
  }, []);

  useEffect(() => {
    const toggleHandler = () => {
      setLoaded(true);
      setOpen((value) => !value);
    };
    const openHandler = (event: Event) => {
      const detail = (event as CustomEvent<{ section?: HubPanelSection }>).detail;
      openPanel(detail?.section);
    };
    const shortcutHandler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.key.toLowerCase() !== "i") return;
      event.preventDefault();
      setLoaded(true);
      setOpen((value) => !value);
    };

    window.addEventListener("toggle-hub", toggleHandler);
    window.addEventListener("open-hub", openHandler);
    window.addEventListener("keydown", shortcutHandler);
    return () => {
      window.removeEventListener("toggle-hub", toggleHandler);
      window.removeEventListener("open-hub", openHandler);
      window.removeEventListener("keydown", shortcutHandler);
    };
  }, [openPanel]);

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
        onOpenChange={setOpen}
      />
    </Suspense>
  );
}
