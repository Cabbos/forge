import { useRef, useState, useCallback, useEffect } from "react";
import { Cpu, ArrowUp, AtSign, Slash, ChevronDown, X, FileText, Terminal, Puzzle, Sparkles } from "lucide-react";
import { useSession } from "@/hooks/useSession";
import { useStore } from "@/store";
import { searchWorkspaceFiles, listCapabilities, type CapabilityInfo } from "@/lib/tauri";

interface InputBarProps { sessionId: string }

interface Chip { id: string; type: "file" | "command"; value: string; }

const MODELS = [
  { id: "deepseek-v4-pro[1m]", name: "V4 Pro" },
  { id: "deepseek-v4-flash[1m]", name: "V4 Flash" },
];

const COMMANDS = [
  { prefix: "/cr", text: "/code-review", desc: "检查有没有风险" },
  { prefix: "/fix", text: "/fix", desc: "帮我修一个问题" },
  { prefix: "/explain", text: "/explain", desc: "用人话解释" },
  { prefix: "/refactor", text: "/refactor", desc: "整理代码结构" },
  { prefix: "/test", text: "/test", desc: "确认改动没坏" },
  { prefix: "/docs", text: "/docs", desc: "补充说明文档" },
];

const QUICK_PROMPTS = [
  "先帮我看懂当前项目，告诉我它能做什么、哪里最重要。",
  "帮我检查当前改动有没有明显风险，并用小白能懂的话总结。",
  "帮我把界面往小白友好方向优化，先列出你准备改哪里。",
];

