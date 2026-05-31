import { useRef } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { commandIconMeta, fileReferenceIconMeta } from "@/lib/capability-icons";
import { ForgeIcon } from "@/components/primitives/icon";
import { COMPOSER_COMMANDS } from "./composerCommands";
import type { ComposerChip, ComposerMenuMode } from "./composerTypes";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

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
  const menuRef = useRef<HTMLDivElement>(null);

  useGSAP(() => {
    if (prefersReducedMotion()) return;
    const menu = menuRef.current;
    if (!menu) return;

    gsap.fromTo(
      menu,
      { autoAlpha: 0, y: 6, scale: 0.99 },
      {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        duration: forgeMotion.evidence.duration,
        ease: forgeMotion.evidence.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
  }, { scope: menuRef });

  return (
    <div
      ref={menuRef}
      id={id}
      data-testid="composer-command-menu"
      role="listbox"
      aria-label={mode === "@" ? "引用文件" : "常用请求"}
      className="forge-floating-menu forge-composer-suggestion-menu"
    >
      {mode === "@" && (
        <>
          <div className="forge-menu-heading">引用文件</div>
          {atResults.length === 0 && <div className="px-3 py-2 text-xs text-muted-foreground/80">输入文件名搜索</div>}
          {atResults.map((file, index) => {
            const meta = fileReferenceIconMeta(file);
            return (
              <ButtonPrimitive
                key={file}
                role="option"
                aria-selected={index === activeIndex}
                onPointerMove={() => onActiveIndexChange(index)}
                onClick={() => onAddChip("file", file)}
                className="forge-menu-option forge-menu-option--path font-mono"
                title={file}
              >
                <ForgeIcon icon={meta.icon} tone={meta.tone} />
                <span className="forge-menu-option-label">{file}</span>
              </ButtonPrimitive>
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
              <ButtonPrimitive
                key={command.prefix}
                role="option"
                aria-selected={index === activeIndex}
                onPointerMove={() => onActiveIndexChange(index)}
                onClick={() => onAddChip("command", command.text)}
                className="forge-menu-option"
              >
                <ForgeIcon icon={meta.icon} tone={meta.tone} />
                <span className="forge-menu-option-label font-mono">{command.text}</span>
                <span className="forge-menu-option-meta">{command.desc}</span>
              </ButtonPrimitive>
            );
          })}
        </>
      )}
    </div>
  );
}
