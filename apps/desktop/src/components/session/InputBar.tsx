import { useRef, useState, useCallback, useEffect, type CSSProperties } from "react";
import { AlertCircle, ArrowUp, ChevronDown, X, RotateCcw } from "lucide-react";
import { useSession } from "@/hooks/useSession";
import { useStore } from "@/store";
import { createProjectCheckpoint, searchWorkspaceFiles } from "@/lib/tauri";
import { formatContextWindow, getModelContextWindow, getModelLabel, getProviderDefinition, PROVIDERS } from "@/lib/providers";
import { modeAwarePlaceholder } from "@/lib/task-mode";
import { buildFirstLoopAgentPrompt, deriveFirstLoopDraft } from "@/lib/first-loop";
import { commandIconMeta, composerToolbarIcons, fileReferenceIconMeta } from "@/lib/capability-icons";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import { cn } from "@/lib/utils";

interface InputBarProps { sessionId: string }

interface Chip { id: string; type: "file" | "command"; value: string; }

const COMMANDS = [
  { prefix: "/cr", text: "/code-review", desc: "检查有没有风险" },
  { prefix: "/fix", text: "/fix", desc: "帮我修一个问题" },
  { prefix: "/explain", text: "/explain", desc: "解释清楚" },
  { prefix: "/refactor", text: "/refactor", desc: "整理代码结构" },
  { prefix: "/test", text: "/test", desc: "运行相关检查" },
  { prefix: "/docs", text: "/docs", desc: "补充说明文档" },
];

const COMPOSER_MAX_INPUT_HEIGHT = 140;
const ACTIVE_MENU_OPTION_STYLE: CSSProperties = {
  backgroundColor: "rgba(255, 255, 255, 0.052)",
  borderColor: "var(--forge-border-subtle)",
  color: "var(--forge-text-primary)",
};

