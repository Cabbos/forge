use std::path::Path;
use std::sync::Arc;

use crate::harness::capability::CapabilityKind;
use crate::harness::registry::CapabilityEntry;
use crate::harness::skills::SkillLoader;
use crate::state::AppState;
use serde::Serialize;

#[derive(Serialize)]
pub struct CapabilityInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub source: String,
    pub version: String,
    pub enabled: bool,
}

#[tauri::command]
pub async fn list_capabilities(
    state: tauri::State<'_, Arc<AppState>>,
) -> Result<Vec<CapabilityInfo>, String> {
    // 1. Get registry capabilities (tools, hooks, mcp)
    let mut result: Vec<CapabilityInfo> =
        global_registry_capability_infos(state.harness.capability_registry.all_entries());

    // 2. Get user-level skills. Project-level skills are session context, not global settings.
    let skill_loader = SkillLoader::new();
    skill_loader.attach_database(state.harness.database.clone());
    let skills = skill_loader.scan_all().await;
    for s in skills {
        if !result.iter().any(|c| c.id == s.id) {
            result.push(CapabilityInfo {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                kind: "skill".into(),
                source: "local".into(),
                version: "1.0.0".into(),
                enabled: s.enabled,
            });
        }
    }
    Ok(result)
}

fn capability_kind_label(kind: &CapabilityKind) -> &'static str {
    match kind {
        CapabilityKind::Skill => "skill",
        CapabilityKind::Hook => "hook",
        CapabilityKind::McpServer => "mcp_server",
        CapabilityKind::Tool => "tool",
    }
}

fn global_registry_capability_infos(entries: Vec<CapabilityEntry>) -> Vec<CapabilityInfo> {
    entries
        .into_iter()
        .filter(|entry| !is_hidden_global_capability(entry))
        .map(|entry| {
            let m = entry.metadata;
            CapabilityInfo {
                id: m.id,
                name: m.name,
                description: m.description,
                kind: capability_kind_label(&m.kind).to_string(),
                source: m.source,
                version: m.version,
                enabled: entry.enabled,
            }
        })
        .collect()
}

fn is_hidden_global_capability(entry: &CapabilityEntry) -> bool {
    is_internal_infrastructure_capability(entry) || is_workspace_scoped_capability(entry)
}

fn is_internal_infrastructure_capability(entry: &CapabilityEntry) -> bool {
    entry.metadata.id == "skill-loader"
}

fn is_workspace_scoped_capability(entry: &CapabilityEntry) -> bool {
    if !matches!(entry.metadata.kind, CapabilityKind::McpServer) {
        return false;
    }

    let source = entry.metadata.source.replace('\\', "/");
    source == ".forge/mcp.json" || source.ends_with("/.forge/mcp.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability_kind_labels_match_frontend_contract() {
        assert_eq!(capability_kind_label(&CapabilityKind::Skill), "skill");
        assert_eq!(capability_kind_label(&CapabilityKind::Hook), "hook");
        assert_eq!(
            capability_kind_label(&CapabilityKind::McpServer),
            "mcp_server"
        );
        assert_eq!(capability_kind_label(&CapabilityKind::Tool), "tool");
    }

    #[test]
    fn global_capability_list_omits_workspace_scoped_mcp_servers() {
        let entries = vec![
            capability_entry(
                "read_file",
                "File Reader",
                CapabilityKind::Tool,
                "builtin",
                true,
            ),
            capability_entry(
                "mcp:obsidian",
                "Obsidian",
                CapabilityKind::McpServer,
                "/tmp/demo/.forge/mcp.json",
                true,
            ),
        ];

        let infos = global_registry_capability_infos(entries);

        assert!(infos.iter().any(|info| info.id == "read_file"));
        assert!(!infos.iter().any(|info| info.id == "mcp:obsidian"));
    }

    #[test]
    fn global_capability_list_omits_internal_infrastructure_capabilities() {
        let entries = vec![
            capability_entry(
                "skill-loader",
                "Skill Loader",
                CapabilityKind::Skill,
                "builtin",
                true,
            ),
            capability_entry(
                "hook:workspace-boundary",
                "Workspace Boundary Guard",
                CapabilityKind::Hook,
                "builtin",
                true,
            ),
        ];

        let infos = global_registry_capability_infos(entries);

        assert!(!infos.iter().any(|info| info.id == "skill-loader"));
        assert!(infos
            .iter()
            .any(|info| info.id == "hook:workspace-boundary"));
    }

    fn capability_entry(
        id: &str,
        name: &str,
        kind: CapabilityKind,
        source: &str,
        enabled: bool,
    ) -> crate::harness::registry::CapabilityEntry {
        crate::harness::registry::CapabilityEntry {
            metadata: crate::harness::capability::CapabilityMetadata {
                id: id.to_string(),
                name: name.to_string(),
                description: format!("{name} description"),
                version: "1.0.0".to_string(),
                source: source.to_string(),
                kind,
            },
            enabled,
        }
    }
}

