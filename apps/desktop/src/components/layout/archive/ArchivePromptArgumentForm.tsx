import { useState, type ReactNode } from "react";
import { Button as ButtonPrimitive } from "@base-ui/react/button";
import { ForgeActionButton } from "@/components/primitives/action";
import { ForgeTextInput } from "@/components/primitives/input";
import { cn } from "@/lib/utils";
import type { McpContextSelection } from "@/lib/tauri";
import type { ContextFile } from "./contextMaterialMapper";

interface ArchivePromptArgumentFormProps {
  file: ContextFile;
  onToggle: (selection: McpContextSelection) => void;
  onDone: () => void;
}

export function ArchivePromptArgumentForm({ file, onToggle, onDone }: ArchivePromptArgumentFormProps) {
  const [values, setValues] = useState<Record<string, string>>({});
  const argumentsList = file.promptArguments ?? [];

  const addPrompt = () => {
    if (!file.selection || file.selection.kind !== "prompt") return;
    onToggle({
      ...file.selection,
      arguments: values,
    });
    onDone();
  };

  return (
    <div className="space-y-2 border-t border-border/70 px-3 py-2.5">
      {argumentsList.map((argument) => (
        <label key={argument.name} className="block space-y-1">
          <span className="text-[10px] text-muted-foreground">
            {argument.name}{argument.required ? " *" : ""}
          </span>
          <ForgeTextInput
            aria-label={argument.name}
            value={values[argument.name] ?? ""}
            onChange={(event) => setValues((current) => ({
              ...current,
              [argument.name]: event.target.value,
            }))}
            placeholder={argument.description || argument.name}
            className="h-7 w-full bg-muted/20 text-xs placeholder:text-muted-foreground/65 focus:border-primary/45"
          />
        </label>
      ))}
      <div className="flex justify-end">
        <ForgeActionButton onClick={addPrompt}>
          加入本轮
        </ForgeActionButton>
      </div>
    </div>
  );
}

interface ArchivePromptMaterialRowProps {
  content: ReactNode;
  file: ContextFile;
  rowClassName: string;
  onToggle: (selection: McpContextSelection) => void;
}

export function ArchivePromptMaterialRow({
  content,
  file,
  rowClassName,
  onToggle,
}: ArchivePromptMaterialRowProps) {
  const [editing, setEditing] = useState(false);

  const handleRowClick = () => {
    if (file.inContext) {
      onToggle(file.selection!);
      return;
    }
    setEditing((value) => !value);
  };

  return (
    <div>
      <ButtonPrimitive
        type="button"
        aria-pressed={file.inContext}
        onClick={handleRowClick}
        className={cn(rowClassName, "transition-colors hover:bg-muted/25")}
      >
        {content}
      </ButtonPrimitive>
      {editing && !file.inContext ? (
        <ArchivePromptArgumentForm
          file={file}
          onToggle={onToggle}
          onDone={() => setEditing(false)}
        />
      ) : null}
    </div>
  );
}
