use std::path::PathBuf;
use tokio::sync::RwLock;

/// A loaded skill from a SKILL.md file.
#[derive(Debug, Clone)]
pub struct LoadedSkill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub instruction: String,  // full SKILL.md content
    pub source: SkillSource,
    pub enabled: bool,
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
}

impl SkillLoader {
    pub fn new() -> Self {
        let mut scan_dirs = Vec::new();
        // User-level skills
        if let Ok(home) = std::env::var("HOME") {
            scan_dirs.push(PathBuf::from(home).join(".ai-studio/skills"));
        }
        Self {
            skills: RwLock::new(Vec::new()),
            scan_dirs: RwLock::new(scan_dirs),
        }
    }

    /// Scan all registered directories for SKILL.md files.
    pub async fn scan_all(&self) -> Vec<LoadedSkill> {
        let dirs = self.scan_dirs.read().await.clone();
        let mut discovered = Vec::new();

        for dir in &dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let skill_md = entry.path().join("SKILL.md");
                    if skill_md.exists() {
                        if let Ok(content) = std::fs::read_to_string(&skill_md) {
                            let name = entry.file_name().to_string_lossy().to_string();
                            let (desc, tools) = parse_skill_metadata(&content);
                            discovered.push(LoadedSkill {
                                id: name.clone(),
                                name: name.clone(),
                                description: desc,
                                instruction: content,
                                source: SkillSource::Local(entry.path()),
                                enabled: true,
                                tools,
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
                            if skill_md.exists() {
                                if let Ok(content) = std::fs::read_to_string(&skill_md) {
                                    let name = entry.file_name().to_string_lossy().to_string();
                                    let (desc, tools) = parse_skill_metadata(&content);
                                    discovered.push(LoadedSkill {
                                        id: format!("builtin-{name}"),
                                        name,
                                        description: desc,
                                        instruction: content,
                                        source: SkillSource::Local(entry.path()),
                                        enabled: true,
                                        tools,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        *self.skills.write().await = discovered.clone();
        discovered
    }

    pub async fn enabled_skills(&self) -> Vec<LoadedSkill> {
        self.skills.read().await.iter()
            .filter(|s| s.enabled)
            .cloned()
            .collect()
    }

    pub async fn toggle(&self, id: &str, enabled: bool) {
        if let Some(skill) = self.skills.write().await.iter_mut().find(|s| s.id == id) {
            skill.enabled = enabled;
        }
    }

    /// Add a scan directory (e.g., project-level .ai-studio/skills).
    pub async fn add_scan_dir(&self, dir: PathBuf) {
        self.scan_dirs.write().await.push(dir);
    }
}

/// Parse basic metadata from a SKILL.md file: description and contributed tools.
fn parse_skill_metadata(content: &str) -> (String, Vec<SkillTool>) {
    let desc = content.lines()
        .find(|l| l.starts_with("description:"))
        .map(|l| l.trim_start_matches("description:").trim().to_string())
        .unwrap_or_default();

    // Try to parse embedded tools.json alongside SKILL.md (handled separately)
    (desc, Vec::new())
}
