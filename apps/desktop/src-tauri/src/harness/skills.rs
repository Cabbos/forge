use std::path::{Path, PathBuf};
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

#[derive(Debug, Clone)]
pub struct MatchedSkill {
    pub skill: LoadedSkill,
    pub reason: String,
}

impl MatchedSkill {
    pub fn label(&self) -> String {
        let reason = match self.reason.as_str() {
            "always_on" => "常驻".to_string(),
            "default" => "默认".to_string(),
            reason if reason.starts_with("trigger:") => {
                let triggers = reason.trim_start_matches("trigger:");
                format!("触发：{triggers}")
            }
            other => other.to_string(),
        };
        format!("{}（{}）", self.skill.name, reason)
    }
}

pub struct SkillLoader {
    skills: RwLock<Vec<LoadedSkill>>,
    /// Directories to scan for skills.
    scan_dirs: RwLock<Vec<PathBuf>>,
    db: StdRwLock<Option<Arc<Database>>>,
}

impl SkillLoader {
    pub fn new() -> Self {
        let scan_dirs = home_dir()
            .map(|home| user_skill_scan_dirs(&home))
            .unwrap_or_default();
        Self::with_scan_dirs(scan_dirs)
    }

    pub fn new_for_workspace(working_dir: &Path) -> Self {
        Self::with_scan_dirs(workspace_skill_scan_dirs(working_dir))
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
        self.matched_skills_for_request(request)
            .await
            .into_iter()
            .map(|matched| matched.skill)
            .collect()
    }