export function InputBar({ sessionId }: InputBarProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [value, setValue] = useState("");
  const [chips, setChips] = useState<Chip[]>([]);
  const [showModelMenu, setShowModelMenu] = useState(false);
  const selectedModel = useStore((s) => s.selectedModel);
  const setSelectedModel = useStore((s) => s.setSelectedModel);
  const [showSuggestions, setShowSuggestions] = useState<"@" | "/" | null>(null);
  const [atResults, setAtResults] = useState<string[]>([]);
  const valueRef = useRef("");

  const { send, kill } = useSession();
  const session = useStore((s) => s.sessions.get(sessionId));
  const isRunning = session?.status === "running";
  const isStreaming = session?.streaming ?? false;
  const [activeSkills, setActiveSkills] = useState<CapabilityInfo[]>([]);

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

  const handleSend = useCallback(() => {
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

    useStore.getState().addUserMessage(sessionId, message);
    send(sessionId, message);
    setValue("");
    setChips([]);
    if (textareaRef.current) textareaRef.current.style.height = "auto";
  }, [value, chips, sessionId, send, isRunning]);

  const useQuickPrompt = useCallback((prompt: string) => {
    setValue(prompt);
    valueRef.current = prompt;
    setTimeout(() => {
      textareaRef.current?.focus();
      adjustHeight();
    }, 0);
  }, [adjustHeight]);

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
    <div className="px-10 pb-5 pt-3 flex-shrink-0 relative" style={{ borderTop: "1px solid #1c1c1c" }}>
      {isRunning && !value.trim() && chips.length === 0 && (
        <div className="flex flex-wrap gap-2 mb-2">
          {QUICK_PROMPTS.map((prompt) => (
            <button
              key={prompt}
              onClick={() => useQuickPrompt(prompt)}
              className="inline-flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-[11px] transition-colors hover:text-foreground"
              style={{ borderColor: "#1c1c1c", background: "#101010", color: "#777" }}
            >
              <Sparkles className="size-3" style={{ color: "#D4A853" }} />
              {prompt}
            </button>
          ))}
        </div>
      )}

      {/* Suggestion popup */}
      {showSuggestions && (
        <div className="absolute left-10 right-10 rounded-lg py-1 shadow-xl z-20 max-h-[200px] overflow-y-auto"
          style={{ bottom: "calc(100% - 8px)", background: "#141414", border: "1px solid #1c1c1c" }}>
          {showSuggestions === "@" && (
            <>
              <div className="px-3 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground/40">选择相关文件</div>
              {atResults.length === 0 && <div className="px-3 py-2 text-xs text-muted-foreground/40">输入文件名来搜索</div>}
              {atResults.map(f => (
                <button key={f} onClick={() => addChip("file", f)}
                  className="w-full text-left px-3 py-1.5 text-xs text-foreground hover:bg-secondary font-mono flex items-center gap-2">
                  {f.endsWith("/") ? <FileText className="size-3" style={{ color: "#5B9BD5" }} /> : <FileText className="size-3" style={{ color: "#888" }} />}
                  {f}
                </button>
              ))}
            </>
          )}
          {showSuggestions === "/" && (
            <>
              <div className="px-3 py-1.5 text-[10px] uppercase tracking-wider text-muted-foreground/40">常用请求</div>
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

      <div className="rounded-2xl" style={{ background: "#0F0F0F", border: "1px solid #1c1c1c" }}>
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
            placeholder={isRunning ? "描述你想完成的事，比如：帮我把登录页面改得更清楚" : "这个会话已停止"}
            rows={1}
            disabled={!isRunning}
            className="w-full bg-transparent border-none outline-none resize-none text-sm leading-relaxed placeholder:text-[#555]"
            style={{ color: "#E4E4E4", fontFamily: "'Geist Variable', system-ui, sans-serif", minHeight: "28px", maxHeight: "140px", paddingTop: chips.length > 0 ? "4px" : undefined }}
          />
        </div>

        {/* Toolbar */}
        <div className="flex items-center justify-between px-4 pb-2.5">
          <div className="flex gap-1.5 text-[10px] font-mono" style={{ color: "#555" }}>
            <button onClick={() => { textareaRef.current?.focus(); setShowSuggestions((s) => s === "@" ? null : "@"); }}
              className="px-1.5 py-0.5 rounded cursor-pointer hover:text-[#999] inline-flex items-center gap-1 transition-colors" style={{ background: "#111" }}>
              <AtSign className="size-3" /> 添加文件
            </button>
            <button onClick={() => { textareaRef.current?.focus(); setShowSuggestions((s) => s === "/" ? null : "/"); }}
              className="px-1.5 py-0.5 rounded cursor-pointer hover:text-[#999] inline-flex items-center gap-1 transition-colors" style={{ background: "#111" }}>
              <Slash className="size-3" /> 常用请求
            </button>
          </div>

          <div className="flex items-center gap-2">
            <div className="relative">
              <button onClick={() => setShowModelMenu(!showModelMenu)}
                className="flex items-center gap-1 px-2 py-1 rounded text-[10px] font-mono transition-colors"
                style={{ border: "1px solid #1c1c1c", color: "#999", background: "#0f0f0f" }}>
                <Cpu className="size-3" style={{ color: "#D4A853" }} />
                {MODELS.find(m => m.id === selectedModel)?.name ?? selectedModel}
                <ChevronDown className="size-3" style={{ color: "#555" }} />
              </button>
              {showModelMenu && (
                <div className="absolute bottom-full right-0 mb-1 rounded-lg py-1 min-w-[140px] shadow-xl z-20"
                  style={{ background: "#141414", border: "1px solid #1c1c1c" }}>
                  {MODELS.map(m => (
                    <button key={m.id} onClick={() => { setSelectedModel(m.id); setShowModelMenu(false); }}
                      className="w-full text-left px-3 py-1.5 text-xs transition-colors font-mono flex justify-between items-center hover:bg-secondary"
                      style={{ color: m.id === selectedModel ? "#D4A853" : "#c0c0c0" }}>
                      {m.name}<span style={{ color: "#555", fontSize: "9px" }}>DS</span>
                    </button>
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
                  style={{ background: "#141414", border: "1px solid #1c1c1c" }}>
                  <div className="text-[10px] uppercase tracking-wider text-muted-foreground/40 mb-1">Active Skills</div>
                  {activeSkills.map((s) => (
                    <div key={s.id} className="text-xs text-foreground py-0.5 font-mono" style={{ color: "#5B9BD5" }}>
                      {s.name}
                    </div>
                  ))}
                </div>
              </div>
            )}

            {isStreaming ? (
              <button onClick={() => kill(sessionId)}
                className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0 transition-colors animate-pulse"
                style={{ background: "#D47777", color: "#fff", cursor: "pointer" }}>
                <X className="size-4" />
              </button>
            ) : (
              <button onClick={handleSend} disabled={!isRunning || (!value.trim() && chips.length === 0)}
                className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0 transition-colors"
                style={{ background: !isRunning || (!value.trim() && chips.length === 0) ? "#141414" : "#D4A853", color: !isRunning || (!value.trim() && chips.length === 0) ? "#666" : "#fff", cursor: !isRunning || (!value.trim() && chips.length === 0) ? "default" : "pointer" }}>
                <ArrowUp className="size-4" />
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
