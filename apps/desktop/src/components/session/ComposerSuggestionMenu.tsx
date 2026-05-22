import type { CSSProperties } from "react";
import { commandIconMeta, fileReferenceIconMeta } from "@/lib/capability-icons";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import { COMPOSER_COMMANDS } from "./composerCommands";
import type { ComposerChip, ComposerMenuMode } from "./composerTypes";

const ACTIVE_MENU_OPTION_STYLE: CSSProperties = {
  backgroundColor: "rgba(255, 255, 255, 0.052)",
  borderColor: "var(--forge-border-subtle)",
  color: "var(--forge-text-primary)",
};

interface ComposerSuggestionMenuProps {
  activeIndex: number;
  atResults: string[];
  id: string;
  mode: Exclude<ComposerMenuMode, null>;
  onActiveIndexChange: (index: number) => void;
  onAddChip: (type: ComposerChip["type"], value: string) => void;
}

export function ComposerSuggestionMenu({
  activeIndex,
  atResults,
  id,
  mode,
  onActiveIndexChange,
  onAddChip,
}: ComposerSuggestionMenuProps) {
  return (
    <div
      id={id}
      data-testid="composer-command-menu"
      role="listbox"
      aria-label={mode === "@" ? "引用文件" : "常用请求"}
      className="forge-floating-menu forge-composer-suggestion-menu"
    >
      {mode === "@" && (
        <>
          <div className="forge-menu-heading">引用文件</div>
          {atResults.length === 0 && <div className="px-3 py-2 text-xs text-muted-foreground/65">输入文件名搜索</div>}
          {atResults.map((file, index) => {
            const meta = fileReferenceIconMeta(file);
            return (
              <button
                key={file}
                role="option"
                aria-selected={index === activeIndex}
                onMouseEnter={() => onActiveIndexChange(index)}
                onClick={() => onAddChip("file", file)}
                className="forge-menu-option forge-menu-option--path font-mono"
                title={file}
                style={index === activeIndex ? ACTIVE_MENU_OPTION_STYLE : undefined}
              >
                <ForgeIcon icon={meta.icon} tone={meta.tone} />
                <span className="forge-menu-option-label">{file}</span>
              </button>
            );
          })}
        </>
      )}
      {mode === "/" && (
        <>
          <div className="forge-menu-heading">常用请求</div>
          {COMPOSER_COMMANDS.map((command, index) => {
            const meta = commandIconMeta(command.text);
            return (
              <button
                key={command.prefix}
                role="option"
                aria-selected={index === activeIndex}
                onMouseEnter={() => onActiveIndexChange(index)}
                onClick={() => onAddChip("command", command.text)}
                className="forge-menu-option"
                style={index === activeIndex ? ACTIVE_MENU_OPTION_STYLE : undefined}
              >
                <ForgeIcon icon={meta.icon} tone={meta.tone} />
                <span className="forge-menu-option-label font-mono">{command.text}</span>
                <span className="forge-menu-option-meta">{command.desc}</span>
              </button>
            );
          })}
        </>
      )}
    </div>
  );
}
