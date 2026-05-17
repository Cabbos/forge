import { useRef, useState, useCallback, useEffect } from "react";
import { ArrowUp, AtSign, Slash, ChevronDown, X, FileText, Terminal, RotateCcw } from "lucide-react";
import { useSession } from "@/hooks/useSession";
import { useStore } from "@/store";
import { createProjectCheckpoint, searchWorkspaceFiles } from "@/lib/tauri";
import { formatContextWindow, getModelContextWindow, getModelLabel, getProviderDefinition, PROVIDERS } from "@/lib/providers";
import { modeAwarePlaceholder } from "@/lib/task-mode";
import { buildFirstLoopAgentPrompt, deriveFirstLoopDraft } from "@/lib/first-loop";
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

export function InputBar({ sessionId }: InputBarProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
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
  const workflow = useStore((s) => s.workflowBySession.get(sessionId) ?? null);
  const [showSuggestions, setShowSuggestions] = useState<"@" | "/" | null>(null);
  const [atResults, setAtResults] = useState<string[]>([]);
  const valueRef = useRef("");
  const suggestionListId = `${sessionId}-composer-suggestions`;
  const modelMenuId = `${sessionId}-model-menu`;

  const { send, kill, resume } = useSession();
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

  const adjustHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 140) + "px";
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
    if (chips.some(c => c.value === val)) return;
    setChips(prev => [...prev, { id: crypto.randomUUID(), type, value: val }]);
    setShowSuggestions(null);
    // Remove the @ or / trigger text from the textarea
    setValue(prev => {
      const pos = textareaRef.current?.selectionStart ?? prev.length;
      const before = prev.slice(0, pos);
      const after = prev.slice(pos);
      const lastAt = before.lastIndexOf(type === "file" ? "@" : "/");
      if (lastAt >= 0) return before.slice(0, lastAt) + after;
      return prev;
    });
    setTimeout(() => textareaRef.current?.focus(), 0);
  }, [chips]);

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
      setShowSuggestions("@");
      const q = lastWord.slice(1);
      searchWorkspaceFiles(q).then(setAtResults).catch(() => setAtResults([]));
    } else if (lastWord === "/") {
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
    send(sessionId, buildFirstLoopAgentPrompt(message));
    setValue("");
    setChips([]);
    if (textareaRef.current) textareaRef.current.style.height = "auto";
  }, [value, chips, sessionId, send, isRunning, setFirstLoopDraft]);

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

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing) return;
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); handleSend(); }
    if (e.key === "Escape") { setShowSuggestions(null); setShowModelMenu(false); }
    // Backspace/Delete removes last chip ONLY if field was already empty
    // valueRef hasn't been updated by onChange yet, so it reflects the current DOM value
    const currentVal = valueRef.current;
    if ((e.key === "Backspace" || e.key === "Delete") && currentVal === "" && chips.length > 0) {
      e.preventDefault();
      setChips(prev => prev.slice(0, -1));
    }
  }, [handleSend, chips.length]);

  return (
    <div className="relative flex-shrink-0 border-t px-4 pb-4 pt-3 sm:px-6" style={{ borderColor: "var(--border)" }}>
      <div data-testid="composer-lane" className="relative mx-auto w-full max-w-[820px]">
      {!isRunning && resumeError && (
        <div className="mb-2 rounded-md border px-3 py-2 text-xs"
          style={{ borderColor: "rgba(212,119,119,0.35)", background: "rgba(212,119,119,0.08)", color: "#D47777" }}>
          {resumeError}
        </div>
      )}
      {/* Suggestion popup */}
      {showSuggestions && (
        <div
          id={suggestionListId}
          data-testid="composer-command-menu"
          role="listbox"
          aria-label={showSuggestions === "@" ? "引用文件" : "常用请求"}
          className="absolute left-0 right-0 rounded-md py-1 shadow-xl z-20 max-h-[200px] overflow-y-auto"
          style={{ bottom: "calc(100% - 8px)", background: "var(--popover)", border: "1px solid var(--border)" }}>
          {showSuggestions === "@" && (
            <>
              <div className="px-3 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground/70">引用文件</div>
              {atResults.length === 0 && <div className="px-3 py-2 text-xs text-muted-foreground/65">输入文件名搜索</div>}
              {atResults.map(f => (
                <button key={f} role="option" aria-selected="false" onClick={() => addChip("file", f)}
                  className="w-full text-left px-3 py-1.5 text-xs text-foreground hover:bg-secondary font-mono flex items-center gap-2">
                  {f.endsWith("/") ? <FileText className="size-3" style={{ color: "#5B9BD5" }} /> : <FileText className="size-3" style={{ color: "var(--muted-foreground)" }} />}
                  {f}
                </button>
              ))}
            </>
          )}
          {showSuggestions === "/" && (
            <>
              <div className="px-3 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground/70">常用请求</div>
              {COMMANDS.map(cmd => (
                <button key={cmd.prefix} role="option" aria-selected="false" onClick={() => addChip("command", cmd.text)}
                  className="w-full text-left px-3 py-1.5 text-xs text-foreground hover:bg-secondary flex justify-between items-center">
                  <span className="font-mono">{cmd.text}</span>
                  <span className="text-[10px] text-muted-foreground">{cmd.desc}</span>
                </button>
              ))}
            </>
          )}
        </div>
      )}

      <div data-testid="composer-surface" className="forge-composer">
        {/* Chips row */}
        {chips.length > 0 && (
          <div className="flex flex-wrap gap-1.5 px-4 pt-3 pb-0">
            {chips.map(chip => (
              <span key={chip.id}
                className="forge-composer-chip">
                {chip.type === "file" ? (
                  <FileText className="size-3 text-[#6BA6D8]" />
                ) : (
                  <Terminal className="size-3 text-primary/80" />
                )}
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
            ))}
          </div>
        )}

        {/* Textarea */}
        <div className="px-4 pt-3 pb-1">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={handleChange}
            onKeyDown={handleKeyDown}
            placeholder={modeAwarePlaceholder(workflow, isRunning)}
            rows={1}
            disabled={!isRunning}
            className="w-full bg-transparent border-none outline-none resize-none text-sm leading-relaxed placeholder:text-muted-foreground/65"
            style={{ color: "var(--foreground)", fontFamily: "'Geist Variable', system-ui, sans-serif", minHeight: "28px", maxHeight: "140px", paddingTop: chips.length > 0 ? "4px" : undefined }}
          />
        </div>

        {/* Toolbar */}
        <div className="flex items-center justify-between px-4 pb-2.5">
          <div className="flex gap-1.5 text-[10px] font-mono" style={{ color: "var(--muted-foreground)" }}>
            <button
              type="button"
              aria-label="引用文件"
              aria-controls={showSuggestions === "@" ? suggestionListId : undefined}
              aria-expanded={showSuggestions === "@"}
              aria-haspopup="listbox"
              title="引用文件"
              onClick={() => { textareaRef.current?.focus(); setShowSuggestions((s) => s === "@" ? null : "@"); }}
              className="forge-composer-tool"
            >
              <AtSign className="size-3.5" />
            </button>
            <button
              type="button"
              aria-label="常用请求"
              aria-controls={showSuggestions === "/" ? suggestionListId : undefined}
              aria-expanded={showSuggestions === "/"}
              aria-haspopup="listbox"
              title="常用请求"
              onClick={() => { textareaRef.current?.focus(); setShowSuggestions((s) => s === "/" ? null : "/"); }}
              className="forge-composer-tool"
            >
              <Slash className="size-3.5" />
            </button>
          </div>

          <div className="flex items-center gap-2">
            <div className="relative">
              <button
                type="button"
                id={`${modelMenuId}-button`}
                onClick={() => setShowModelMenu(!showModelMenu)}
                aria-label={`模型：${selectedModelLabel}`}
                aria-controls={showModelMenu ? modelMenuId : undefined}
                aria-expanded={showModelMenu}
                aria-haspopup="menu"
                className="flex h-7 max-w-[190px] items-center gap-1 rounded-md px-1.5 text-[11px] transition-colors hover:bg-secondary hover:text-foreground"
                style={{ color: "var(--muted-foreground)", background: "transparent" }}
                title={selectedContextWindow ? `${selectedProviderLabel} · ${selectedModelLabel} · 上下文 ${selectedContextWindow}` : `${selectedProviderLabel} · ${selectedModelLabel}`}>
                <span className="truncate">{selectedModelLabel}</span>
                <ChevronDown className="size-3" style={{ color: "var(--muted-foreground)" }} />
              </button>
              {showModelMenu && (
                <div
                  id={modelMenuId}
                  role="menu"
                  aria-labelledby={`${modelMenuId}-button`}
                  className="absolute bottom-full right-0 mb-1 max-h-[320px] min-w-[260px] overflow-y-auto rounded-md py-1.5 shadow-xl z-20"
                  style={{ background: "var(--popover)", border: "1px solid var(--border)" }}>
                  {PROVIDERS.map((provider) => (
                    <div key={provider.id} className="py-1">
                      <div className="flex items-center justify-between px-3 pb-1 pt-1 text-[10px] uppercase tracking-wider text-muted-foreground/70">
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
                            className="w-full px-3 py-1.5 text-left text-xs transition-colors hover:bg-secondary"
                            style={{ color: active ? "#D4A853" : "#E4E7EC" }}
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
                onClick={() => kill(sessionId)}
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
