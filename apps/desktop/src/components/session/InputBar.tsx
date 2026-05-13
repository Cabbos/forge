import { useRef, useState, useCallback, useEffect } from "react";
import { Cpu, ArrowUp, AtSign, Slash, ChevronDown, X, FileText, Terminal, Puzzle, Sparkles, RotateCcw } from "lucide-react";
import { useSession } from "@/hooks/useSession";
import { useStore } from "@/store";
import { createProjectCheckpoint, searchWorkspaceFiles, listCapabilities, type CapabilityInfo } from "@/lib/tauri";
import { formatContextWindow, getModelContextWindow, getModelLabel, getProviderDefinition, PROVIDERS } from "@/lib/providers";

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

const QUICK_PROMPTS = [
  "先梳理当前项目结构，并指出最重要的入口。",
  "检查当前改动的风险，按严重程度排序。",
  "继续优化最影响使用体验的一处问题。",
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
  const selectedContextCount = useStore((s) => s.selectedContextBySession.get(sessionId)?.length ?? 0);
  const workflow = useStore((s) => s.workflowBySession.get(sessionId) ?? null);
  const [showSuggestions, setShowSuggestions] = useState<"@" | "/" | null>(null);
  const [atResults, setAtResults] = useState<string[]>([]);
  const valueRef = useRef("");

  const { send, kill, resume } = useSession();
  const session = useStore((s) => s.sessions.get(sessionId));
  const isRunning = session?.status === "running";
  const isStreaming = session?.streaming ?? false;
  const [isResuming, setIsResuming] = useState(false);
  const [resumeError, setResumeError] = useState("");
  const [activeSkills, setActiveSkills] = useState<CapabilityInfo[]>([]);
  const selectedProviderDef = getProviderDefinition(selectedProvider);
  const selectedContextWindow = formatContextWindow(getModelContextWindow(selectedModel));

  useEffect(() => {
    listCapabilities()
      .then((all) => setActiveSkills(all.filter((c) => c.kind === "skill" && c.enabled !== false)))
      .catch(() => {});
  }, [sessionId]);

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

    await createProjectCheckpoint(sessionId).catch(() => {});
    useStore.getState().addUserMessage(sessionId, message);
    send(sessionId, message);
    setValue("");
    setChips([]);
    if (textareaRef.current) textareaRef.current.style.height = "auto";
  }, [value, chips, sessionId, send, isRunning]);

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

  const useQuickPrompt = useCallback((prompt: string) => {
    setValue(prompt);
    valueRef.current = prompt;
    setTimeout(() => {
      textareaRef.current?.focus();
      adjustHeight();
    }, 0);
  }, [adjustHeight]);

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
    <div className="relative flex-shrink-0 px-10 pb-5 pt-3" style={{ borderTop: "1px solid var(--border)" }}>
      {isRunning && !value.trim() && chips.length === 0 && (
        <div className="flex flex-wrap gap-2 mb-2">
          {QUICK_PROMPTS.map((prompt) => (
            <button
              key={prompt}
              onClick={() => useQuickPrompt(prompt)}
              className="inline-flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-[11px] transition-colors hover:bg-secondary hover:text-foreground"
              style={{ borderColor: "var(--border)", background: "var(--card)", color: "var(--muted-foreground)" }}
            >
              <Sparkles className="size-3" style={{ color: "#D4A853" }} />
              {prompt}
            </button>
          ))}
        </div>
      )}
      {!isRunning && resumeError && (
        <div className="mb-2 rounded-md border px-3 py-2 text-xs"
          style={{ borderColor: "rgba(212,119,119,0.35)", background: "rgba(212,119,119,0.08)", color: "#D47777" }}>
          {resumeError}
        </div>
      )}
      {selectedContextCount > 0 && (
        <div className="mb-2 text-[11px] text-muted-foreground/75">
          上轮带入 {selectedContextCount} 条相关背景
        </div>
      )}
      {workflow?.gate === "soft" && (
        <div className="mb-2 rounded-md border border-border bg-card px-3 py-2 text-[11px] text-muted-foreground">
          这个需求会影响多个部分，我会先帮你梳理方案。你也可以选择直接做。
        </div>
      )}
      {workflow?.gate === "approval_required" && (
        <div className="mb-2 rounded-md border border-amber-500/25 bg-amber-500/10 px-3 py-2 text-[11px] text-amber-200">
          这个请求风险较高，建议先确认方案和步骤。
        </div>
      )}

      {/* Suggestion popup */}
      {showSuggestions && (
        <div className="absolute left-10 right-10 rounded-lg py-1 shadow-xl z-20 max-h-[200px] overflow-y-auto"
          style={{ bottom: "calc(100% - 8px)", background: "var(--popover)", border: "1px solid var(--border)" }}>
          {showSuggestions === "@" && (
            <>
              <div className="px-3 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground/70">引用文件</div>
              {atResults.length === 0 && <div className="px-3 py-2 text-xs text-muted-foreground/65">输入文件名搜索</div>}
              {atResults.map(f => (
                <button key={f} onClick={() => addChip("file", f)}
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
                <button key={cmd.prefix} onClick={() => addChip("command", cmd.text)}
                  className="w-full text-left px-3 py-1.5 text-xs text-foreground hover:bg-secondary flex justify-between items-center">
                  <span className="font-mono">{cmd.text}</span>
                  <span className="text-[10px] text-muted-foreground">{cmd.desc}</span>
                </button>
              ))}
            </>
          )}
        </div>
      )}

      <div className="rounded-lg" style={{ background: "var(--card)", border: "1px solid var(--border)" }}>
        {/* Chips row */}
        {chips.length > 0 && (
          <div className="flex flex-wrap gap-1.5 px-4 pt-3 pb-0">
            {chips.map(chip => (
              <span key={chip.id}
                className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[11px] font-mono select-none"
                style={{
                  background: chip.type === "file" ? "rgba(75,156,211,0.1)" : "rgba(212,168,83,0.1)",
                  color: chip.type === "file" ? "#5B9BD5" : "#D4A853",
                  border: `1px solid ${chip.type === "file" ? "rgba(75,156,211,0.2)" : "rgba(212,168,83,0.2)"}`,
                }}>
                {chip.type === "file" ? <FileText className="size-3" /> : <Terminal className="size-3" />}
                {chip.value}
                <button onClick={() => removeChip(chip.id)} className="ml-0.5 opacity-40 hover:opacity-100">
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
            placeholder={isRunning ? "描述要完成的任务，例如：把登录页的信息层级整理清楚" : "这个会话已停止，可以继续后再发送"}
            rows={1}
            disabled={!isRunning}
            className="w-full bg-transparent border-none outline-none resize-none text-sm leading-relaxed placeholder:text-muted-foreground/65"
            style={{ color: "var(--foreground)", fontFamily: "'Geist Variable', system-ui, sans-serif", minHeight: "28px", maxHeight: "140px", paddingTop: chips.length > 0 ? "4px" : undefined }}
          />
        </div>

        {/* Toolbar */}
        <div className="flex items-center justify-between px-4 pb-2.5">
          <div className="flex gap-1.5 text-[10px] font-mono" style={{ color: "var(--muted-foreground)" }}>
            <button onClick={() => { textareaRef.current?.focus(); setShowSuggestions((s) => s === "@" ? null : "@"); }}
              className="px-1.5 py-0.5 rounded cursor-pointer hover:text-foreground inline-flex items-center gap-1 transition-colors" style={{ background: "var(--secondary)" }}>
              <AtSign className="size-3" /> 引用文件
            </button>
            <button onClick={() => { textareaRef.current?.focus(); setShowSuggestions((s) => s === "/" ? null : "/"); }}
              className="px-1.5 py-0.5 rounded cursor-pointer hover:text-foreground inline-flex items-center gap-1 transition-colors" style={{ background: "var(--secondary)" }}>
              <Slash className="size-3" /> 常用请求
            </button>
          </div>

          <div className="flex items-center gap-2">
            <div className="relative">
              <button onClick={() => setShowModelMenu(!showModelMenu)}
                className="flex items-center gap-1 px-2 py-1 rounded text-[10px] font-mono transition-colors"
                style={{ border: "1px solid var(--border)", color: "var(--muted-foreground)", background: "var(--card)" }}>
                <Cpu className="size-3" style={{ color: "#D4A853" }} />
                <span className="text-primary">{selectedProviderDef.shortLabel}</span>
                <span>{getModelLabel(selectedModel)}</span>
                {selectedContextWindow && <span className="text-muted-foreground">上下文 {selectedContextWindow}</span>}
                <ChevronDown className="size-3" style={{ color: "var(--muted-foreground)" }} />
              </button>
              {showModelMenu && (
                <div className="absolute bottom-full right-0 mb-1 max-h-[320px] min-w-[260px] overflow-y-auto rounded-lg py-1.5 shadow-xl z-20"
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

            {/* Active skills badge */}
            {activeSkills.length > 0 && (
              <div className="relative group">
                <span
                  className="flex items-center gap-1 px-2 py-1 rounded text-[10px] font-mono cursor-default"
                  style={{ border: "1px solid rgba(91,155,213,0.3)", color: "#5B9BD5", background: "rgba(91,155,213,0.08)" }}>
                  <Puzzle className="size-3" />
                  {activeSkills.length}
                </span>
                <div className="absolute bottom-full right-0 mb-1 rounded-lg py-1.5 px-3 min-w-[160px] shadow-xl z-20 hidden group-hover:block"
                  style={{ background: "var(--popover)", border: "1px solid var(--border)" }}>
                  <div className="text-[10px] uppercase tracking-wider text-muted-foreground/70 mb-1">已启用插件</div>
                  {activeSkills.map((s) => (
                    <div key={s.id} className="text-xs text-foreground py-0.5 font-mono" style={{ color: "#5B9BD5" }}>
                      {s.name}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {!isRunning ? (
              <button
                onClick={handleResume}
                disabled={isResuming}
                className="h-7 rounded-full px-3 text-[11px] font-medium flex items-center gap-1.5 flex-shrink-0 transition-colors"
                style={{ background: isResuming ? "var(--secondary)" : "#D4A853", color: isResuming ? "#8C93A0" : "#111216", cursor: isResuming ? "default" : "pointer" }}
              >
                <RotateCcw className={isResuming ? "size-3 animate-spin" : "size-3"} />
                {isResuming ? "恢复中" : "继续会话"}
              </button>
            ) : isStreaming ? (
              <button onClick={() => kill(sessionId)}
                className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0 transition-colors animate-pulse"
                style={{ background: "#D47777", color: "#fff", cursor: "pointer" }}>
                <X className="size-4" />
              </button>
            ) : (
              <button onClick={handleSend} disabled={!isRunning || (!value.trim() && chips.length === 0)}
                className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0 transition-colors"
                style={{ background: !isRunning || (!value.trim() && chips.length === 0) ? "var(--secondary)" : "#D4A853", color: !isRunning || (!value.trim() && chips.length === 0) ? "#8C93A0" : "#111216", cursor: !isRunning || (!value.trim() && chips.length === 0) ? "default" : "pointer" }}>
                <ArrowUp className="size-4" />
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
