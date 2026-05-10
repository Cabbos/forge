import { useEffect, useRef, useState, useCallback } from "react";
import { EditorView, keymap, placeholder as cmPlaceholder } from "@codemirror/view";
import { EditorState, Compartment } from "@codemirror/state";
import { history, historyKeymap, defaultKeymap } from "@codemirror/commands";
import { useSession } from "../../hooks/useSession";
import { useStore } from "../../store";
import { Button } from "../ui/button";
import { SendHorizonal } from "lucide-react";
import { cn } from "../../lib/utils";

interface InputBarProps { sessionId: string }

export function InputBar({ sessionId }: InputBarProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const editorRef = useRef<EditorView | null>(null);
  const [isEmpty, setIsEmpty] = useState(true);
  const { send } = useSession();
  const session = useStore((s) => s.sessions.get(sessionId));
  const isRunning = session?.status === "running";
  const theme = useStore((s) => s.theme);

  const sendRef = useRef(send);
  sendRef.current = send;
  const sessionIdRef = useRef(sessionId);
  sessionIdRef.current = sessionId;
  const docRef = useRef("");
  const readOnlyCompartment = useRef(new Compartment());
  const placeholderCompartment = useRef(new Compartment());

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;
    const isDark = theme === "dark";

    const sendBinding = {
      key: "Enter",
      run: (view: EditorView): boolean => {
        const text = view.state.doc.toString();
        if (!text.trim()) return true;
        const sid = sessionIdRef.current;
        useStore.getState().addUserMessage(sid, text);
        sendRef.current(sid, text).then(() => {
          view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: "" } });
          view.focus();
        }).catch(() => view.focus());
        return true;
      },
    };

    const shiftEnterBinding = {
      key: "Shift-Enter",
      run: (view: EditorView): boolean => {
        view.dispatch(view.state.replaceSelection("\n"));
        return true;
      },
    };

    const theme_ = EditorView.theme({
      "&": { fontFamily: "'Geist Variable', monospace", fontSize: "14px", backgroundColor: "transparent", color: isDark ? "#e6edf3" : "#0d1117", border: "none" },
      ".cm-content": { padding: "10px 14px", caretColor: isDark ? "#58a6ff" : "#4a6cf7", minHeight: "36px" },
      ".cm-scroller": { overflow: "auto !important", maxHeight: "200px", fontFamily: "inherit", lineHeight: "1.6" },
      ".cm-line": { lineHeight: "1.6" },
      ".cm-placeholder": { color: isDark ? "#484f58" : "#8b949e", fontStyle: "italic" },
      "&.cm-editor.cm-focused": { outline: "none" },
      ".cm-gutters": { display: "none" },
      "&.cm-focused .cm-selectionBackground, ::selection": { backgroundColor: isDark ? "rgba(88,166,255,0.2)" : "rgba(74,108,247,0.15)" },
      ".cm-selectionBackground": { backgroundColor: isDark ? "rgba(88,166,255,0.12)" : "rgba(74,108,247,0.08)" },
    }, { dark: isDark });

    const extensions = [
      history(),
      keymap.of([...defaultKeymap, ...historyKeymap, sendBinding, shiftEnterBinding]),
      EditorView.lineWrapping,
      readOnlyCompartment.current.of(EditorState.readOnly.of(!isRunning)),
      placeholderCompartment.current.of(cmPlaceholder(isRunning ? "Message... (Enter to send)" : "Session not running")),
      theme_,
      EditorView.updateListener.of((update) => {
        if (update.docChanged) { docRef.current = update.state.doc.toString(); setIsEmpty(update.state.doc.toString().trim() === ""); }
      }),
    ];

    const view = new EditorView({ state: EditorState.create({ doc: docRef.current, extensions }), parent: container });
    editorRef.current = view;
    return () => { editorRef.current = null; view.destroy(); };
  }, [sessionId, theme]);

  useEffect(() => {
    const view = editorRef.current;
    if (!view) return;
    view.dispatch({
      effects: [
        readOnlyCompartment.current.reconfigure(EditorState.readOnly.of(!isRunning)),
        placeholderCompartment.current.reconfigure(cmPlaceholder(isRunning ? "Message... (Enter to send)" : "Session not running")),
      ],
    });
  }, [isRunning]);

  const handleClick = useCallback(() => {
    const view = editorRef.current;
    if (!view) return;
    const text = view.state.doc.toString();
    if (!text.trim()) return;
    useStore.getState().addUserMessage(sessionId, text);
    send(sessionId, text).then(() => {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: "" } });
      view.focus();
    }).catch(() => view.focus());
  }, [sessionId, send]);

  return (
    <div className="px-4 pb-4">
      <div className="max-w-3xl mx-auto flex items-end gap-2">
        <div ref={containerRef} className={cn(
          "flex-1 rounded-xl bg-muted/30 border border-border/30 overflow-hidden transition-all duration-200",
          "focus-within:ring-1 focus-within:ring-primary/30 focus-within:border-primary/30",
          !isRunning && "opacity-40 pointer-events-none"
        )} />
        <Button onClick={handleClick} disabled={!isRunning || isEmpty} size="icon"
          className="shrink-0 rounded-xl size-9 transition-all duration-200" title="Send (Enter)">
          <SendHorizonal className="size-4" />
        </Button>
      </div>
    </div>
  );
}
