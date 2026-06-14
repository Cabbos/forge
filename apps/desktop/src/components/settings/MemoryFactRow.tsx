import { useState, useCallback } from "react";
import { AlertCircle, Loader2, Pencil, Trash2 } from "lucide-react";
import { type MemoryFact } from "@/lib/tauri";
import { formatMutationError, formatTimestamp } from "./settingsUtils";
import { MemoryInlineEditor } from "./MemoryInlineEditor";
import { Button as ButtonPrimitive } from "@base-ui/react/button";

export function MemoryFactRow({
  fact,
  onDelete,
  onUpdate,
}: {
  fact: MemoryFact;
  onDelete: (id: string) => Promise<void> | void;
  onUpdate: (id: string, text: string, tags: string[]) => Promise<void> | void;
}) {
  const [editing, setEditing] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const handleDelete = useCallback(async () => {
    setDeleteError(null);
    setDeleting(true);
    try {
      await onDelete(fact.id);
    } catch (error) {
      setDeleteError(formatMutationError(error));
    } finally {
      setDeleting(false);
    }
  }, [fact.id, onDelete]);

  if (editing) {
    return (
      <div className="forge-memory-fact" data-editing="true">
        <MemoryInlineEditor
          initial={fact}
          onSave={async (text, tags) => {
            await onUpdate(fact.id, text, tags);
            setEditing(false);
          }}
          onCancel={() => setEditing(false)}
        />
      </div>
    );
  }

  return (
    <div className="forge-memory-fact">
      <div className="forge-memory-fact-body">
        <p className="forge-memory-fact-text">{fact.text}</p>
        <div className="forge-memory-fact-meta">
          {fact.tags.length > 0 && (
            <span className="forge-memory-fact-tags">
              {fact.tags.map((t) => (
                <span key={t} className="forge-memory-tag">{t}</span>
              ))}
            </span>
          )}
          <span className="forge-memory-fact-time">
            {formatTimestamp(fact.updated_at_ms)}
          </span>
          {fact.source && (
            <span className="forge-memory-fact-source">{fact.source}</span>
          )}
        </div>
      </div>
      <div className="forge-memory-fact-actions">
        <ButtonPrimitive
          type="button"
          className="forge-memory-action-btn"
          onClick={() => setEditing(true)}
          aria-label="编辑"
        >
          <Pencil className="size-3" />
        </ButtonPrimitive>
        <ButtonPrimitive
          type="button"
          className="forge-memory-action-btn forge-memory-action-btn--danger"
          onClick={handleDelete}
          disabled={deleting}
          aria-label="删除"
        >
          {deleting ? (
            <Loader2 className="size-3 animate-spin" />
          ) : (
            <Trash2 className="size-3" />
          )}
        </ButtonPrimitive>
      </div>
      {deleteError && (
        <div className="forge-memory-row-error" role="alert">
          <AlertCircle className="size-3" />
          <span>{deleteError}</span>
        </div>
      )}
    </div>
  );
}
