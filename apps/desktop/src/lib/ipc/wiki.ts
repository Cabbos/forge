import { invoke } from "@tauri-apps/api/core";
import { hasTauriRuntime } from "./core";
import type {
  ForgeWikiPage,
  ForgeWikiState,
  ForgeWikiUpdateProposal,
  SelectedForgeWikiPage,
} from "./types";
import { getRememberedWorkingDir } from "./app";

export async function getForgeWikiState(
  projectPath: string,
  sessionId?: string | null,
): Promise<ForgeWikiState> {
  if (!hasTauriRuntime()) return fallbackForgeWikiState(projectPath);
  return invoke("get_forge_wiki_state", { projectPath, sessionId: sessionId ?? null });
}

export async function initForgeWiki(
  projectPath: string,
  sessionId?: string | null,
): Promise<ForgeWikiState> {
  if (!hasTauriRuntime()) return fallbackForgeWikiState(projectPath);
  return invoke("init_forge_wiki", { projectPath, sessionId: sessionId ?? null });
}

export async function listForgeWikiPages(
  projectPath: string,
  sessionId?: string | null,
): Promise<ForgeWikiPage[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("list_forge_wiki_pages", { projectPath, sessionId: sessionId ?? null });
}

export async function readForgeWikiPage(
  projectPath: string,
  pagePath: string,
  sessionId?: string | null,
): Promise<string> {
  if (!hasTauriRuntime()) return "";
  return invoke("read_forge_wiki_page", { projectPath, pagePath, sessionId: sessionId ?? null });
}

export async function selectForgeWikiContext(
  projectPath: string,
  message: string,
  sessionId?: string | null,
): Promise<SelectedForgeWikiPage[]> {
  if (!hasTauriRuntime()) return [];
  return invoke("select_forge_wiki_context", {
    projectPath,
    message,
    sessionId: sessionId ?? null,
  });
}

export async function createForgeWikiUpdateProposal(
  projectPath: string,
  sessionId: string | null,
  targetPages: string[],
  title: string,
  summary: string,
): Promise<ForgeWikiUpdateProposal> {
  return invoke("create_forge_wiki_update_proposal", {
    projectPath,
    sessionId,
    targetPages,
    title,
    summary,
  });
}

export async function acceptForgeWikiUpdateProposal(
  projectPath: string,
  proposalId: string,
  sessionId?: string | null,
): Promise<ForgeWikiUpdateProposal> {
  return invoke("accept_forge_wiki_update_proposal", {
    projectPath,
    proposalId,
    sessionId: sessionId ?? null,
  });
}

export async function discardForgeWikiUpdateProposal(
  projectPath: string,
  proposalId: string,
  sessionId?: string | null,
): Promise<ForgeWikiUpdateProposal> {
  return invoke("discard_forge_wiki_update_proposal", {
    projectPath,
    proposalId,
    sessionId: sessionId ?? null,
  });
}

function fallbackForgeWikiState(projectPath: string): ForgeWikiState {
  const normalizedProjectPath = projectPath || getRememberedWorkingDir() || "";
  return {
    project_path: normalizedProjectPath,
    exists: false,
    wiki_dir: normalizedProjectPath
      ? joinPath(normalizedProjectPath, ".forge", "wiki")
      : "",
    pages: [],
    message: "项目记录在浏览器预览中不可用。",
  };
}

function joinPath(...parts: string[]): string {
  return parts
    .map((part, index) => {
      if (index === 0) return part.replace(/\/+$/, "");
      return part.replace(/^\/+|\/+$/g, "");
    })
    .filter(Boolean)
    .join("/");
}
