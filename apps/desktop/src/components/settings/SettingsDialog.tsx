import { useCallback, useState, useEffect, useRef } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Settings, Key, Eye, EyeOff, Check, AlertCircle, Trash2, Database, ShieldCheck, Sparkles } from "lucide-react";
import { deleteSession, getApiKeyStatus, setApiKey, type KeyStatus } from "@/lib/tauri";
import { useStore } from "@/store";
import { formatContextWindow, PROVIDERS } from "@/lib/providers";
import { forgeMotion, gsap, prefersReducedMotion, useGSAP } from "@/lib/forgeMotion";

interface SettingsDialogProps {
  triggerClassName?: string;
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  hideTrigger?: boolean;
}

export function SettingsDialog({ triggerClassName, open, onOpenChange, hideTrigger = false }: SettingsDialogProps = {}) {
  const dialogRef = useRef<HTMLDivElement>(null);
  const [internalOpen, setInternalOpen] = useState(false);
  const [keys, setKeys] = useState<KeyStatus[]>([]);
  const [editing, setEditing] = useState<string | null>(null);
  const [value, setValue] = useState("");
  const [visible, setVisible] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cleared, setCleared] = useState(false);
  const sessions = useStore((s) => s.sessions);
  const removeSession = useStore((s) => s.removeSession);
  const dialogOpen = open ?? internalOpen;
  const setDialogOpen = useCallback((nextOpen: boolean) => {
    if (open === undefined) setInternalOpen(nextOpen);
    onOpenChange?.(nextOpen);
  }, [onOpenChange, open]);

  useGSAP(() => {
    if (!dialogOpen || prefersReducedMotion()) return;
    const dialog = dialogRef.current;
    if (!dialog) return;

    const rows = gsap.utils.toArray<HTMLElement>(
      ".forge-settings-summary-item, [data-testid='settings-provider-row'], .forge-settings-danger-zone",
      dialog,
    );
    const timeline = gsap.timeline();
    timeline.fromTo(
      dialog,
      { autoAlpha: 0, y: 10, scale: 0.985 },
      {
        autoAlpha: 1,
        y: 0,
        scale: 1,
        duration: forgeMotion.surface.duration,
        ease: forgeMotion.surface.ease,
        clearProps: "transform,opacity,visibility",
      },
    );
    if (rows.length > 0) {
      timeline.fromTo(
        rows,
        { autoAlpha: 0, y: 5 },
        {
          autoAlpha: 1,
          y: 0,
          duration: forgeMotion.evidence.duration,
          ease: forgeMotion.evidence.ease,
          stagger: 0.025,
          clearProps: "transform,opacity,visibility",
        },
        "-=0.1",
      );
    }
  }, { dependencies: [dialogOpen] });

  const handleClearAll = async () => {
    // Remove all sessions from the backend source of truth, then clear the UI projection.
    for (const [id] of sessions) {
      await deleteSession(id).catch(() => {});
      removeSession(id);
    }
    setCleared(true);
    setTimeout(() => setCleared(false), 3000);
  };

  const refresh = async () => {
    try {
      const status = await getApiKeyStatus();
      setKeys(status);
    } catch (e) {
      console.error("Failed to get key status:", e);
    }
  };

  useEffect(() => {
    if (dialogOpen) refresh();
  }, [dialogOpen]);

  useEffect(() => {
    const openSettings = () => setDialogOpen(true);
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === ",") {
        event.preventDefault();
        setDialogOpen(true);
      }
    };

    window.addEventListener("forge:open-settings", openSettings);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("forge:open-settings", openSettings);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [setDialogOpen]);

  const handleSave = async () => {
    if (!editing) return;
    setSaving(true);
    setError(null);
    try {
      await setApiKey(editing, value);
      setEditing(null);
      setValue("");
      await refresh();
    } catch (e) {
      setError(String(e));
    }
    setSaving(false);
  };

  const handleRemove = async (provider: string) => {
    setSaving(true);
    setError(null);
    try {
      await setApiKey(provider, "");
      await refresh();
    } catch (e) {
      setError(String(e));
    }
    setSaving(false);
  };

  const keyByProvider = new Map(keys.map((key) => [key.provider, key]));
  const knownProviderStatuses: KeyStatus[] = PROVIDERS.map((provider) =>
    keyByProvider.get(provider.id) ?? { provider: provider.id, set: false, preview: "" },
  );
  const unknownProviderStatuses = keys.filter((key) => !PROVIDERS.some((provider) => provider.id === key.provider));
  const sortedKeys = [...knownProviderStatuses, ...unknownProviderStatuses].sort((a, b) => {
    const aIndex = PROVIDERS.findIndex((provider) => provider.id === a.provider);
    const bIndex = PROVIDERS.findIndex((provider) => provider.id === b.provider);
    return (aIndex < 0 ? 99 : aIndex) - (bIndex < 0 ? 99 : bIndex);
  });
  const configuredCount = sortedKeys.filter((key) => key.set).length;
  const providerTotal = sortedKeys.length || PROVIDERS.length;

  return (
    <Dialog open={dialogOpen} onOpenChange={setDialogOpen}>
      {!hideTrigger && (
        <DialogTrigger
          render={<Button variant="ghost" size="icon-sm" aria-label="设置" title="设置" className={triggerClassName} />}
        >
          <Settings className="size-4" />
        </DialogTrigger>
      )}
      <DialogContent ref={dialogRef} data-forge-motion="settings-dialog" className="forge-settings-dialog sm:max-w-[590px]">
        <DialogHeader>
          <DialogTitle className="forge-settings-title">
            <Settings className="size-4" />
            设置
          </DialogTitle>
          <DialogDescription>
            管理模型服务和本机对话。密钥只保存在这台电脑。
          </DialogDescription>
        </DialogHeader>

        <div data-testid="settings-summary-strip" className="forge-settings-summary-strip" aria-label="设置摘要">
          <SettingsSummaryItem
            icon={<Sparkles className="size-3.5" />}
            label="模型服务"
            value={`${configuredCount}/${providerTotal} 已配置`}
          />
          <SettingsSummaryItem
            icon={<Database className="size-3.5" />}
            label="本机对话"
            value={`${sessions.size} 个`}
          />
          <SettingsSummaryItem
            icon={<ShieldCheck className="size-3.5" />}
            label="密钥存储"
            value="仅本机"
          />
        </div>

        <section className="forge-settings-section space-y-2">
          <div className="forge-settings-heading">
            <Key className="size-3.5 text-muted-foreground" />
            <h3 className="text-sm font-medium text-foreground">模型服务</h3>
          </div>
          <div data-testid="settings-preferences-panel" className="forge-settings-preferences-panel">
            {sortedKeys.map((k) => {
              const provider = PROVIDERS.find((item) => item.id === k.provider);
              const providerLabel = provider?.label ?? k.provider;
              const defaultModel = provider?.models.find((model) => model.id === provider.defaultModel);
              const defaultContext = formatContextWindow(defaultModel?.contextWindowTokens);

              return (
                <div
                  key={k.provider}
                  data-testid="settings-provider-row"
                  data-configured={k.set}
                  className="forge-settings-row"
                >
                  <div
                    className="forge-settings-provider-mark"
                    data-configured={k.set ? "true" : "false"}
                    aria-hidden="true"
                  >
                    {providerLabel.slice(0, 1)}
                  </div>
                  <div className="forge-settings-provider-copy min-w-0">
                    <div className="flex min-w-0 items-center gap-2">
                      <div className="truncate text-xs font-medium text-foreground">{providerLabel}</div>
                      <div className="truncate text-[11px] text-muted-foreground">
                        {k.set ? "已连接" : "等待密钥"}
                      </div>
                    </div>
                    {defaultModel && (
                      <>
                        <div className="mt-1 truncate text-[11px] text-muted-foreground/85">
                          {defaultModel.name}
                        </div>
                        <div className="mt-0.5 text-[10px] text-muted-foreground">
                          {["默认模型", defaultContext && `上下文 ${defaultContext}`].filter(Boolean).join(" · ")}
                        </div>
                      </>
                    )}
                  </div>

                  <div className="forge-settings-row-side">
                    <span
                      data-testid="settings-provider-status"
                      data-state={k.set ? "configured" : "empty"}
                      className="forge-settings-status-pill"
                      title={k.set ? k.preview : undefined}
                    >
                      {k.set ? "已配置" : "未配置"}
                    </span>
                    {editing !== k.provider && (
                      <div className="flex items-center justify-end gap-2">
                        <Button
                          size="xs"
                          variant="outline"
                          onClick={() => {
                            setEditing(k.provider);
                            setValue("");
                            setError(null);
                          }}
                        >
                          {k.set ? "更新" : "添加"}
                        </Button>
                        {k.set && (
                          <Button
                            size="xs"
                            variant="ghost"
                            onClick={() => handleRemove(k.provider)}
                            className="text-destructive hover:text-destructive"
                          >
                            移除
                          </Button>
                        )}
                      </div>
                    )}
                  </div>

                  {editing === k.provider && (
                    <div className="forge-settings-edit-row">
                      <div className="relative">
                        <Input
                          type={visible ? "text" : "password"}
                          value={value}
                          onChange={(e) => setValue(e.target.value)}
                          placeholder={provider?.keyPlaceholder ?? "sk-..."}
                          className="h-8 pr-9 text-xs"
                          autoFocus
                        />
                        <button
                          type="button"
                          onClick={() => setVisible(!visible)}
                          className="absolute right-2 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                          title={visible ? "隐藏密钥" : "显示密钥"}
                        >
                          {visible ? <EyeOff className="size-3.5" /> : <Eye className="size-3.5" />}
                        </button>
                      </div>
                      <div className="flex gap-1.5">
                        <Button size="xs" onClick={handleSave} disabled={saving}>
                          <Check className="size-3" />
                          保存
                        </Button>
                        <Button
                          size="xs"
                          variant="ghost"
                          onClick={() => {
                            setEditing(null);
                            setValue("");
                            setError(null);
                          }}
                        >
                          取消
                        </Button>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        </section>

        <section className="forge-settings-section space-y-2">
          <div className="forge-settings-heading">
            <Trash2 className="size-3.5 text-muted-foreground" />
            <h3 className="text-sm font-medium text-foreground">本机数据</h3>
          </div>
          <div className="forge-settings-danger-zone">
            <p className="text-xs leading-relaxed text-muted-foreground">
              清除这台电脑保存的对话列表，不会删除项目文件。
            </p>
            <Button
              size="sm"
              variant="destructive"
              onClick={handleClearAll}
              disabled={sessions.size === 0}
            >
              <Trash2 className="size-3.5" />
              {cleared ? "已清除" : `清除本机对话（${sessions.size}）`}
            </Button>
          </div>
        </section>

        {error && (
          <div className="flex items-center gap-1.5 text-xs text-destructive">
            <AlertCircle className="size-3" />
            {error}
          </div>
        )}
      </DialogContent>
    </Dialog>
  );
}

function SettingsSummaryItem({
  icon,
  label,
  value,
}: {
  icon: React.ReactNode;
  label: string;
  value: string;
}) {
  return (
    <div className="forge-settings-summary-item">
      <span className="forge-settings-summary-icon">{icon}</span>
      <span className="forge-settings-summary-copy">
        <span className="forge-settings-summary-label">{label}</span>
        <span className="forge-settings-summary-value">{value}</span>
      </span>
    </div>
  );
}
