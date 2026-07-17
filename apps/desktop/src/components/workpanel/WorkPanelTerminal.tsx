import { useEffect, useRef, useState } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { listen } from "@tauri-apps/api/event";
import { FitAddon } from "@xterm/addon-fit";
import { Terminal } from "@xterm/xterm";
import "@xterm/xterm/css/xterm.css";
import { RefreshCw, TerminalSquare } from "lucide-react";
import {
  closeWorkspaceTerminal,
  resizeWorkspaceTerminal,
  startWorkspaceTerminal,
  writeWorkspaceTerminal,
  type WorkspaceTerminalOutput,
} from "@/lib/tauri";
import { useActiveWorkspace, useStore } from "@/store";
import type { WorkPanelTab } from "./workPanelTypes";

type WorkPanelTerminalTab = Extract<WorkPanelTab, { kind: "terminal" }>;

export function WorkPanelTerminal({ tab }: { tab: WorkPanelTerminalTab }) {
  const hostRef = useRef<HTMLDivElement>(null);
  const [restartKey, setRestartKey] = useState(0);
  const [status, setStatus] = useState<"starting" | "running" | "exited" | "error">("starting");
  const [error, setError] = useState<string | null>(null);
  const activeWorkspace = useActiveWorkspace();
  const boundSession = useStore((state) => state.sessions.get(tab.taskId) ?? null);
  const workingDir = boundSession?.workingDir ?? activeWorkspace?.path ?? null;
  const sessionId = boundSession ? tab.taskId : null;

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let disposed = false;
    let terminalId: string | null = null;
    let removeListener: (() => void) | null = null;
    let resizeTimer: ReturnType<typeof setTimeout> | null = null;

    setStatus("starting");
    setError(null);
    const terminal = new Terminal({
      cursorBlink: true,
      convertEol: false,
      fontFamily: "var(--font-mono)",
      fontSize: 12,
      lineHeight: 1.35,
      scrollback: 2_000,
      theme: {
        background: "#171817",
        foreground: "#e9e8e5",
        cursor: "#f5f3ed",
        selectionBackground: "#4d514d",
      },
    });
    const fitAddon = new FitAddon();
    terminal.loadAddon(fitAddon);
    terminal.open(host);
    fitAddon.fit();

    const inputDisposable = terminal.onData((data) => {
      if (!terminalId) return;
      void writeWorkspaceTerminal(tab.taskId, terminalId, data).catch((cause) => {
        if (!disposed) setError(errorMessage(cause));
      });
    });
    const resizeObserver = new ResizeObserver(() => {
      if (resizeTimer) clearTimeout(resizeTimer);
      resizeTimer = setTimeout(() => {
        if (disposed) return;
        fitAddon.fit();
        if (terminalId) {
          void resizeWorkspaceTerminal(tab.taskId, terminalId, terminal.rows, terminal.cols).catch(() => {});
        }
      }, 40);
    });
    resizeObserver.observe(host);

    void (async () => {
      try {
        removeListener = await listen<WorkspaceTerminalOutput>("work-panel-terminal-output", (event) => {
          if (event.payload.task_id !== tab.taskId || event.payload.terminal_id !== terminalId) return;
          if (event.payload.chunk) terminal.write(event.payload.chunk);
          if (event.payload.exited) {
            setStatus("exited");
            terminal.writeln("\r\n\x1b[2m[临时终端已结束]\x1b[0m");
          }
        });
        const info = await startWorkspaceTerminal({
          taskId: tab.taskId,
          sessionId,
          workingDir,
          rows: terminal.rows,
          cols: terminal.cols,
        });
        if (disposed) {
          await closeWorkspaceTerminal(tab.taskId, info.terminal_id).catch(() => {});
          return;
        }
        terminalId = info.terminal_id;
        setStatus("running");
        terminal.focus();
      } catch (cause) {
        if (!disposed) {
          setStatus("error");
          setError(errorMessage(cause));
        }
      }
    })();

    return () => {
      disposed = true;
      if (resizeTimer) clearTimeout(resizeTimer);
      resizeObserver.disconnect();
      inputDisposable.dispose();
      removeListener?.();
      terminal.dispose();
      if (terminalId) void closeWorkspaceTerminal(tab.taskId, terminalId).catch(() => {});
    };
  }, [restartKey, sessionId, tab.taskId, workingDir]);

  return (
    <section className="forge-work-panel-terminal" data-testid="work-panel-terminal">
      <div className="forge-work-panel-content-toolbar">
        <div className="forge-work-panel-content-title">
          <TerminalSquare className="size-4" />
          <span>临时验证终端</span>
          <small>{terminalStatusLabel(status)}</small>
        </div>
        <ButtonPrimitive type="button" onClick={() => setRestartKey((value) => value + 1)}>
          <RefreshCw className="size-3.5" />
          重启
        </ButtonPrimitive>
      </div>
      {error ? <p className="forge-work-panel-inline-error" role="alert">{error}</p> : null}
      <div ref={hostRef} className="forge-work-panel-terminal-host" aria-label="临时验证终端输入" />
    </section>
  );
}

function terminalStatusLabel(status: "starting" | "running" | "exited" | "error") {
  switch (status) {
    case "starting": return "正在启动";
    case "running": return "当前任务";
    case "exited": return "已结束";
    case "error": return "不可用";
  }
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
