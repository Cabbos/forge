import { useRef } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { formatContextWindow, PROVIDERS, type ProviderDefinition } from "@/lib/providers";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

interface ComposerModelMenuProps {
  id: string;
  labelledBy: string;
  selectedModel: string;
  selectedProvider: string;
  providers?: ProviderDefinition[];
  onSelect: (provider: string, model: string) => void;
}

export function ComposerModelMenu({
  id,
  labelledBy,
  providers = PROVIDERS,
  selectedModel,
  selectedProvider,
  onSelect,
}: ComposerModelMenuProps) {
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
      role="menu"
      aria-labelledby={labelledBy}
      className="forge-floating-menu forge-composer-model-menu"
    >
      {providers.map((provider) => (
        <div key={provider.id} className="py-1">
          <div className="forge-menu-heading flex items-center justify-between">
            <span>{provider.label}</span>
            <span>{provider.shortLabel}</span>
          </div>
          {provider.models.map((model) => {
            const active = provider.id === selectedProvider && model.id === selectedModel;
            return (
              <ButtonPrimitive
                key={`${provider.id}:${model.id}`}
                role="menuitemradio"
                aria-checked={active}
                onClick={() => onSelect(provider.id, model.id)}
                className="forge-menu-option h-auto min-h-10 flex-col items-stretch gap-0.5 py-1.5"
              >
                <div className="flex items-center justify-between gap-3">
                  <span className="font-mono">{model.name}</span>
                  {active && (
                    <span data-testid="composer-model-current-badge" className="forge-composer-model-current">
                      当前
                    </span>
                  )}
                </div>
                {model.description && (
                  <div className="mt-0.5 truncate text-[10px] text-muted-foreground/75">
                    {[model.description, formatContextWindow(model.contextWindowTokens) && `上下文 ${formatContextWindow(model.contextWindowTokens)}`].filter(Boolean).join(" · ")}
                  </div>
                )}
              </ButtonPrimitive>
            );
          })}
        </div>
      ))}
    </div>
  );
}
