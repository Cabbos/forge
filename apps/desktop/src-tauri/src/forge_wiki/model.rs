use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeWikiPageKind {
    Index,
    Schema,
    Sources,
    Decisions,
    Tasks,
    Log,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeWikiPage {
    pub id: String,
    pub project_path: String,
    pub path: String,
    pub title: String,
    pub kind: ForgeWikiPageKind,
    pub summary: Option<String>,
    pub updated_at: Option<String>,
    pub token_estimate: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeWikiState {
    pub project_path: String,
    pub exists: bool,
    pub wiki_dir: String,
    pub pages: Vec<ForgeWikiPage>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectedForgeWikiPage {
    pub page_id: String,
    pub title: String,
    pub path: String,
    pub kind: ForgeWikiPageKind,
    pub summary: String,
    pub score: f32,
    pub reason: String,
    pub injected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForgeWikiProposalStatus {
    Pending,
    Accepted,
    Discarded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForgeWikiUpdateProposal {
    pub id: String,
    pub project_path: String,
    pub session_id: Option<String>,
    pub target_pages: Vec<String>,
    pub title: String,
    pub summary: String,
    pub patch_preview: Option<String>,
    pub status: ForgeWikiProposalStatus,
    pub created_at: String,
}