export function InputBar({ sessionId }: InputBarProps) {
  const composerRootRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const composingRef = useRef(false);
  const [value, setValue] = useState("");
  const [chips, setChips] = useState<Chip[]>([]);
  const [showModelMenu, setShowModelMenu] = useState(false);
  const selectedProvider = useStore((s) => s.selectedProvider);
  const setSelectedProvider = useStore((s) => s.setSelectedProvider);
  const selectedModel = useStore((s) => s.selectedModel);
  const setSelectedModel = useStore((s) => s.setSelectedModel);
  const pendingInput = useStore((s) => s.pendingInput);
  const setPendingInput = useStore((s) => s.setPendingInput);
  const setFirstLoopDraft = useStore((s) => s.setFirstLoopDraft);
  const selectedMcpContext = useStore((s) => s.mcpContextBySession.get(sessionId) ?? []);
  const workflow = useStore((s) => s.workflowBySession.get(sessionId) ?? null);
  const [showSuggestions, setShowSuggestions] = useState<"@" | "/" | null>(null);
  const [activeSuggestionIndex, setActiveSuggestionIndex] = useState(0);
  const [atResults, setAtResults] = useState<string[]>([]);
  const valueRef = useRef("");
  const suggestionListId = `${sessionId}-composer-suggestions`;
  const modelMenuId = `${sessionId}-model-menu`;

  const { send, stop, resume } = useSession();
  const session = useStore((s) => s.sessions.get(sessionId));
  const isRunning = session?.status === "running";
  const isStreaming = session?.streaming ?? false;
  const [isResuming, setIsResuming] = useState(false);
  const [resumeError, setResumeError] = useState("");
  const selectedContextWindow = formatContextWindow(getModelContextWindow(selectedModel));
  const selectedProviderLabel = getProviderDefinition(selectedProvider).label;
  const selectedModelLabel = getModelLabel(selectedModel);
  const canSend = isRunning && (value.trim().length > 0 || chips.length > 0);

  useEffect(() => { if (isRunning) textareaRef.current?.focus(); }, [sessionId, isRunning]);
  useEffect(() => {
    setActiveSuggestionIndex(0);
  }, [showSuggestions, atResults.length]);
  useEffect(() => {
    if (!showSuggestions && !showModelMenu) return;

    const handlePointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && composerRootRef.current?.contains(target)) return;
      setShowSuggestions(null);
      setShowModelMenu(false);
    };

    document.addEventListener("pointerdown", handlePointerDown);
    return () => document.removeEventListener("pointerdown", handlePointerDown);
  }, [showModelMenu, showSuggestions]);

  const adjustHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    const nextHeight = Math.min(el.scrollHeight, COMPOSER_MAX_INPUT_HEIGHT);
    el.style.height = `${nextHeight}px`;
    el.style.overflowY = el.scrollHeight > COMPOSER_MAX_INPUT_HEIGHT ? "auto" : "hidden";
  }, []);

  useEffect(() => {
    if (!pendingInput) return;

    setValue((current) => {
      const next = current.trim()
        ? `${current.trimEnd()}\n\n${pendingInput}`
        : pendingInput;
      valueRef.current = next;
      return next;
    });
    setPendingInput("");
    setTimeout(() => {
      textareaRef.current?.focus();
      adjustHeight();
    }, 0);
  }, [pendingInput, setPendingInput, adjustHeight]);

  const addChip = useCallback((type: "file" | "command", val: string) => {
    setChips(prev => prev.some((chip) => chip.value === val)
      ? prev
      : [...prev, { id: crypto.randomUUID(), type, value: val }]);
    setShowSuggestions(null);
    // Remove the @ or / trigger text from the textarea
    setValue(prev => {
      const pos = textareaRef.current?.selectionStart ?? prev.length;
      const before = prev.slice(0, pos);
      const after = prev.slice(pos);
      const lastAt = before.lastIndexOf(type === "file" ? "@" : "/");
      const next = lastAt >= 0 ? before.slice(0, lastAt) + after : prev;
      valueRef.current = next;
      return next;
    });
    setTimeout(() => textareaRef.current?.focus(), 0);
  }, []);

  const removeChip = useCallback((id: string) => {
    setChips(prev => prev.filter(c => c.id !== id));
  }, []);

  const handleChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const v = e.target.value;
    valueRef.current = v;
    setValue(v);
    adjustHeight();
    const pos = e.target.selectionStart;
    const before = v.slice(0, pos);
    const lastWord = before.split(/\s/).pop() || "";
    if (lastWord.startsWith("@") && lastWord.length >= 1) {
      setShowModelMenu(false);
      setShowSuggestions("@");
      const q = lastWord.slice(1);
      searchWorkspaceFiles(q, sessionId).then(setAtResults).catch(() => setAtResults([]));
    } else if (lastWord === "/") {
      setShowModelMenu(false);
      setShowSuggestions("/");
    } else if (showSuggestions) {
      setShowSuggestions(null);
    }
  }, [adjustHeight, showSuggestions]);

  const handleSend = useCallback(async () => {
    const text = value.trim();
    if (!text && chips.length === 0) return;
    if (!isRunning) return;
    // Build message: chips + text
    let message = text;
    const fileChips = chips.filter(c => c.type === "file");
    const cmdChips = chips.filter(c => c.type === "command");
    if (fileChips.length > 0) message = fileChips.map(c => `@${c.value}`).join(" ") + (message ? "\n" + message : "");
    if (cmdChips.length > 0) message = cmdChips.map(c => c.value).join(" ") + (message ? "\n" + message : "");
    if (!message.trim()) return;

    const firstLoopDraft = deriveFirstLoopDraft(sessionId, message);
    if (firstLoopDraft) {
      setFirstLoopDraft(sessionId, firstLoopDraft);
    }

    await createProjectCheckpoint(sessionId).catch(() => {});
    useStore.getState().addUserMessage(sessionId, message);
    send(sessionId, buildFirstLoopAgentPrompt(message), selectedMcpContext);
    setValue("");
    setChips([]);
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
      textareaRef.current.style.overflowY = "hidden";
    }
  }, [value, chips, sessionId, send, isRunning, setFirstLoopDraft, selectedMcpContext]);

  const handleResume = useCallback(async () => {
    if (isRunning || isResuming) return;
    setResumeError("");
    setIsResuming(true);
    try {
      await resume(sessionId);
      setTimeout(() => textareaRef.current?.focus(), 0);
    } catch (error) {
      setResumeError(error instanceof Error ? error.message : String(error));
    } finally {
      setIsResuming(false);
    }
  }, [isRunning, isResuming, resume, sessionId]);

  const selectModel = useCallback((provider: string, model: string) => {
    setSelectedProvider(provider);
    setSelectedModel(model);
    setShowModelMenu(false);
  }, [setSelectedModel, setSelectedProvider]);

  const commitActiveSuggestion = useCallback(() => {
    if (showSuggestions === "/") {
      const command = COMMANDS[activeSuggestionIndex];
      if (!command) return false;
      addChip("command", command.text);
      return true;
    }
    if (showSuggestions === "@") {
      const file = atResults[activeSuggestionIndex];
      if (!file) return false;
      addChip("file", file);
      return true;
    }
    return false;
  }, [activeSuggestionIndex, addChip, atResults, showSuggestions]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (composingRef.current || e.nativeEvent.isComposing) return;
    const suggestionCount = showSuggestions === "/" ? COMMANDS.length : showSuggestions === "@" ? atResults.length : 0;
    if (showSuggestions && suggestionCount > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setActiveSuggestionIndex((index) => (index + 1) % suggestionCount);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setActiveSuggestionIndex((index) => (index - 1 + suggestionCount) % suggestionCount);
        return;
      }
      if ((e.key === "Enter" && !e.shiftKey) || e.key === "Tab") {
        e.preventDefault();
        if (commitActiveSuggestion()) return;
      }
    }
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); }
    if (e.key === "Escape") { setShowSuggestions(null); setShowModelMenu(false); }
    // Backspace/Delete removes last chip ONLY if field was already empty
    // valueRef hasn't been updated by onChange yet, so it reflects the current DOM value
    const currentVal = valueRef.current;
    if ((e.key === "Backspace" || e.key === "Delete") && currentVal === "" && chips.length > 0) {
      e.preventDefault();
      setChips(prev => prev.slice(0, -1));
    }
  }, [atResults.length, chips.length, commitActiveSuggestion, handleSend, showSuggestions]);

  return (
    <div data-testid="composer-frame" className="forge-composer-frame relative flex-shrink-0">
      <div ref={composerRootRef} data-testid="composer-lane" className="forge-conversation-lane relative">
      {!isRunning && resumeError && (
        <div data-testid="composer-error" role="status" aria-live="polite" className="forge-composer-error">
          <AlertCircle className="size-3.5 shrink-0" />
          <span className="min-w-0 truncate">{resumeError}</span>
        </div>
      )}
      {/* Suggestion popup */}
      {showSuggestions && (
        <div
          id={suggestionListId}
          data-testid="composer-command-menu"
          role="listbox"
          aria-label={showSuggestions === "@" ? "引用文件" : "常用请求"}
          className="forge-floating-menu forge-composer-suggestion-menu">
          {showSuggestions === "@" && (
            <>
              <div className="forge-menu-heading">引用文件</div>
              {atResults.length === 0 && <div className="px-3 py-2 text-xs text-muted-foreground/65">输入文件名搜索</div>}
              {atResults.map((f, index) => {
                const meta = fileReferenceIconMeta(f);
                return (
                  <button key={f} role="option" aria-selected={index === activeSuggestionIndex} onMouseEnter={() => setActiveSuggestionIndex(index)} onClick={() => addChip("file", f)}
                    className="forge-menu-option font-mono"
                    style={index === activeSuggestionIndex ? ACTIVE_MENU_OPTION_STYLE : undefined}>
                    <ForgeIcon icon={meta.icon} tone={meta.tone} />
                    {f}
                  </button>
                );
              })}
            </>
          )}
          {showSuggestions === "/" && (
            <>
              <div className="forge-menu-heading">常用请求</div>
              {COMMANDS.map((cmd, index) => {
                const meta = commandIconMeta(cmd.text);
                return (
                  <button key={cmd.prefix} role="option" aria-selected={index === activeSuggestionIndex} onMouseEnter={() => setActiveSuggestionIndex(index)} onClick={() => addChip("command", cmd.text)}
                    className="forge-menu-option"
                    style={index === activeSuggestionIndex ? ACTIVE_MENU_OPTION_STYLE : undefined}>
                    <ForgeIcon icon={meta.icon} tone={meta.tone} />
                    <span className="min-w-0 flex-1 truncate font-mono">{cmd.text}</span>
                    <span className="text-[10px] text-muted-foreground">{cmd.desc}</span>
                  </button>
                );
              })}
            </>
          )}
        </div>
      )}

      <div
        data-testid="composer-surface"
        data-menu-open={showSuggestions || showModelMenu ? "true" : "false"}
        data-streaming={isStreaming ? "true" : "false"}
        data-state={isStreaming ? "running" : isRunning ? "busy" : "idle"}
        className="forge-composer"
      >
        {/* Chips row */}
        {chips.length > 0 && (
          <div className="forge-composer-chips">
            {chips.map((chip) => {
              const meta = chip.type === "file" ? fileReferenceIconMeta(chip.value) : commandIconMeta(chip.value);
              return (
                <span key={chip.id}
                  className="forge-composer-chip">
                  <ForgeIcon icon={meta.icon} tone={meta.tone} contained={false} className="size-3.5" />
                  {chip.value}
                  <button
                    type="button"
                    aria-label={`移除 ${chip.value}`}
                    onClick={() => removeChip(chip.id)}
                    className="ml-0.5 opacity-45 transition-opacity hover:opacity-100"
                  >
                    <X className="size-2.5" />
                  </button>
                </span>
              );
            })}
          </div>
        )}

        {/* Textarea */}
        <div data-testid="composer-textarea-wrap" className="forge-composer-textarea-wrap">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            onCompositionStart={() => {
              composingRef.current = true;
            }}
            onCompositionEnd={() => {
              composingRef.current = false;
              adjustHeight();
            }}
            placeholder={modeAwarePlaceholder(workflow, isRunning)}
            rows={1}
            disabled={!isRunning}
            className="forge-composer-textarea"
            style={{ paddingTop: chips.length > 0 ? "4px" : undefined }}
          />
        </div>

        {/* Toolbar */}
        <div data-testid="composer-toolbar" className="forge-composer-toolbar">
          <div data-testid="composer-tool-cluster" className="forge-composer-tool-cluster">
            <div className="forge-composer-tool-buttons">
              <button
                type="button"
                data-testid="composer-tool-button"
                aria-label="引用文件"
                aria-controls={showSuggestions === "@" ? suggestionListId : undefined}
                aria-expanded={showSuggestions === "@"}
                aria-haspopup="listbox"
                title="引用文件"
                onClick={() => {
                  textareaRef.current?.focus();
                  setShowModelMenu(false);
                  setShowSuggestions((s) => s === "@" ? null : "@");
                }}
                data-active={showSuggestions === "@" ? "true" : "false"}
                className="forge-composer-tool"
              >
                <ForgeIcon icon={composerToolbarIcons.file.icon} tone={composerToolbarIcons.file.tone} contained={false} />
              </button>
              <button
                type="button"
                data-testid="composer-tool-button"
                aria-label="常用请求"
                aria-controls={showSuggestions === "/" ? suggestionListId : undefined}
                aria-expanded={showSuggestions === "/"}
                aria-haspopup="listbox"
                title="常用请求"
                onClick={() => {
                  textareaRef.current?.focus();
                  setShowModelMenu(false);
                  setShowSuggestions((s) => s === "/" ? null : "/");
                }}
                data-active={showSuggestions === "/" ? "true" : "false"}
                className="forge-composer-tool"
              >
                <ForgeIcon icon={composerToolbarIcons.command.icon} tone={composerToolbarIcons.command.tone} contained={false} />
              </button>
            </div>
            <span className="forge-composer-hint hidden truncate sm:inline">
              Enter 发送 · Shift↵ 换行
            </span>
          </div>

          <div data-testid="composer-control-cluster" className="forge-composer-control-cluster">
            <div className="relative">
              <button
                type="button"
                data-testid="composer-model-chip"
                id={`${modelMenuId}-button`}
                onClick={() => {
                  setShowSuggestions(null);
                  setShowModelMenu(!showModelMenu);
                }}
                aria-label={`模型：${selectedModelLabel}`}
                aria-controls={showModelMenu ? modelMenuId : undefined}
                aria-expanded={showModelMenu}
                aria-haspopup="menu"
                data-active={showModelMenu ? "true" : "false"}
                className="forge-composer-model"
                title={selectedContextWindow ? `${selectedProviderLabel} · ${selectedModelLabel} · 上下文 ${selectedContextWindow}` : `${selectedProviderLabel} · ${selectedModelLabel}`}>
                <span data-testid="composer-model-indicator" className="forge-composer-model-indicator" aria-hidden="true" />
                <span className="truncate">{selectedModelLabel}</span>
                <ChevronDown className="size-3" style={{ color: "var(--muted-foreground)" }} />
              </button>
              {showModelMenu && (
                <div
                  id={modelMenuId}
                  role="menu"
                  aria-labelledby={`${modelMenuId}-button`}
                  className="forge-floating-menu forge-composer-model-menu">
                  {PROVIDERS.map((provider) => (
                    <div key={provider.id} className="py-1">
                      <div className="forge-menu-heading flex items-center justify-between">
                        <span>{provider.label}</span>
                        <span>{provider.shortLabel}</span>
                      </div>
                      {provider.models.map((model) => {
                        const active = provider.id === selectedProvider && model.id === selectedModel;
                        return (
                          <button
                            key={`${provider.id}:${model.id}`}
                            role="menuitemradio"
                            aria-checked={active}
                            onClick={() => selectModel(provider.id, model.id)}
                            className="forge-menu-option h-auto min-h-10 flex-col items-stretch gap-0.5 py-1.5"
                            style={active ? { ...ACTIVE_MENU_OPTION_STYLE, color: "var(--primary)" } : { color: "var(--forge-text-secondary)" }}
                          >
                            <div className="flex items-center justify-between gap-3">
                              <span className="font-mono">{model.name}</span>
                              {active && <span className="text-[10px] text-primary">当前</span>}
                            </div>
                            {model.description && (
                              <div className="mt-0.5 truncate text-[10px] text-muted-foreground/75">
                                {[model.description, formatContextWindow(model.contextWindowTokens) && `上下文 ${formatContextWindow(model.contextWindowTokens)}`].filter(Boolean).join(" · ")}
                              </div>
                            )}
                          </button>
                        );
                      })}
                    </div>
                  ))}
                </div>
              )}
            </div>

            {!isRunning ? (
              <button
                type="button"
                aria-label="继续会话"
                onClick={handleResume}
                disabled={isResuming}
                className="forge-composer-resume disabled:cursor-default disabled:opacity-60"
              >
                <RotateCcw className={isResuming ? "size-3 animate-spin" : "size-3"} />
                {isResuming ? "恢复中" : "继续会话"}
              </button>
            ) : isStreaming ? (
              <button
                type="button"
                data-testid="composer-stop"
                aria-label="停止生成"
                onClick={() => stop(sessionId)}
                className="forge-composer-send text-destructive hover:border-destructive/35 hover:bg-destructive/10"
              >
                <X className="size-4" />
              </button>
            ) : (
              <button
                type="button"
                data-testid="composer-send"
                aria-label="发送"
                onClick={handleSend}
                disabled={!canSend}
                data-ready={canSend}
                className={cn("forge-composer-send", canSend && "text-primary")}
              >
                <ArrowUp className="size-4" />
              </button>
            )}
          </div>
        </div>
      </div>
      </div>
    </div>
  );
}
