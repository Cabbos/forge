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
import { PROVIDERS } from "@/lib/providers";

export function SettingsDialog() {
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
        render={<Button variant="ghost" size="icon-sm" />}
      >
        <Settings className="size-4" />
        <span className="sr-only">设置</span>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <Key className="size-4" />
            模型密钥
          </DialogTitle>
          <DialogDescription>
            管理 Anthropic、DeepSeek、OpenAI 和 OpenRouter 的 API Key。
            密钥保存在 <code className="mx-1 bg-muted px-1 py-0.5 rounded text-xs">~/.tui-to-gui/config.json</code>。
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-3">
          {sortedKeys.map((k) => {
            const provider = PROVIDERS.find((item) => item.id === k.provider);
            return (
            <div key={k.provider} className="space-y-1.5">
              <label className="text-xs font-medium text-muted-foreground">
                {provider?.label ?? k.provider}
              </label>
              {editing === k.provider ? (
                <div className="space-y-2">
                  <div className="relative">
                    <Input
                      type={visible ? "text" : "password"}
                      value={value}
                      onChange={(e) => setValue(e.target.value)}
                      placeholder={provider?.keyPlaceholder ?? "sk-..."}
                      className="h-8 text-xs pr-16"
                      autoFocus
                    />
                    <button
                      onClick={() => setVisible(!visible)}
                      className="absolute right-8 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
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
                <div className="flex items-center gap-2">
                  {k.set ? (
                    <span className="text-xs font-mono text-muted-foreground bg-muted px-2 py-1 rounded flex-1">
                      {k.preview}
                    </span>
                  ) : (
                    <span className="text-xs text-muted-foreground italic flex-1">
                      未配置
                    </span>
                  )}
                  <Button
                    size="xs"
                    variant="outline"
                    onClick={() => {
                      setEditing(k.provider);
                      setValue("");
                      setError(null);
                    }}
                  >
                    {k.set ? "编辑" : "设置"}
                  </Button>
                  {k.set && (
                    <Button
                      size="xs"
                      variant="ghost"
                      onClick={() => handleRemove(k.provider)}
                      className="text-destructive hover:text-destructive"
                    >
                      清除
                    </Button>
                  )}
                </div>
              )}
            </div>
            );
          })}
        </div>

        <div className="pt-3 border-t border-border/40">
          <p className="text-xs text-muted-foreground mb-2">
            清除本机保存的任务记录（IndexedDB）。
          </p>
          <Button
            size="sm"
            variant="destructive"
            onClick={handleClearAll}
            disabled={sessions.size === 0}
            className="w-full"
          >
            <Trash2 className="size-3.5" />
            {cleared ? "已清除" : `清除任务记录（${sessions.size}）`}
          </Button>
        </div>

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
