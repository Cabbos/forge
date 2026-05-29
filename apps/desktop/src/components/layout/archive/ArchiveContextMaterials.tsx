import { useState, type ReactNode } from "react";
import { FilePlus2, FileText } from "lucide-react";
import { cn } from "@/lib/utils";
import type { McpContextSelection } from "@/lib/tauri";
import { statusClass, statusLabel, type ContextFile } from "./contextMaterialMapper";

export function ContextFilesSection({
  files,
  onToggle,
}: {
  files: ContextFile[];
  onToggle: (selection: McpContextSelection) => void;
}) {
  return (
    <section>
      <div className="mb-2 flex items-center justify-end">
        <button
          type="button"
          disabled
          className="forge-action text-muted-foreground disabled:cursor-default disabled:opacity-70"
          title="添加文件"
        >
          <FilePlus2 className="size-3" />
          添加文件
        </button>
      </div>

      <div className="forge-surface overflow-hidden">
        <div className="grid grid-cols-[minmax(0,1fr)_42px_58px_52px] gap-2 border-b border-border px-3 py-2 text-[10px] uppercase tracking-wider text-muted-foreground">
          <span>文件名</span>
          <span>类型</span>
          <span>解析状态</span>
          <span className="text-right">参考</span>
        </div>

        {files.length === 0 ? (
          <div className="flex flex-col items-center justify-center gap-2 px-3 py-8 text-center">
            <FileText className="size-5 text-muted-foreground" />
            <div className="text-xs text-muted-foreground">还没有添加资料</div>
          </div>
        ) : (
          <div className="divide-y divide-border">
            {files.map((file) => (
              <ContextFileRow key={file.id} file={file} onToggle={onToggle} />
            ))}
          </div>
        )}
      </div>
    </section>
  );
}

function ContextFileRow({
  file,
  onToggle,
}: {
  file: ContextFile;
  onToggle: (selection: McpContextSelection) => void;
}) {
  const content = (
    <>
      <div className="min-w-0">
        <div className="truncate text-foreground">{file.name}</div>
        <div className="mt-0.5 truncate text-[10px] text-muted-foreground">
          {[file.sourceLabel, file.statusMessage].filter(Boolean).join(" · ")}
        </div>
      </div>
      <span className="truncate font-mono text-[10px] text-muted-foreground">{file.type}</span>
      <span className={cn("truncate text-[10px]", statusClass(file.status))}>
        {statusLabel(file.status)}
      </span>
      <span className={cn(
        "text-right text-[10px]",
        file.inContext ? "text-primary" : "text-muted-foreground",
      )}>
        {file.inContext ? "已加入" : "未加入"}
      </span>
    </>
  );

  if (file.selection) {
    const hasPromptArguments =
      file.selection.kind === "prompt" && (file.promptArguments?.length ?? 0) > 0;
    if (hasPromptArguments) {
      return (
        <ContextPromptRow
          content={content}
          file={file}
          onToggle={onToggle}
        />
      );
    }

    return (
      <button
        type="button"
        aria-pressed={file.inContext}
        onClick={() => onToggle(file.selection!)}
        className="grid w-full grid-cols-[minmax(0,1fr)_42px_58px_52px] items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-muted/25"
      >
        {content}
      </button>
    );
  }

  return (
    <div className="grid grid-cols-[minmax(0,1fr)_42px_58px_52px] items-center gap-2 px-3 py-2 text-xs">
      {content}
    </div>
  );
}

function ContextPromptRow({
  content,
  file,
  onToggle,
}: {
  content: ReactNode;
  file: ContextFile;
  onToggle: (selection: McpContextSelection) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [values, setValues] = useState<Record<string, string>>({});
  const argumentsList = file.promptArguments ?? [];

  const handleRowClick = () => {
    if (file.inContext) {
      onToggle(file.selection!);
      return;
    }
    setEditing((value) => !value);
  };

  const addPrompt = () => {
    if (!file.selection || file.selection.kind !== "prompt") return;
    onToggle({
      ...file.selection,
      arguments: values,
    });
    setEditing(false);
  };

  return (
    <div>
      <button
        type="button"
        aria-pressed={file.inContext}
        onClick={handleRowClick}
        className="grid w-full grid-cols-[minmax(0,1fr)_42px_58px_52px] items-center gap-2 px-3 py-2 text-left text-xs transition-colors hover:bg-muted/25"
      >
        {content}
      </button>
      {editing && !file.inContext ? (
        <div className="space-y-2 border-t border-border/70 px-3 py-2.5">
          {argumentsList.map((argument) => (
            <label key={argument.name} className="block space-y-1">
              <span className="text-[10px] text-muted-foreground">
                {argument.name}{argument.required ? " *" : ""}
              </span>
              <input
                aria-label={argument.name}
                value={values[argument.name] ?? ""}
                onChange={(event) => setValues((current) => ({
                  ...current,
                  [argument.name]: event.target.value,
                }))}
                placeholder={argument.description || argument.name}
                className="h-7 w-full rounded-md border border-border bg-muted/20 px-2 text-xs text-foreground outline-none transition-colors placeholder:text-muted-foreground/65 focus:border-primary/45"
              />
            </label>
          ))}
          <div className="flex justify-end">
            <button
              type="button"
              onClick={addPrompt}
              className="forge-action"
            >
              加入本轮
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
