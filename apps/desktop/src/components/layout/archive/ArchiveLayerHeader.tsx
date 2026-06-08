export function ArchiveLayerHeader({ title, meta }: { title: string; meta?: string | null }) {
  return (
    <div className="flex items-center justify-between pt-1">
      <h3 className="text-[11px] font-semibold text-foreground">{title}</h3>
      {meta && <span className="text-[10px] text-muted-foreground">{meta}</span>}
    </div>
  );
}
