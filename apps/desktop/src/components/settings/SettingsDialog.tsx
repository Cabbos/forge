import { useState, useEffect } from "react";
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
import { Settings, Key, Eye, EyeOff, Check, AlertCircle, Trash2 } from "lucide-react";
import { getApiKeyStatus, setApiKey, type KeyStatus } from "@/lib/tauri";
import { useStore } from "@/store";
import { formatContextWindow, PROVIDERS } from "@/lib/providers";

interface SettingsDialogProps {
  triggerClassName?: string;
}

export function SettingsDialog({ triggerClassName }: SettingsDialogProps = {}) {
  const [open, setOpen] = useState(false);
  const [keys, setKeys] = useState<KeyStatus[]>([]);
  const [editing, setEditing] = useState<string | null>(null);
  const [value, setValue] = useState("");
  const [visible, setVisible] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cleared, setCleared] = useState(false);
  const sessions = useStore((s) => s.sessions);
  const removeSession = useStore((s) => s.removeSession);

  const handleClearAll = async () => {
    // Remove all sessions from store + IndexedDB
    for (const [id] of sessions) {
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
    if (open) refresh();
  }, [open]);

  useEffect(() => {
    const openSettings = () => setOpen(true);
    const handleKeyDown = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key === ",") {
        event.preventDefault();
        setOpen(true);
      }
    };

    window.addEventListener("forge:open-settings", openSettings);
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("forge:open-settings", openSettings);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, []);

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

  const sortedKeys = [...keys].sort((a, b) => {
    const aIndex = PROVIDERS.findIndex((provider) => provider.id === a.provider);
    const bIndex = PROVIDERS.findIndex((provider) => provider.id === b.provider);
    return (aIndex < 0 ? 99 : aIndex) - (bIndex < 0 ? 99 : bIndex);
  });

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger
        render={<Button variant="ghost" size="icon-sm" aria-label="设置" title="设置" className={triggerClassName} />}
      >
        <Settings className="size-4" />
      </DialogTrigger>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Settings className="size-4" />
            设置
          </DialogTitle>
          <DialogDescription>
            管理模型服务和本机对话。密钥只保存在这台电脑。
          </DialogDescription>
        </DialogHeader>

        <section className="space-y-2">
          <div className="flex items-center gap-2">
            <Key className="size-3.5 text-muted-foreground" />
            <h3 className="text-sm font-medium text-foreground">模型服务</h3>
          </div>
          <div className="forge-surface overflow-hidden">
            {sortedKeys.map((k) => {
              const provider = PROVIDERS.find((item) => item.id === k.provider);
              const providerLabel = provider?.label ?? k.provider;
              const defaultModel = provider?.models.find((model) => model.id === provider.defaultModel);
              const defaultContext = formatContextWindow(defaultModel?.contextWindowTokens);

              return (
                <div key={k.provider} className="border-t border-border px-3 py-3 first:border-t-0">
                  <div className="flex items-center justify-between gap-3">
                    <div className="min-w-0">
                      <div className="truncate text-xs font-medium text-foreground">{providerLabel}</div>
                      <div className="mt-0.5 text-[11px] text-muted-foreground">
                        {k.set ? "已连接" : "需要添加密钥"}
                      </div>
                      {defaultModel && (
                        <>
                          <div className="mt-1 truncate text-[11px] text-muted-foreground/80">
                            {defaultModel.name}
                          </div>
                          <div className="mt-0.5 text-[10px] text-muted-foreground/60">
                            {["默认模型", defaultContext && `上下文 ${defaultContext}`].filter(Boolean).join(" · ")}
                          </div>
                        </>
                      )}
                    </div>
                    <span
                      className="shrink-0 rounded border border-border px-1.5 py-0.5 text-[10px] text-muted-foreground"
                      title={k.set ? k.preview : undefined}
                    >
                      {k.set ? "已配置" : "未配置"}
                    </span>
                  </div>

                  {editing === k.provider ? (
                    <div className="mt-2 space-y-2">
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
                  ) : (
                    <div className="mt-2 flex items-center justify-end gap-2">
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
              );
            })}
          </div>
        </section>

        <section className="space-y-2 border-t border-border/40 pt-3">
          <div className="flex items-center gap-2">
            <Trash2 className="size-3.5 text-muted-foreground" />
            <h3 className="text-sm font-medium text-foreground">本机数据</h3>
          </div>
          <p className="text-xs leading-relaxed text-muted-foreground">
            清除这台电脑保存的对话列表，不会删除项目文件。
          </p>
          <Button
            size="sm"
            variant="destructive"
            onClick={handleClearAll}
            disabled={sessions.size === 0}
            className="w-full"
          >
            <Trash2 className="size-3.5" />
            {cleared ? "已清除" : `清除本机对话（${sessions.size}）`}
          </Button>
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
