import * as React from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { Check, ChevronDown, Shield, ShieldAlert, ShieldCheck, type LucideIcon } from "lucide-react";
import { createPortal } from "react-dom";
import {
  confirmResponse,
  getPermissionMode,
  setPermissionMode,
  type PermissionMode,
  type PermissionModeState,
} from "@/lib/tauri";
import type { BlockState } from "@/lib/protocol";
import { parseWriteBoundary } from "@/lib/write-boundary";
import { useActiveWorkspace, useStore } from "@/store";
import { cn } from "@/lib/utils";

interface ComposerPermissionModeButtonProps {
  sessionId: string;
}

export function ComposerPermissionModeButton({ sessionId }: ComposerPermissionModeButtonProps) {
  const rootRef = React.useRef<HTMLDivElement>(null);
  const menuRef = React.useRef<HTMLDivElement>(null);
  const activeWorkspace = useActiveWorkspace();
  const session = useStore((state) => state.sessions.get(sessionId) ?? null);
  const updateBlock = useStore((state) => state.updateBlock);
  const workingDir = session?.workingDir ?? activeWorkspace?.path ?? null;
  const [modeState, setModeState] = React.useState<PermissionModeState>(manualPermissionMode);
  const [open, setOpen] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState("");
  const [menuStyle, setMenuStyle] = React.useState<React.CSSProperties>({});

  const loadPermissionMode = React.useCallback(async () => {
    if (!sessionId || !workingDir) {
      setModeState(manualPermissionMode);
      return;
    }
    try {
      setModeState(await getPermissionMode(sessionId, workingDir));
      setError("");
    } catch (event) {
      setError(formatPermissionError(event));
    }
  }, [sessionId, workingDir]);

  React.useEffect(() => {
    void loadPermissionMode();
  }, [loadPermissionMode]);

  React.useEffect(() => {
    if (!open) return;
    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target as Node;
      if (rootRef.current?.contains(target) || menuRef.current?.contains(target)) return;
      setOpen(false);
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("pointerdown", handlePointerDown);
    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("pointerdown", handlePointerDown);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [open]);

  React.useLayoutEffect(() => {
    if (!open) return;
    const updatePosition = () => {
      const button = rootRef.current?.querySelector<HTMLElement>("[data-testid='composer-permission-mode']");
      if (!button) return;
      const rect = button.getBoundingClientRect();
      const viewportGap = 12;
      const menuWidth = Math.min(260, window.innerWidth - viewportGap * 2);
      const menuHeight = 148;
      const floatingGap = 8;
      const fitsAbove = rect.top >= menuHeight + floatingGap + viewportGap;
      const top = fitsAbove
        ? rect.top - menuHeight - floatingGap
        : Math.min(rect.bottom + floatingGap, window.innerHeight - menuHeight - viewportGap);
      const left = Math.min(
        Math.max(viewportGap, rect.left),
        Math.max(viewportGap, window.innerWidth - menuWidth - viewportGap),
      );
      setMenuStyle({ top, left, width: menuWidth });
    };
    updatePosition();
    window.addEventListener("resize", updatePosition);
    window.addEventListener("scroll", updatePosition, true);
    return () => {
      window.removeEventListener("resize", updatePosition);
      window.removeEventListener("scroll", updatePosition, true);
    };
  }, [open]);

  const selectMode = React.useCallback(async (mode: PermissionMode) => {
    if (!sessionId) return;
    if (mode !== "manual_confirm" && !workingDir) {
      setError("需要先打开一个项目");
      return;
    }
    setBusy(true);
    setError("");
    try {
      const nextMode = await setPermissionMode({
        sessionId,
        mode,
        workspacePath: workingDir,
      });
      setModeState(nextMode);
      setOpen(false);

      if (mode !== "manual_confirm" && workingDir) {
        const pendingConfirm = findLatestPendingWorkspaceConfirm(
          session?.blocks ?? [],
          workingDir,
          mode === "full_access",
        );
        if (pendingConfirm) {
          await confirmResponse(pendingConfirm.block_id, true);
          updateBlock(sessionId, pendingConfirm.block_id, {
            metadata: { ...pendingConfirm.metadata, confirmed: true, answer: true },
          });
        }
      }
    } catch (event) {
      setError(formatPermissionError(event));
    } finally {
      setBusy(false);
    }
  }, [session?.blocks, sessionId, updateBlock, workingDir]);

  const view = permissionModeView(modeState.mode);
  const Icon = view.icon;
  const unavailable = !workingDir;

  return (
    <div ref={rootRef} className="forge-composer-permission-wrap">
      <ButtonPrimitive
        type="button"
        data-testid="composer-permission-mode"
        data-state={view.state}
        aria-label={`权限模式：${view.label}`}
        aria-haspopup="menu"
        aria-expanded={open}
        title={error || (unavailable ? "需要先打开一个项目" : view.title)}
        disabled={busy || unavailable}
        onClick={() => {
          void loadPermissionMode();
          setOpen((value) => !value);
        }}
        className="forge-composer-permission disabled:cursor-default disabled:opacity-60"
      >
        <Icon className={cn("size-3.5", busy && "animate-pulse")} />
        <span>{view.label}</span>
        <ChevronDown className="size-3" />
      </ButtonPrimitive>

      {open ? createPortal(
        <div
          ref={menuRef}
          data-testid="composer-permission-menu"
          role="menu"
          style={menuStyle}
          className="forge-composer-permission-menu forge-floating-menu"
        >
          {permissionModeOptions.map((option) => {
            const OptionIcon = option.icon;
            const checked = option.mode === modeState.mode;
            return (
              <ButtonPrimitive
                key={option.mode}
                type="button"
                role="menuitemradio"
                aria-checked={checked}
                data-testid={option.testId}
                data-active={checked ? "true" : "false"}
                onClick={() => void selectMode(option.mode)}
                className="forge-composer-permission-option"
              >
                <OptionIcon className="size-3.5" />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-xs font-medium">{option.label}</span>
                  <span className="block truncate text-[10px] text-muted-foreground">{option.description}</span>
                </span>
                {checked ? <Check className="size-3" /> : null}
              </ButtonPrimitive>
            );
          })}
        </div>,
        document.body,
      ) : null}
    </div>
  );
}

const manualPermissionMode: PermissionModeState = {
  mode: "manual_confirm",
  workspace_path: null,
  session_scoped: true,
};

const permissionModeOptions: Array<{
  mode: PermissionMode;
  label: string;
  description: string;
  testId: string;
  icon: LucideIcon;
}> = [
  {
    mode: "full_access",
    label: "完全访问",
    description: "跳过确认，保留硬阻断",
    testId: "composer-permission-full-access",
    icon: ShieldAlert,
  },
  {
    mode: "trust_current_project",
    label: "信任项目",
    description: "项目内非敏感写入直通",
    testId: "composer-permission-trust-current-project",
    icon: ShieldCheck,
  },
  {
    mode: "manual_confirm",
    label: "手动确认",
    description: "写入、命令和连接操作先询问",
    testId: "composer-permission-manual",
    icon: Shield,
  },
];

function permissionModeView(mode: PermissionMode): {
  label: string;
  title: string;
  state: string;
  icon: LucideIcon;
} {
  if (mode === "full_access") {
    return {
      label: "完全访问",
      title: "完全访问：跳过确认类操作，保留项目外写入和灾难命令硬阻断",
      state: "full_access",
      icon: ShieldAlert,
    };
  }
  if (mode === "trust_current_project") {
    return {
      label: "信任项目",
      title: "信任当前项目：项目内非敏感写入直接继续",
      state: "trusted",
      icon: ShieldCheck,
    };
  }
  return {
    label: "手动确认",
    title: "手动确认：写入、命令和连接操作先询问",
    state: "manual",
    icon: Shield,
  };
}

function findLatestPendingWorkspaceConfirm(
  blocks: BlockState[],
  workingDir: string,
  allowAnyOperation: boolean,
): BlockState | null {
  const normalizedWorkingDir = normalizeProjectPath(workingDir);
  for (let index = blocks.length - 1; index >= 0; index -= 1) {
    const block = blocks[index];
    if (block.event_type !== "confirm_ask") continue;
    if (block.metadata.confirmed === true || block.metadata.confirm_interrupted === true) continue;

    const boundary = parseWriteBoundary(block.metadata.boundary);
    if (!boundary) continue;
    if (normalizeProjectPath(boundary.workspacePath) !== normalizedWorkingDir) continue;
    if (!allowAnyOperation && !isWriteBoundaryOperation(boundary.operationLabel)) continue;
    if (!isAutoApprovableBoundary(block.metadata.boundary, workingDir, allowAnyOperation)) continue;
    return block;
  }
  return null;
}

function isWriteBoundaryOperation(operationLabel: string): boolean {
  return operationLabel === "写入文件" || operationLabel === "编辑文件" || operationLabel === "修改文件";
}

function normalizeProjectPath(path: string): string {
  const normalized = path.trim().replace(/\/+$/, "");
  if (!normalized || normalized === "/") return "";
  return normalized;
}

function isAutoApprovableBoundary(
  boundary: unknown,
  workingDir: string,
  allowSensitiveWorkspaceFiles: boolean,
): boolean {
  if (!boundary || typeof boundary !== "object" || Array.isArray(boundary)) return false;
  const rawFiles = (boundary as { affected_files?: unknown }).affected_files;
  if (!Array.isArray(rawFiles)) return true;
  const normalizedWorkingDir = normalizeProjectPath(workingDir);
  return rawFiles.every((file) => {
    if (typeof file !== "string") return false;
    const normalizedFile = normalizeProjectPath(file);
    const projectRelativeFile = normalizedFile.startsWith(`${normalizedWorkingDir}/`)
      ? normalizedFile.slice(normalizedWorkingDir.length + 1)
      : normalizedFile;
    if (normalizedFile.startsWith("~")) return false;
    if (normalizedFile.startsWith("/") && normalizedFile !== normalizedWorkingDir && !normalizedFile.startsWith(`${normalizedWorkingDir}/`)) return false;
    if (projectRelativeFile === ".." || projectRelativeFile.startsWith("../") || projectRelativeFile.includes("/../")) return false;
    if (!allowSensitiveWorkspaceFiles && isSensitiveProjectPath(projectRelativeFile)) return false;
    return true;
  });
}

function isSensitiveProjectPath(path: string): boolean {
  const normalized = path.replace(/\\/g, "/").toLowerCase();
  return normalized === ".env" || normalized.startsWith(".env.") || normalized.endsWith("/.env") || normalized.includes("/.env.");
}

function formatPermissionError(event: unknown): string {
  return event instanceof Error ? event.message : String(event);
}
