use std::sync::Arc;
use std::path::Path;

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
    // Always refresh skills before listing
    let _ = state.harness.skill_loader.scan_all().await;

    // 1. Get registry capabilities (tools, hooks, mcp)
    let mut result: Vec<CapabilityInfo> = state.harness.capability_registry.all()
        .iter()
        .map(|m| CapabilityInfo {
            id: m.id.clone(), name: m.name.clone(), description: m.description.clone(),
            kind: format!("{:?}", m.kind).to_lowercase(), source: m.source.clone(),
            version: m.version.clone(), enabled: true,
        }).collect();

    // 2. Get discovered skills from SkillLoader
    let skills = state.harness.skill_loader.enabled_skills().await;
    for s in skills {
        if !result.iter().any(|c| c.id == s.id) {
            result.push(CapabilityInfo {
                id: s.id.clone(), name: s.name.clone(), description: s.description.clone(),
                kind: "skill".into(), source: "local".into(),
                version: "1.0.0".into(), enabled: s.enabled,
            });
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn toggle_capability(
    state: tauri::State<'_, Arc<AppState>>,
    capability_id: String,
    enabled: bool,
) -> Result<(), String> {
    // Try registry first, then skill loader
    match state.harness.capability_registry.toggle(&capability_id, enabled) {
        Ok(()) => Ok(()),
        Err(reg_err) => {
            state.harness.skill_loader.toggle(&capability_id, enabled).await;
            // Verify the toggle actually found the skill
            let skills = state.harness.skill_loader.enabled_skills().await;
            if skills.iter().any(|s| s.id == capability_id) {
                Ok(())
            } else {
                Err(format!("Capability not found in registry or skills: {} ({})", capability_id, reg_err))
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
        if !ALLOWED_HOSTS.iter().any(|allowed| host_root == *allowed || host == *allowed) {
            return Err(format!("Untrusted Git host: {}. Allowed: {:?}", host, ALLOWED_HOSTS));
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
    let md_path = if skill_md.exists() { &skill_md }
        else if claude_md.exists() { &claude_md }
        else { return Err(format!("No SKILL.md or CLAUDE.md found in {}", dir.display())); };

    let content = std::fs::read_to_string(md_path).map_err(|e| e.to_string())?;
    let desc = content.lines()
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
    let name = repo.split('/').last().unwrap_or(&repo).replace(".git", "");
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let skills_dir = std::path::PathBuf::from(home).join(".ai-studio").join("skills");
    let dest = skills_dir.join(&name);

    if dest.exists() {
        let (desc, _) = read_skill_metadata(&dest)?;
        let _ = state.harness.skill_loader.scan_all().await;
        return Ok(CapabilityInfo {
            id: name.clone(), name: name.clone(), description: desc,
            kind: "skill".into(), source: repo, version: "1.0.0".into(), enabled: true,
        });
    }

    std::fs::create_dir_all(&skills_dir).map_err(|e| e.to_string())?;

    let dest_str = dest.to_string_lossy();
    let output = std::process::Command::new("git")
        .args(["clone", "--depth=1", &url, &*dest_str])
        .output()
        .map_err(|e| format!("Git failed: {}", e))?;

    if !output.status.success() {
        return Err(format!("Git clone failed: {}", String::from_utf8_lossy(&output.stderr)));
    }

    // Re-scan and read metadata
    let _ = state.harness.skill_loader.scan_all().await;
    let (desc, _) = read_skill_metadata(&dest).unwrap_or_default();

    Ok(CapabilityInfo {
        id: name.clone(), name: name.clone(), description: desc,
        kind: "skill".into(), source: repo, version: "1.0.0".into(), enabled: true,
    })
}