#[tauri::command]
pub async fn toggle_capability(
    state: tauri::State<'_, Arc<AppState>>,
    capability_id: String,
    enabled: bool,
) -> Result<(), String> {
    // Try registry first, then skill loader
    match state
        .harness
        .capability_registry
        .toggle_with_event(&capability_id, enabled)
        .await
    {
        Ok(_) => {
            let sessions = state
                .sessions
                .read()
                .await
                .values()
                .cloned()
                .collect::<Vec<_>>();
            for session in sessions {
                let _ = session
                    .harness
                    .capability_registry
                    .toggle_with_event(&capability_id, enabled)
                    .await;
            }
            Ok(())
        }
        Err(reg_err) => {
            let skill_loader = SkillLoader::new();
            skill_loader.attach_database(state.harness.database.clone());
            let _ = skill_loader.scan_all().await;
            skill_loader.toggle(&capability_id, enabled).await;
            let skills = skill_loader.all_skills().await;
            if skills.iter().any(|s| s.id == capability_id) {
                let sessions = state
                    .sessions
                    .read()
                    .await
                    .values()
                    .cloned()
                    .collect::<Vec<_>>();
                for session in sessions {
                    let _ = session.harness.skill_loader.scan_all().await;
                    session
                        .harness
                        .skill_loader
                        .toggle(&capability_id, enabled)
                        .await;
                }
                Ok(())
            } else {
                Err(format!(
                    "Capability not found in registry or skills: {} ({})",
                    capability_id, reg_err
                ))
            }
        }
    }
}

/// Allowed Git hosts for skill installation.
const ALLOWED_HOSTS: &[&str] = &["github.com", "gitlab.com", "bitbucket.org"];

fn validate_skill_url(repo: &str) -> Result<String, String> {
    if repo.trim().is_empty() {
        return Err("Repository cannot be empty".into());
    }

    // If it's a full URL, validate the host
    if repo.starts_with("http://") || repo.starts_with("https://") {
        // Extract host from URL with simple string parsing
        let without_scheme = if let Some(s) = repo.strip_prefix("https://") {
            s
        } else if repo.strip_prefix("http://").is_some() {
            return Err("Only HTTPS URLs are allowed".into());
        } else {
            return Err("Invalid URL scheme".into());
        };
        let host = without_scheme.split('/').next().unwrap_or("");
        if host.is_empty() {
            return Err("Invalid URL: missing host".into());
        }
        let host_root = host.rsplitn(2, '.').last().unwrap_or(host);
        if !ALLOWED_HOSTS
            .iter()
            .any(|allowed| host_root == *allowed || host == *allowed)
        {
            return Err(format!(
                "Untrusted Git host: {}. Allowed: {:?}",
                host, ALLOWED_HOSTS
            ));
        }
        return Ok(repo.to_string());
    }

    // Otherwise treat as owner/repo shorthand (GitHub default)
    if !repo.contains('/') {
        return Err("Repository must be in owner/repo format".into());
    }
    let cleaned = repo.trim_end_matches(".git");
    // Basic sanity: no path traversal, no shell metacharacters
    if cleaned.contains("..") || cleaned.contains('$') || cleaned.contains('`') {
        return Err("Invalid repository name".into());
    }
    Ok(format!("https://github.com/{}.git", cleaned))
}

fn read_skill_metadata(dir: &Path) -> Result<(String, String), String> {
    let skill_md = dir.join("SKILL.md");
    let claude_md = dir.join("CLAUDE.md");
    let md_path = if skill_md.exists() {
        &skill_md
    } else if claude_md.exists() {
        &claude_md
    } else {
        return Err(format!(
            "No SKILL.md or CLAUDE.md found in {}",
            dir.display()
        ));
    };

    let content = std::fs::read_to_string(md_path).map_err(|e| e.to_string())?;
    let desc = content
        .lines()
        .find(|l| l.starts_with("description:"))
        .map(|l| l.trim_start_matches("description:").trim().to_string())
        .unwrap_or_default();
    Ok((desc, content))
}

#[tauri::command]
pub async fn install_skill(
    state: tauri::State<'_, Arc<AppState>>,
    repo: String,
) -> Result<CapabilityInfo, String> {
    let url = validate_skill_url(&repo)?;
    let name = repo
        .split('/')
        .next_back()
        .unwrap_or(&repo)
        .replace(".git", "");
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let skills_dir = std::path::PathBuf::from(home).join(".forge").join("skills");
    let dest = skills_dir.join(&name);

    if dest.exists() {
        let (desc, _) = read_skill_metadata(&dest)?;
        let _ = state.harness.skill_loader.scan_all().await;
        return Ok(CapabilityInfo {
            id: name.clone(),
            name: name.clone(),
            description: desc,
            kind: "skill".into(),
            source: repo,
            version: "1.0.0".into(),
            enabled: true,
        });
    }

    std::fs::create_dir_all(&skills_dir).map_err(|e| e.to_string())?;

    let dest_str = dest.to_string_lossy();
    let output = std::process::Command::new("git")
        .args(["clone", "--depth=1", &url, &*dest_str])
        .output()
        .map_err(|e| format!("Git failed: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "Git clone failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // Re-scan and read metadata
    let _ = state.harness.skill_loader.scan_all().await;
    let (desc, _) = read_skill_metadata(&dest).unwrap_or_default();

    Ok(CapabilityInfo {
        id: name.clone(),
        name: name.clone(),
        description: desc,
        kind: "skill".into(),
        source: repo,
        version: "1.0.0".into(),
        enabled: true,
    })
}
