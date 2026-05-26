import { X } from "lucide-react";
import { commandIconMeta, fileReferenceIconMeta } from "@/lib/capability-icons";
import { ForgeIcon } from "@/components/ui/ForgeIcon";
import type { ComposerChip } from "./composerTypes";

interface ComposerChipTrayProps {
  chips: ComposerChip[];
  onRemove: (id: string) => void;
}

export function ComposerChipTray({ chips, onRemove }: ComposerChipTrayProps) {
  if (chips.length === 0) return null;

  return (
    <div className="forge-composer-chips">
      {chips.map((chip) => {
        const meta = chip.type === "file" ? fileReferenceIconMeta(chip.value) : commandIconMeta(chip.value);
        return (
          <span
            key={chip.id}
            className="forge-composer-chip"
            title={chip.value}
          >
            <ForgeIcon icon={meta.icon} tone={meta.tone} contained={false} className="size-3.5" />
            <span className="forge-composer-chip-label">{chip.value}</span>
            <button
              type="button"
              aria-label={`移除 ${chip.value}`}
              onClick={() => onRemove(chip.id)}
              className="forge-composer-chip-remove"
            >
              <X className="size-2.5" />
            </button>
          </span>
        );
      })}
    </div>
  );
}
