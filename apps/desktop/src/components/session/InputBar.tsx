import { useRef, useState, useCallback, useEffect } from "react";
import { useSession } from "@/hooks/useSession";
import { useStore } from "@/store";

interface InputBarProps { sessionId: string }

const MODELS = [
  { id: "deepseek-v4-pro", name: "V4 Pro" },
  { id: "deepseek-v4-flash", name: "V4 Flash" },
];

export function InputBar({ sessionId }: InputBarProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [value, setValue] = useState("");
  const [showModelMenu, setShowModelMenu] = useState(false);
  const [selectedModel, setSelectedModel] = useState("deepseek-v4-pro");

  const { send } = useSession();
  const session = useStore((s) => s.sessions.get(sessionId));
  const isRunning = session?.status === "running";

  // Auto-focus textarea when session becomes active
  useEffect(() => {
    if (isRunning) textareaRef.current?.focus();
  }, [sessionId, isRunning]);

  // Auto-resize textarea
  const adjustHeight = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = Math.min(el.scrollHeight, 140) + "px";
  }, []);

  const handleSend = useCallback(() => {
    const text = value.trim();
    if (!text || !isRunning) return;
    useStore.getState().addUserMessage(sessionId, text);
    send(sessionId, text);
    setValue("");
    // Reset height
    if (textareaRef.current) {
      textareaRef.current.style.height = "auto";
    }
  }, [value, sessionId, send, isRunning]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing) return; // Skip during IME composition
    // Enter sends, Shift+Enter for newline
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
    // Escape blurs
    if (e.key === "Escape") {
      e.preventDefault();
      (e.target as HTMLTextAreaElement).blur();
    }
  }, [handleSend]);

  return (
    <div className="px-10 pb-5 pt-3 flex-shrink-0" style={{ borderTop: "1px solid #1c1c1c" }}>
      <div className="rounded-2xl overflow-hidden" style={{ background: "#0F0F0F", border: "1px solid #1c1c1c" }}>
        {/* Textarea */}
        <div className="px-4 pt-3 pb-1">
          <textarea
            ref={textareaRef}
            value={value}
            onChange={(e) => { setValue(e.target.value); adjustHeight(); }}
            onKeyDown={handleKeyDown}
            placeholder={isRunning ? "Message @file /command..." : "Session not running"}
            rows={1}
            disabled={!isRunning}
            className="w-full bg-transparent border-none outline-none resize-none text-sm leading-relaxed placeholder:text-[#555]"
            style={{ color: "#E4E4E4", fontFamily: "'Geist Variable', system-ui, sans-serif", minHeight: "28px", maxHeight: "140px" }}
          />
        </div>

        {/* Toolbar: hints + model selector + send */}
        <div className="flex items-center justify-between px-4 pb-2.5">
          <div className="flex gap-1.5 text-[10px] font-mono" style={{ color: "#555" }}>
            <span className="px-1.5 py-0.5 rounded cursor-pointer hover:text-[#999]" style={{ background: "#111" }}>@ files</span>
            <span className="px-1.5 py-0.5 rounded cursor-pointer hover:text-[#999]" style={{ background: "#111" }}>/ commands</span>
          </div>

          <div className="flex items-center gap-2">
            {/* Model selector */}
            <div className="relative">
              <button onClick={() => setShowModelMenu(!showModelMenu)}
                className="flex items-center gap-1 px-2 py-1 rounded text-[10px] font-mono transition-colors"
                style={{ border: "1px solid #1c1c1c", color: "#999", background: "#0f0f0f" }}>
                🐋 {MODELS.find(m => m.id === selectedModel)?.name ?? selectedModel}
                <span style={{ fontSize: "7px", color: "#555" }}>▾</span>
              </button>
              {showModelMenu && (
                <div className="absolute bottom-full right-0 mb-1 rounded-lg py-1 min-w-[160px] shadow-xl z-20"
                  style={{ background: "#141414", border: "1px solid #1c1c1c" }}>
                  {MODELS.map(m => (
                    <button key={m.id} onClick={() => { setSelectedModel(m.id); setShowModelMenu(false); }}
                      className="w-full text-left px-3 py-1.5 text-xs transition-colors font-mono flex justify-between items-center"
                      style={{ color: m.id === selectedModel ? "#D4A853" : "#c0c0c0" }}>
                      {m.name}
                      <span style={{ color: "#555", fontSize: "9px" }}>DS</span>
                    </button>
                  ))}
                </div>
              )}
            </div>

            {/* Send button */}
            <button
              onClick={handleSend}
              disabled={!isRunning || !value.trim()}
              className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0 transition-colors"
              style={{
                background: !isRunning || !value.trim() ? "#141414" : "#D4A853",
                color: !isRunning || !value.trim() ? "#666" : "#0D0D0D",
                cursor: !isRunning || !value.trim() ? "default" : "pointer",
              }}>
              ↑
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
