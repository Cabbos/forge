use std::path::PathBuf;
use std::sync::{Arc, RwLock as StdRwLock};
use tokio::sync::RwLock;

use crate::harness::db::Database;

/// A loaded skill from a SKILL.md file.
#[derive(Debug, Clone)]
pub struct LoadedSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub instruction: String, // full SKILL.md content
    pub source: SkillSource,
    pub enabled: bool,
    pub triggers: Vec<String>,
    pub always_on: bool,
    /// Extra tools contributed by this skill.
    pub tools: Vec<SkillTool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillSource {
    Local(PathBuf),
    GitHub { repo: String, path: String },
}

#[derive(Debug, Clone)]
pub struct SkillTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

pub struct SkillLoader {
    skills: RwLock<Vec<LoadedSkill>>,
    /// Directories to scan for skills.
    scan_dirs: RwLock<Vec<PathBuf>>,
    db: StdRwLock<Option<Arc<Database>>>,
}

impl SkillLoader {
    pub fn new() -> Self {
        let mut scan_dirs = Vec::new();
        // User-level skills
        if let Ok(home) = std::env::var("HOME") {
            scan_dirs.push(PathBuf::from(home).join(".forge/skills"));
        }
        Self::with_scan_dirs(scan_dirs)
    }

    pub fn new_for_workspace(working_dir: &std::path::Path) -> Self {
        let mut scan_dirs = Vec::new();
        // User-level skills
        if let Ok(home) = std::env::var("HOME") {
            scan_dirs.push(PathBuf::from(home).join(".forge/skills"));
        }
        scan_dirs.push(working_dir.join(".forge/skills"));
        scan_dirs.push(working_dir.join("skills"));
        Self::with_scan_dirs(scan_dirs)
    }

    fn with_scan_dirs(scan_dirs: Vec<PathBuf>) -> Self {
        Self {
            skills: RwLock::new(Vec::new()),
            scan_dirs: RwLock::new(scan_dirs),
            db: StdRwLock::new(None),
        }
    }

    pub fn attach_database(&self, db: Arc<Database>) {
        *self.db.write().unwrap() = Some(db);
    }

