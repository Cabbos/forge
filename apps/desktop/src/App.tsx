import { useEffect } from "react";
import { AppShell } from "./components/layout/AppShell";
import { useStore } from "./store";

export default function App() {
  const hydrate = useStore((s) => s.hydrate);
  const hydrated = useStore((s) => s.hydrated);

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  if (!hydrated) {
    return (
      <div className="flex h-screen items-center justify-center bg-background">
        <div className="text-muted-foreground text-sm animate-pulse">
          Loading...
        </div>
      </div>
    );
  }

  return <AppShell />;
}
