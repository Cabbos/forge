import type { ForgeWikiState } from "@/lib/protocol";
import { ForgeActionButton } from "@/components/primitives/action";
import { ForgeSurface } from "@/components/primitives/surface";
import { EmptyState, SectionHeader } from "./WikiSectionChrome";
import { FORGE_WIKI_INIT_OPERATION_ID } from "./WikiSectionTypes";

export function ProjectRecordsSection({
  currentProjectPath,
  forgeWikiState,
  loading,
  busyId,
  onRefresh,
  onInitForgeWiki,
}: {
  currentProjectPath: string;
  forgeWikiState: ForgeWikiState | null;
  loading: boolean;
  busyId: string | null;
  onRefresh: () => void;
  onInitForgeWiki: () => void;
}) {
  return (
    <section>
      <SectionHeader
        title="项目记录"
        meta={forgeWikiState?.exists ? `${forgeWikiState.pages.length} 页` : null}
        loading={loading}
        onRefresh={onRefresh}
        refreshDisabled={loading}
      />
      <ForgeSurface className="overflow-hidden">
        {!currentProjectPath ? (
          <EmptyState label="打开项目后可以建立项目记录" />
        ) : !forgeWikiState?.exists ? (
          <div className="space-y-3 px-3 py-5 text-center">
            <EmptyState label="还没有项目记录" compact />
            <ForgeActionButton
              onClick={onInitForgeWiki}
              disabled={busyId === FORGE_WIKI_INIT_OPERATION_ID}
              className="h-8 text-xs focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-primary/60 disabled:cursor-default disabled:opacity-50"
            >
              建立项目记录
            </ForgeActionButton>
          </div>
        ) : forgeWikiState.pages.length === 0 ? (
          <EmptyState label="还没有项目记录" />
        ) : (
          <div className="divide-y divide-border">
            {forgeWikiState.pages.map((page) => (
              <ForgeWikiPageRow key={page.id} page={page} />
            ))}
          </div>
        )}
      </ForgeSurface>
    </section>
  );
}

function ForgeWikiPageRow({ page }: { page: ForgeWikiState["pages"][number] }) {
  return (
    <div className="px-3 py-2.5">
      <div className="min-w-0">
        <div className="truncate text-xs font-medium text-foreground">{page.title}</div>
        <div className="mt-1 truncate font-mono text-[10px] text-muted-foreground/70">{page.path}</div>
        {page.summary && (
          <div className="mt-1 max-h-[3.8rem] overflow-hidden break-words text-[11px] leading-relaxed text-muted-foreground">
            {page.summary}
          </div>
        )}
      </div>
    </div>
  );
}