    /// Scan all registered directories for SKILL.md files.
    pub async fn scan_all(&self) -> Vec<LoadedSkill> {
        let dirs = self.scan_dirs.read().await.clone();
        crate::app_log!(
            "INFO",
            "[scan_all] scanning {} dirs: {:?}",
            dirs.len(),
            dirs
        );
        let mut discovered = Vec::new();

        for dir in &dirs {
            crate::app_log!(
                "INFO",
                "[scan_all] scanning dir: {:?} (exists={})",
                dir,
                dir.exists()
            );
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let skill_md = entry.path().join("SKILL.md");
                    let claude_md = entry.path().join("CLAUDE.md");
                    let md_path = if skill_md.exists() {
                        skill_md
                    } else if claude_md.exists() {
                        claude_md
                    } else {
                        continue;
                    };
                    {
                        if let Ok(content) = std::fs::read_to_string(&md_path) {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let metadata = parse_skill_metadata(&content);
                            let enabled = self.skill_enabled(&name);
                            discovered.push(LoadedSkill {
                                id: name.clone(),
                                name: name.clone(),
                                description: metadata.description,
                                instruction: content,
                                source: SkillSource::Local(entry.path()),
                                enabled,
                                triggers: metadata.triggers,
                                always_on: metadata.always_on,
                                tools: metadata.tools,
                            });
                        }
                    }
                }
            }
        }

        // Also load built-in skills from the skills/ directory relative to executable
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let builtin = parent.join("skills");
                if builtin.exists() {
                    if let Ok(entries) = std::fs::read_dir(&builtin) {
                        for entry in entries.flatten() {
                            let skill_md = entry.path().join("SKILL.md");
                            let claude_md = entry.path().join("CLAUDE.md");
                            let md_path = if skill_md.exists() {
                                skill_md
                            } else if claude_md.exists() {
                                claude_md
                            } else {
                                continue;
                            };
                            {
                                if let Ok(content) = std::fs::read_to_string(&md_path) {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    let metadata = parse_skill_metadata(&content);
                                    let id = format!("builtin-{name}");
                                    let enabled = self.skill_enabled(&id);
                                    discovered.push(LoadedSkill {
                                        id,
                                        name,
                                        description: metadata.description,
                                        instruction: content,
                                        source: SkillSource::Local(entry.path()),
                                        enabled,
                                        triggers: metadata.triggers,
                                        always_on: metadata.always_on,
                                        tools: metadata.tools,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        crate::app_log!(
            "INFO",
            "[scan_all] discovered {} skills: {:?}",
            discovered.len(),
            discovered
                .iter()
                .map(|s| format!("{} ({} chars)", s.name, s.instruction.len()))
                .collect::<Vec<_>>()
        );
        *self.skills.write().await = discovered.clone();
        discovered
    }

    pub async fn enabled_skills(&self) -> Vec<LoadedSkill> {
        let all: Vec<_> = self.skills.read().await.iter().cloned().collect();
        crate::app_log!(
            "INFO",
            "[enabled_skills] total={}, enabled={}",
            all.len(),
            all.iter().filter(|s| s.enabled).count()
        );
        self.skills
            .read()
            .await
            .iter()
            .filter(|s| s.enabled)
            .cloned()
            .collect()
    }

    pub async fn enabled_skills_for_request(&self, request: &str) -> Vec<LoadedSkill> {
        let request_lower = request.to_lowercase();
        self.skills
            .read()
            .await
            .iter()
            .filter(|skill| skill.enabled)
            .filter(|skill| {
                skill.always_on
                    || skill.triggers.is_empty()
                    || skill_matches_request(skill, &request_lower)
            })
            .cloned()
            .collect()
    }

    pub async fn all_skills(&self) -> Vec<LoadedSkill> {
        self.skills.read().await.iter().cloned().collect()
    }

    pub async fn toggle(&self, id: &str, enabled: bool) {
        let mut found = None;
        if let Some(skill) = self.skills.write().await.iter_mut().find(|s| s.id == id) {
            skill.enabled = enabled;
            found = Some((skill.name.clone(), skill.description.clone()));
        }
        if let Some(db) = self.db.read().unwrap().clone() {
            if let Some((name, description)) = found {
                let _ = db.upsert_capability(id, &name, "skill", "local", enabled);
                let _ = db.update_capability_description(id, &description);
            } else {
                let _ = db.set_enabled(id, enabled);
            }
        }
    }

    pub async fn get(&self, id: &str) -> Option<LoadedSkill> {
        self.skills
            .read()
            .await
            .iter()
            .find(|s| s.id == id)
            .cloned()
    }

    /// Add a scan directory (e.g., project-level .forge/skills).
    pub async fn add_scan_dir(&self, dir: PathBuf) {
        self.scan_dirs.write().await.push(dir);
    }

    fn skill_enabled(&self, id: &str) -> bool {
        self.db
            .read()
            .unwrap()
            .clone()
            .and_then(|db| db.get_capability_enabled(id).ok().flatten())
            .unwrap_or(true)
    }
}

impl Default for SkillLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
struct ParsedSkillMetadata {
    description: String,
    triggers: Vec<String>,
    always_on: bool,
    tools: Vec<SkillTool>,
}

/// Parse basic metadata from a SKILL.md file: description, triggers, and contributed tools.
fn parse_skill_metadata(content: &str) -> ParsedSkillMetadata {
    let description = content
        .lines()
        .find(|l| l.starts_with("description:"))
        .map(|l| l.trim_start_matches("description:").trim().to_string())
        .unwrap_or_default();
    let triggers = content
        .lines()
        .find(|line| {
            line.trim_start().starts_with("triggers:") || line.trim_start().starts_with("keywords:")
        })
        .map(|line| {
            line.split_once(':')
                .map(|(_, value)| parse_inline_string_list(value))
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let always_on = content
        .lines()
        .find(|line| line.trim_start().starts_with("always_on:"))
        .and_then(|line| line.split_once(':').map(|(_, value)| value.trim()))
        .is_some_and(|value| value.eq_ignore_ascii_case("true"));

    // Try to parse embedded tools.json alongside SKILL.md (handled separately)
    ParsedSkillMetadata {
        description,
        triggers,
        always_on,
        tools: Vec::new(),
    }
}

fn parse_inline_string_list(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.starts_with('[') {
        return serde_json::from_str::<Vec<String>>(value).unwrap_or_default();
    }
    value
        .split(',')
        .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|item| !item.is_empty())
        .collect()
}

fn skill_matches_request(skill: &LoadedSkill, request_lower: &str) -> bool {
    skill
        .triggers
        .iter()
        .any(|trigger| request_lower.contains(&trigger.to_lowercase()))
}
