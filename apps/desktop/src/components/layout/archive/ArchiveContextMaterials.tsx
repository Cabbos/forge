import { FilePlus2, FileText } from "lucide-react";
import { ForgeActionButton } from "@/components/primitives/action";
import { ForgeSurface } from "@/components/primitives/surface";
import type { McpContextSelection } from "@/lib/tauri";
import { ContextMaterialRows } from "./ArchiveContextMaterialRows";
import type { ContextFile } from "./contextMaterialMapper";

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
        <ForgeActionButton
          disabled
          className="text-muted-foreground disabled:cursor-default disabled:opacity-70"
          title="添加文件"
        >
          <FilePlus2 className="size-3" />
          添加文件
        </ForgeActionButton>
      </div>

      <ForgeSurface className="overflow-hidden">
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
          <ContextMaterialRows files={files} onToggle={onToggle} />
        )}
      </ForgeSurface>
    </section>
  );
}
