import { useEffect, useRef, useState, type FormEvent } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { listen } from "@tauri-apps/api/event";
import { CornerDownLeft, RefreshCw, TerminalSquare } from "lucide-react";
import {
  closeWorkspaceTerminal,
  startWorkspaceTerminal,
  writeWorkspaceTerminal,
  type WorkspaceTerminalOutput,
} from "@/lib/tauri";
import { useActiveWorkspace, useStore } from "@/store";
import type { WorkPanelTab } from "./workPanelTypes";
import { createTerminalOutputSanitizer } from "./workPanelTerminalOutput";

type WorkPanelTerminalTab = Extract<WorkPanelTab, { kind: "terminal" }>;

export function WorkPanelTerminal({ tab }: { tab: WorkPanelTerminalTab }) {
  const terminalIdRef = useRef<string | null>(null);
  const [restartKey, setRestartKey] = useState(0);
  const [status, setStatus] = useState<"starting" | "running" | "exited" | "error">("starting");
  const [error, setError] = useState<string | null>(null);
  const [command, setCommand] = useState("");
  const [recentOutput, setRecentOutput] = useState("");
  const activeWorkspace = useActiveWorkspace();
  const boundSession = useStore((state) => state.sessions.get(tab.taskId) ?? null);
  const workingDir = boundSession?.workingDir ?? activeWorkspace?.path ?? null;
  const sessionId = boundSession ? tab.taskId : null;

  useEffect(() => {
    let disposed = false;
    let removeListener: (() => void) | null = null;
    const outputSanitizer = createTerminalOutputSanitizer();

    setStatus("starting");
    setError(null);
    setRecentOutput("");
    terminalIdRef.current = null;

    void (async () => {
      try {
        removeListener = await listen<WorkspaceTerminalOutput>("work-panel-terminal-output", (event) => {
          if (disposed) return;
          if (event.payload.task_id !== tab.taskId || event.payload.terminal_id !== terminalIdRef.current) return;
          if (event.payload.chunk) {
            setRecentOutput((current) => {
              const next = `${current}${outputSanitizer.push(event.payload.chunk)}`;
              return next.slice(-8_000);
            });
          }
          if (event.payload.exited) {
            setStatus("exited");
          }
        });
        if (disposed) {
          await removeListener();
          return;
        }
        const info = await startWorkspaceTerminal({
          taskId: tab.taskId,
          sessionId,
          workingDir,
          rows: 24,
          cols: 100,
        });
        if (disposed) {
          await closeWorkspaceTerminal(tab.taskId, info.terminal_id).catch(() => {});
          return;
        }
        terminalIdRef.current = info.terminal_id;
        setStatus("running");
      } catch (cause) {
        if (!disposed) {
          setStatus("error");
          setError(errorMessage(cause));
        }
      }
    })();

    return () => {
      disposed = true;
      outputSanitizer.reset();
      if (removeListener) void removeListener();
      const terminalId = terminalIdRef.current;
      terminalIdRef.current = null;
      if (terminalId) void closeWorkspaceTerminal(tab.taskId, terminalId).catch(() => {});
    };
  }, [restartKey, sessionId, tab.taskId, workingDir]);

  const submitCommand = (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const terminalId = terminalIdRef.current;
    if (!terminalId || !command.trim() || status !== "running") return;
    setCommand("");
    void writeWorkspaceTerminal(tab.taskId, terminalId, `${command}\r`).catch((cause) => {
      setError(errorMessage(cause));
    });
  };

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
      <div className="forge-work-panel-terminal-host">
        <pre aria-live="polite" role="log">
          {recentOutput || "临时环境已连接。输入一条验证命令即可；这里只保留最近输出。"}
        </pre>
        <form onSubmit={submitCommand}>
          <span aria-hidden="true">$</span>
          <input
            aria-label="临时验证命令"
            autoComplete="off"
            disabled={status !== "running"}
            onChange={(event) => setCommand(event.target.value)}
            placeholder={status === "running" ? "输入验证命令" : "正在准备临时环境"}
            spellCheck={false}
            value={command}
          />
          <ButtonPrimitive type="submit" disabled={status !== "running" || !command.trim()} aria-label="运行验证命令">
            <CornerDownLeft className="size-3.5" />
          </ButtonPrimitive>
        </form>
      </div>
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