    pub async fn matched_skills_for_request(&self, request: &str) -> Vec<MatchedSkill> {
        let request_lower = request.to_lowercase();
        self.skills
            .read()
            .await
            .iter()
            .filter(|skill| skill.enabled)
            .filter_map(|skill| {
                skill_match_reason(skill, &request_lower).map(|reason| MatchedSkill {
                    skill: skill.clone(),
                    reason,
                })
            })
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

fn home_dir() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

fn user_skill_scan_dirs(home: &Path) -> Vec<PathBuf> {
    vec![home.join(".forge").join("skills")]
}

fn workspace_skill_scan_dirs(working_dir: &Path) -> Vec<PathBuf> {
    let mut scan_dirs = home_dir()
        .map(|home| user_skill_scan_dirs(&home))
        .unwrap_or_default();
    scan_dirs.push(working_dir.join(".forge").join("skills"));
    scan_dirs.push(working_dir.join("skills"));
    scan_dirs
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

fn skill_match_reason(skill: &LoadedSkill, request_lower: &str) -> Option<String> {
    if skill.always_on {
        return Some("always_on".to_string());
    }
    if skill.triggers.is_empty() {
        return Some("default".to_string());
    }

    let matched_triggers = skill
        .triggers
        .iter()
        .filter(|trigger| request_lower.contains(&trigger.to_lowercase()))
        .cloned()
        .collect::<Vec<_>>();
    if matched_triggers.is_empty() {
        None
    } else {
        Some(format!("trigger:{}", matched_triggers.join(",")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_skill_scan_dirs_do_not_include_workspace_paths() {
        let home = Path::new("/tmp/forge-home");

        let dirs = user_skill_scan_dirs(home);

        assert_eq!(dirs, vec![home.join(".forge").join("skills")]);
    }

    #[test]
    fn workspace_skill_scan_dirs_include_project_overrides_after_user_dir() {
        let workspace = Path::new("/tmp/project");
        let dirs = workspace_skill_scan_dirs(workspace);
        let project_dirs = [
            workspace.join(".forge").join("skills"),
            workspace.join("skills"),
        ];

        assert!(dirs.ends_with(&project_dirs));
    }

    // ── parse_inline_string_list ──────────────────────────────────────

    #[test]
    fn parses_comma_separated_list() {
        let result = parse_inline_string_list("fix, bug, feature");
        assert_eq!(result, vec!["fix", "bug", "feature"]);
    }

    #[test]
    fn parses_json_array() {
        let result = parse_inline_string_list(r#"["fix", "bug", "feature"]"#);
        assert_eq!(result, vec!["fix", "bug", "feature"]);
    }

    #[test]
    fn parses_quoted_items() {
        let result = parse_inline_string_list(r#""fix bug", 'feature request'"#);
        assert!(!result.is_empty());
    }

    #[test]
    fn empty_string_returns_empty() {
        let result = parse_inline_string_list("");
        assert!(result.is_empty());
    }

    #[test]
    fn whitespace_only_returns_empty() {
        let result = parse_inline_string_list("   ");
        assert!(result.is_empty(), "got {result:?}");
    }

    // ── parse_skill_metadata ──────────────────────────────────────────

    #[test]
    fn parses_description_from_frontmatter() {
        let content = "description: A test skill for fixing bugs\n\ntriggers: fix\n";
        let meta = parse_skill_metadata(content);
        assert_eq!(meta.description, "A test skill for fixing bugs");
    }

    #[test]
    fn parses_triggers() {
        let content = "description: test\ntriggers: fix, bug, feature\n";
        let meta = parse_skill_metadata(content);
        assert!(meta.triggers.contains(&"fix".to_string()));
        assert!(meta.triggers.contains(&"bug".to_string()));
        assert!(meta.triggers.contains(&"feature".to_string()));
    }

    #[test]
    fn parses_keywords_as_triggers() {
        let content = "description: test\nkeywords: refactor, cleanup\n";
        let meta = parse_skill_metadata(content);
        assert!(meta.triggers.contains(&"refactor".to_string()));
        assert!(meta.triggers.contains(&"cleanup".to_string()));
    }

    #[test]
    fn parses_always_on_true() {
        let content = "description: test\nalways_on: true\n";
        let meta = parse_skill_metadata(content);
        assert!(meta.always_on);
    }

    #[test]
    fn parses_always_on_false() {
        let content = "description: test\nalways_on: false\n";
        let meta = parse_skill_metadata(content);
        assert!(!meta.always_on);
    }

    #[test]
    fn always_on_defaults_false_when_missing() {
        let content = "description: test\n";
        let meta = parse_skill_metadata(content);
        assert!(!meta.always_on);
    }

    #[test]
    fn empty_content_returns_defaults() {
        let meta = parse_skill_metadata("");
        assert!(meta.description.is_empty());
        assert!(meta.triggers.is_empty());
        assert!(!meta.always_on);
    }

    // ── skill_match_reason ────────────────────────────────────────────

    fn make_skill(name: &str, triggers: Vec<&str>, always_on: bool) -> LoadedSkill {
        LoadedSkill {
            id: name.to_string(),
            name: name.to_string(),
            description: "test skill".to_string(),
            instruction: "# Test\n\ndescription: test\n".to_string(),
            source: SkillSource::Local(PathBuf::from("/tmp")),
            enabled: true,
            triggers: triggers.iter().map(|s| s.to_string()).collect(),
            always_on,
            tools: vec![],
        }
    }

    #[test]
    fn always_on_skill_matches_any_request() {
        let skill = make_skill("always", vec![], true);
        let reason = skill_match_reason(&skill, "any random text");
        assert_eq!(reason, Some("always_on".to_string()));
    }

    #[test]
    fn skill_without_triggers_matches_as_default() {
        let skill = make_skill("default", vec![], false);
        let reason = skill_match_reason(&skill, "any random text");
        assert_eq!(reason, Some("default".to_string()));
    }

    #[test]
    fn skill_with_triggers_matches_on_keyword() {
        let skill = make_skill("git-helper", vec!["git", "commit", "branch"], false);
        let reason = skill_match_reason(&skill, "please help me fix a git issue");
        assert!(reason.is_some());
        assert!(reason.unwrap().contains("git"));
    }

    #[test]
    fn skill_with_triggers_does_not_match_unrelated_request() {
        let skill = make_skill("git-helper", vec!["git", "commit", "branch"], false);
        let reason = skill_match_reason(&skill, "please write a python script");
        assert!(reason.is_none());
    }

    #[test]
    fn trigger_match_is_case_insensitive() {
        let skill = make_skill("docker", vec!["docker", "container"], false);
        // skill_match_reason expects pre-lowercased input (as done by matched_skills_for_request)
        let reason = skill_match_reason(&skill, "i need to build a docker image");
        assert!(reason.is_some());
    }

    #[test]
    fn multiple_trigger_matches_are_joined() {
        let skill = make_skill("devops", vec!["docker", "k8s", "deploy"], false);
        let reason = skill_match_reason(&skill, "deploy my docker container to k8s");
        assert!(reason.is_some());
        let r = reason.unwrap();
        assert!(r.starts_with("trigger:"));
        assert!(r.contains("docker"));
        assert!(r.contains("k8s"));
        assert!(r.contains("deploy"));
    }

    // ── MatchedSkill::label ───────────────────────────────────────────

    #[test]
    fn label_always_on_shows_chinese_label() {
        let skill = make_skill("helper", vec![], true);
        let matched = MatchedSkill {
            skill,
            reason: "always_on".to_string(),
        };
        assert!(matched.label().contains("常驻"));
    }

    #[test]
    fn label_default_shows_chinese_label() {
        let skill = make_skill("helper", vec![], false);
        let matched = MatchedSkill {
            skill,
            reason: "default".to_string(),
        };
        assert!(matched.label().contains("默认"));
    }

    #[test]
    fn label_trigger_shows_chinese_prefix() {
        let skill = make_skill("git", vec!["git"], false);
        let matched = MatchedSkill {
            skill,
            reason: "trigger:git,commit".to_string(),
        };
        let label = matched.label();
        assert!(label.contains("触发"));
        assert!(label.contains("git"));
    }
}
