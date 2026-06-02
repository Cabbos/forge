import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { cn } from "@/lib/utils";
import type { McpContextSelection } from "@/lib/tauri";
import { ArchivePromptMaterialRow } from "./ArchivePromptArgumentForm";
import { statusClass, statusLabel, type ContextFile } from "./contextMaterialMapper";

interface ContextMaterialRowsProps {
  files: ContextFile[];
  onToggle: (selection: McpContextSelection) => void;
}

const materialRowClass =
  "grid w-full grid-cols-[minmax(0,1fr)_42px_58px_52px] items-center gap-2 px-3 py-2 text-left text-xs";

export function ContextMaterialRows({ files, onToggle }: ContextMaterialRowsProps) {
  return (
    <div className="divide-y divide-border">
      {files.map((file) => (
        <ContextFileRow key={file.id} file={file} onToggle={onToggle} />
      ))}
    </div>
  );
}

function ContextFileRow({
  file,
  onToggle,
}: {
  file: ContextFile;
  onToggle: (selection: McpContextSelection) => void;
}) {
  const content = <ContextFileRowCells file={file} />;

  if (file.selection) {
    const hasPromptArguments =
      file.selection.kind === "prompt" && (file.promptArguments?.length ?? 0) > 0;
    if (hasPromptArguments) {
      return (
        <ArchivePromptMaterialRow
          content={content}
          file={file}
          rowClassName={materialRowClass}
          onToggle={onToggle}
        />
      );
    }

    return (
      <ButtonPrimitive
        type="button"
        aria-pressed={file.inContext}
        onClick={() => onToggle(file.selection!)}
        className={cn(materialRowClass, "transition-colors hover:bg-muted/25")}
      >
        {content}
      </ButtonPrimitive>
    );
  }

  return (
    <div className={materialRowClass}>
      {content}
    </div>
  );
}

function ContextFileRowCells({ file }: { file: ContextFile }) {
  return (
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
}
