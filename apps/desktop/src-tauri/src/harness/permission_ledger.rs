use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::Path;

use crate::harness::permissions::PermissionMode;
use crate::harness::write_boundary::{WriteBoundary, WriteBoundaryRisk};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionLedgerEventKind {
    ModeChanged,
    ManualRequired,
    AutoApproved,
    BlockedExternalPath,
    BlockedSensitivePath,
    BlockedPolicy,
    UserApproved,
    UserDeclined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRiskTier {
    Normal,
    Caution,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionLedgerEvent {
    pub kind: PermissionLedgerEventKind,
    pub workspace_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    pub risk_tier: PermissionRiskTier,
    pub affected_files: Vec<String>,
    pub operation: String,
    pub permission_mode: PermissionMode,
    pub reason: String,
}

impl PermissionLedgerEvent {
    pub fn mode_changed(
        session_id: &str,
        workspace: Option<&Path>,
        mode: PermissionMode,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            kind: PermissionLedgerEventKind::ModeChanged,
            workspace_path: workspace_path(workspace),
            session_id: Some(session_id.to_string()),
            risk_tier: PermissionRiskTier::Normal,
            affected_files: Vec::new(),
            operation: "set_permission_mode".to_string(),
            permission_mode: mode,
            reason: reason.into(),
        }
    }

    pub fn decision(
        kind: PermissionLedgerEventKind,
        session_id: &str,
        tool: &str,
        input: &Value,
        working_dir: &Path,
        permission_mode: PermissionMode,
        reason: impl Into<String>,
    ) -> Self {
        let tool = canonical_tool_name(tool);
        let affected_files = affected_files_for(tool, input);
        let risk_tier = match kind {
            PermissionLedgerEventKind::BlockedExternalPath
            | PermissionLedgerEventKind::BlockedSensitivePath
            | PermissionLedgerEventKind::BlockedPolicy => PermissionRiskTier::High,
            PermissionLedgerEventKind::ManualRequired => PermissionRiskTier::Caution,
            _ => PermissionRiskTier::Normal,
        };
        Self {
            kind,
            workspace_path: workspace_path(Some(working_dir)),
            session_id: Some(session_id.to_string()),
            risk_tier,
            affected_files,
            operation: tool.to_string(),
            permission_mode,
            reason: reason.into(),
        }
    }

    pub fn user_response(
        session_id: &str,
        approved: bool,
        prior: Option<&PermissionLedgerEvent>,
        boundary: Option<&WriteBoundary>,
    ) -> Self {
        Self::user_response_with_workspace(session_id, approved, prior, boundary, None)
    }

    pub fn user_response_with_workspace(
        session_id: &str,
        approved: bool,
        prior: Option<&PermissionLedgerEvent>,
        boundary: Option<&WriteBoundary>,
        workspace: Option<&Path>,
    ) -> Self {
        let mut event = prior.cloned().unwrap_or_else(|| Self {
            kind: PermissionLedgerEventKind::ManualRequired,
            workspace_path: boundary
                .map(|boundary| boundary.workspace_path.clone())
                .unwrap_or_else(|| workspace_path(workspace)),
            session_id: Some(session_id.to_string()),
            risk_tier: boundary
                .map(|boundary| PermissionRiskTier::from(boundary.risk))
                .unwrap_or(PermissionRiskTier::Caution),
            affected_files: boundary
                .map(|boundary| boundary.affected_files.clone())
                .unwrap_or_default(),
            operation: boundary
                .map(|boundary| boundary.operation.clone())
                .unwrap_or_else(|| "confirm".to_string()),
            permission_mode: PermissionMode::ManualConfirm,
            reason: "manual_confirm_requires_user_response".to_string(),
        });
        event.kind = if approved {
            PermissionLedgerEventKind::UserApproved
        } else {
            PermissionLedgerEventKind::UserDeclined
        };
        event.session_id = Some(session_id.to_string());
        event.reason = "user_response".to_string();
        if let Some(boundary) = boundary {
            event.apply_boundary(boundary);
        }
        event
    }

    pub fn apply_boundary(&mut self, boundary: &WriteBoundary) {
        self.workspace_path = boundary.workspace_path.clone();
        self.risk_tier = PermissionRiskTier::from(boundary.risk);
        self.affected_files = boundary.affected_files.clone();
        self.operation = boundary.operation.clone();
    }
}

impl From<WriteBoundaryRisk> for PermissionRiskTier {
    fn from(value: WriteBoundaryRisk) -> Self {
        match value {
            WriteBoundaryRisk::Normal => PermissionRiskTier::Normal,
            WriteBoundaryRisk::Caution => PermissionRiskTier::Caution,
            WriteBoundaryRisk::High => PermissionRiskTier::High,
        }
    }
}

fn workspace_path(workspace: Option<&Path>) -> String {
    workspace
        .map(|path| path.canonicalize().unwrap_or_else(|_| path.to_path_buf()))
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn canonical_tool_name(tool_name: &str) -> &str {
    match tool_name {
        "read" => "read_file",
        "write" | "write_file" => "write_to_file",
        "edit" => "edit_file",
        "ls" | "list" => "list_directory",
        "glob" => "search_files",
        "grep" => "search_content",
        "bash" | "execute_command" | "shell" | "shell_command" | "run_command"
        | "run_shell_command" => "run_shell",
        other => other,
    }
}

fn affected_files_for(tool_name: &str, input: &Value) -> Vec<String> {
    match tool_name {
        "write_to_file" | "edit_file" => input
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| !path.is_empty())
            .map(|path| vec![path.to_string()])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}
