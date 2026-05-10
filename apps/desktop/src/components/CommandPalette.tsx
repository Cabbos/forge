import { useEffect } from "react";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command";
import { Bot, Terminal, FileCode, Sparkles, Sun, Moon, Settings } from "lucide-react";
import { useStore } from "@/store";
import { useSession } from "@/hooks/useSession";
import type { ToolType } from "@/lib/protocol";

interface CommandPaletteProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CommandPalette({ open, onOpenChange }: CommandPaletteProps) {
  const theme = useStore((s) => s.theme);
  const setTheme = useStore((s) => s.setTheme);
  const setActiveSession = useStore((s) => s.setActiveSession);
  const sessions = useStore((s) => s.sessions);
  const { create } = useSession();

  // Keyboard shortcut
  useEffect(() => {
    const down = (e: KeyboardEvent) => {
      if (e.key === "k" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        onOpenChange(true);
      }
    };
    document.addEventListener("keydown", down);
    return () => document.removeEventListener("keydown", down);
  }, [onOpenChange]);

  const handleCreate = async (toolType: ToolType) => {
    onOpenChange(false);
    try {
      await create(toolType, "/Users/cabbos");
    } catch (e) {
      console.error("Failed to create session:", e);
    }
  };

  const sessionList = Array.from(sessions.values());

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange}>
      <CommandInput placeholder="Type a command..." />
      <CommandList>
        <CommandEmpty>No results found.</CommandEmpty>

        <CommandGroup heading="New Session">
          <CommandItem onSelect={() => handleCreate("claude")}>
            <Bot className="size-4" />
            New Claude Session
            <CommandShortcut>⌘C</CommandShortcut>
          </CommandItem>
          <CommandItem onSelect={() => handleCreate("codex")}>
            <FileCode className="size-4" />
            New Codex Session
            <CommandShortcut>⌘D</CommandShortcut>
          </CommandItem>
          <CommandItem onSelect={() => handleCreate("hermes")}>
            <Sparkles className="size-4" />
            New Hermes Session
          </CommandItem>
          <CommandItem onSelect={() => handleCreate("bash")}>
            <Terminal className="size-4" />
            New Bash Session
          </CommandItem>
        </CommandGroup>

        {sessionList.length > 0 && (
          <>
            <CommandSeparator />
            <CommandGroup heading="Switch Session">
              {sessionList.map((s) => (
                <CommandItem
                  key={s.id}
                  onSelect={() => {
                    setActiveSession(s.id);
                    onOpenChange(false);
                  }}
                >
                  <span className="size-2 rounded-full flex-shrink-0" style={{
                    background: s.status === "running" ? "#22c55e" : s.status === "error" ? "#ef4444" : "#9ca3af",
                  }} />
                  <span className="capitalize">{s.agentType}</span>
                  <span className="text-muted-foreground text-xs">{s.id.slice(0, 8)}</span>
                </CommandItem>
              ))}
            </CommandGroup>
          </>
        )}

        <CommandSeparator />
        <CommandGroup heading="Preferences">
          <CommandItem
            onSelect={() => setTheme(theme === "dark" ? "light" : "dark")}
          >
            {theme === "dark" ? (
              <Sun className="size-4" />
            ) : (
              <Moon className="size-4" />
            )}
            Toggle Theme ({theme === "dark" ? "Light" : "Dark"})
            <CommandShortcut>⌘T</CommandShortcut>
          </CommandItem>
          <CommandItem onSelect={() => {
            // Open settings via the settings dialog — just toggle it quickly
            onOpenChange(false);
          }}>
            <Settings className="size-4" />
            API Key Settings
          </CommandItem>
        </CommandGroup>
      </CommandList>
    </CommandDialog>
  );
}
