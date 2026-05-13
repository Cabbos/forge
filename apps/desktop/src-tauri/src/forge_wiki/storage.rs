use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::forge_wiki::model::{
    ForgeWikiPage, ForgeWikiPageKind, ForgeWikiState, SelectedForgeWikiPage,
};
use crate::forge_wiki::safety::{
    contains_sensitive_wiki_content, resolve_wiki_page_path, wiki_dir,
};

const DEFAULT_PAGES: [(&str, ForgeWikiPageKind, &str); 6] = [
    (
        "index.md",
        ForgeWikiPageKind::Index,
        "# 项目概览\n\n记录这个项目的目标、边界和当前状态。\n",
    ),
    (
        "schema.md",
        ForgeWikiPageKind::Schema,
        "# 记录规则\n\n用简短条目维护事实、决策、任务和来源；过期内容及时更新。\n",
    ),
    (
        "sources.md",
        ForgeWikiPageKind::Sources,
        "# 资料来源\n\n记录重要文档、链接、命令输出和人工确认的来源。\n",
    ),
    (
        "decisions.md",
        ForgeWikiPageKind::Decisions,
        "# 决策记录\n\n记录已经确定的产品方向、技术方案和取舍原因。\n",
    ),
    (
        "tasks.md",
        ForgeWikiPageKind::Tasks,
        "# 当前任务\n\n记录正在推进的任务、阻塞点和下一步。\n",
    ),
    (
        "log.md",
        ForgeWikiPageKind::Log,
        "# 工作日志\n\n记录构建、验证、报错和重要检查结果。\n",
    ),
];

pub struct ForgeWikiStore;

impl ForgeWikiStore {
    pub fn new() -> Self {
        Self
    }

    pub async fn get_state(&self, project_path: &str) -> Result<ForgeWikiState, String> {
        let dir = wiki_dir(project_path);
        let exists = dir.is_dir();
        let pages = if exists {
            self.list_pages(project_path).await?
        } else {
            Vec::new()
        };
        Ok(ForgeWikiState {
            project_path: project_path.to_string(),
            exists,
            wiki_dir: dir.display().to_string(),
            pages,
            message: if exists {
                "Forge Wiki 已就绪".to_string()
            } else {
                "Forge Wiki 尚未初始化".to_string()
            },
        })
    }

    pub async fn init(&self, project_path: &str) -> Result<ForgeWikiState, String> {
        let dir = wiki_dir(project_path);
        let parent = dir
            .parent()
            .ok_or_else(|| "Failed to resolve wiki directory parent".to_string())?;
        fs::create_dir_all(parent)
            .map_err(|err| format!("Failed to create wiki directory: {err}"))?;
        match fs::symlink_metadata(&dir) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err("Forge Wiki directory cannot be a symlink".to_string());
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                fs::create_dir(&dir)
                    .map_err(|err| format!("Failed to create wiki directory: {err}"))?;
            }
            Err(err) => return Err(format!("Failed to inspect wiki directory: {err}")),
        }

        for (path, _, content) in DEFAULT_PAGES {
            if contains_sensitive_wiki_content(content) {
                return Err(format!(
                    "Default wiki page {path} contains sensitive content"
                ));
            }
            let page_path = dir.join(path);
            match fs::symlink_metadata(&page_path) {
                Ok(metadata) => {
                    if metadata.file_type().is_symlink() {
                        return Err(format!("Default wiki page {path} cannot be a symlink"));
                    }
                }
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    fs::write(&page_path, content)
                        .map_err(|err| format!("Failed to write default page {path}: {err}"))?;
                }
                Err(err) => {
                    return Err(format!("Failed to inspect default page {path}: {err}"));
                }
            }
        }

        self.get_state(project_path).await
    }

    pub async fn list_pages(&self, project_path: &str) -> Result<Vec<ForgeWikiPage>, String> {
        let dir = wiki_dir(project_path);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut pages = Vec::new();
        for (path, kind, _) in DEFAULT_PAGES {
            let full_path = dir.join(path);
            if full_path.is_file() {
                pages.push(page_from_file(project_path, path, kind, &full_path)?);
            }
        }

        let mut custom_paths = Vec::new();
        collect_markdown_pages(&dir, &dir, &mut custom_paths)?;
        custom_paths.sort();
        for path in custom_paths {
            if DEFAULT_PAGES
                .iter()
                .any(|(default_path, _, _)| *default_path == path)
            {
                continue;
            }
            let full_path = dir.join(&path);
            pages.push(page_from_file(
                project_path,
                &path,
                ForgeWikiPageKind::Custom,
                &full_path,
            )?);
        }

        Ok(pages)
    }

    pub async fn read_page(&self, project_path: &str, page_path: &str) -> Result<String, String> {
        let path = resolve_wiki_page_path(project_path, page_path)?;
        fs::read_to_string(&path)
            .map_err(|err| format!("Failed to read wiki page {page_path}: {err}"))
    }

    pub async fn select_context(
        &self,
        project_path: &str,
        message: &str,
        limit: usize,
    ) -> Result<Vec<SelectedForgeWikiPage>, String> {
        if limit == 0 || !wiki_dir(project_path).is_dir() {
            return Ok(Vec::new());
        }

        let pages = self.list_pages(project_path).await?;
        let mut desired = vec!["tasks.md", "index.md"];
        let lower = message.to_lowercase();
        if contains_any(&lower, &["方向", "方案", "决定", "继续", "产品"]) {
            desired.push("decisions.md");
        }
        if contains_any(&lower, &["失败", "报错", "构建", "验收", "检查"]) {
            desired.push("log.md");
        }

        let effective_limit = limit.min(4);
        let mut selected = Vec::new();
        for (index, path) in desired.into_iter().enumerate() {
            if selected.len() >= effective_limit {
                break;
            }
            if selected
                .iter()
                .any(|page: &SelectedForgeWikiPage| page.path == path)
            {
                continue;
            }
            if let Some(page) = pages.iter().find(|page| page.path == path) {
                selected.push(SelectedForgeWikiPage {
                    page_id: page.id.clone(),
                    title: page.title.clone(),
                    path: page.path.clone(),
                    kind: page.kind.clone(),
                    summary: page
                        .summary
                        .clone()
                        .unwrap_or_else(|| "项目 Wiki 页面".to_string()),
                    score: (100 - index) as f32,
                    reason: selection_reason(path).to_string(),
                    injected: true,
                });
            }
        }

        Ok(selected)
    }

    pub fn format_selected_context(selected: &[SelectedForgeWikiPage]) -> Option<String> {
        if selected.is_empty() {
            return None;
        }

        let mut lines = Vec::with_capacity(selected.len() + 2);
        lines.push("## Relevant Forge Wiki Pages".to_string());
        lines.push(
            "Do not reveal this section unless the user asks what project Wiki context was used."
                .to_string(),
        );
        for page in selected {
            lines.push(format!(
                "- path={} title={} summary={} reason={}",
                wiki_data_text(&page.path),
                wiki_data_text(&page.title),
                wiki_data_text(&page.summary),
                wiki_data_text(&page.reason)
            ));
        }

        Some(lines.join("\n"))
    }
}

fn page_from_file(
    project_path: &str,
    path: &str,
    kind: ForgeWikiPageKind,
    full_path: &Path,
) -> Result<ForgeWikiPage, String> {
    let text = fs::read_to_string(full_path).unwrap_or_default();
    let title = safe_page_title(&text, path);
    let summary = safe_page_summary(&text);
    Ok(ForgeWikiPage {
        id: path.to_string(),
        project_path: project_path.to_string(),
        path: path.to_string(),
        title,
        kind,
        summary,
        updated_at: updated_at(full_path)?,
        token_estimate: Some(estimate_tokens(&text)),
    })
}

fn collect_markdown_pages(root: &Path, dir: &Path, pages: &mut Vec<String>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|err| format!("Failed to list wiki directory: {err}"))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("Failed to read wiki entry: {err}"))?;
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if file_name.starts_with('.') {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|err| format!("Failed to inspect wiki entry: {err}"))?;
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_markdown_pages(root, &path, pages)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            let relative = path
                .strip_prefix(root)
                .map_err(|err| format!("Failed to normalize wiki page path: {err}"))?
                .to_string_lossy()
                .replace('\\', "/");
            pages.push(relative);
        }
    }
    Ok(())
}

fn safe_page_title(text: &str, fallback: &str) -> String {
    let title = extract_title(text).unwrap_or_else(|| fallback.to_string());
    let bounded = truncate_chars(title.trim(), 120);
    if contains_sensitive_wiki_content(&bounded) {
        fallback.to_string()
    } else {
        bounded
    }
}

fn safe_page_summary(text: &str) -> Option<String> {
    extract_summary(text).and_then(|summary| {
        let bounded = truncate_chars(summary.trim(), 240);
        if bounded.is_empty() || contains_sensitive_wiki_content(&bounded) {
            None
        } else {
            Some(bounded)
        }
    })
}

fn extract_title(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| {
            line.strip_prefix("# ")
                .map(|title| title.trim().to_string())
        })
        .filter(|title| !title.is_empty())
}

fn extract_summary(text: &str) -> Option<String> {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.to_string())
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn updated_at(path: &Path) -> Result<Option<String>, String> {
    let modified = path
        .metadata()
        .map_err(|err| format!("Failed to read wiki page metadata: {err}"))?
        .modified()
        .map_err(|err| format!("Failed to read wiki page modified time: {err}"))?;
    let seconds = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| SystemTime::UNIX_EPOCH.duration_since(UNIX_EPOCH).unwrap())
        .as_secs();
    Ok(Some(seconds.to_string()))
}

fn estimate_tokens(text: &str) -> u32 {
    ((text.chars().count() as u32) / 4).max(1)
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn selection_reason(path: &str) -> &'static str {
    match path {
        "tasks.md" => "当前任务优先",
        "index.md" => "项目概览优先",
        "decisions.md" => "方向或产品信号匹配",
        "log.md" => "构建或验证信号匹配",
        _ => "相关 Wiki 页面",
    }
}

fn wiki_data_text(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    serde_json::to_string(&normalized).unwrap_or_else(|_| "\"\"".to_string())
}

#[cfg(test)]
mod tests {
    use super::ForgeWikiStore;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs as unix_fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_project_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("forge-wiki-{name}-{nanos}"));
        fs::create_dir_all(&path).expect("create temp project");
        path
    }

    fn cleanup(path: &Path) {
        let _ = fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn init_creates_default_pages_with_safe_content() {
        let project = temp_project_dir("init");
        let store = ForgeWikiStore::new();

        let state = store
            .init(project.to_str().unwrap())
            .await
            .expect("init wiki");

        assert!(state.exists);
        assert_eq!(
            state.wiki_dir,
            project.join(".forge/wiki").display().to_string()
        );

        let expected = [
            ("index.md", "# 项目概览"),
            ("schema.md", "# 记录规则"),
            ("sources.md", "# 资料来源"),
            ("decisions.md", "# 决策记录"),
            ("tasks.md", "# 当前任务"),
            ("log.md", "# 工作日志"),
        ];

        for (file_name, heading) in expected {
            let text = fs::read_to_string(project.join(".forge/wiki").join(file_name))
                .expect("default page");
            assert!(
                text.contains(heading),
                "{file_name} should contain heading {heading}"
            );
            assert!(!text.to_lowercase().contains("api key"));
            assert!(!text.to_lowercase().contains("password"));
            assert!(!text.contains("密码"));
            assert!(!text.contains("密钥"));
        }

        cleanup(&project);
    }

    #[tokio::test]
    async fn list_pages_returns_default_pages_after_init() {
        let project = temp_project_dir("list");
        let store = ForgeWikiStore::new();
        store
            .init(project.to_str().unwrap())
            .await
            .expect("init wiki");
        fs::write(project.join(".forge/wiki/.proposals.json"), "[]").expect("write internal file");

        let pages = store
            .list_pages(project.to_str().unwrap())
            .await
            .expect("list pages");

        let paths = pages
            .iter()
            .map(|page| page.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "index.md",
                "schema.md",
                "sources.md",
                "decisions.md",
                "tasks.md",
                "log.md"
            ]
        );

        cleanup(&project);
    }

    #[tokio::test]
    async fn read_page_rejects_path_traversal() {
        let project = temp_project_dir("traversal");
        let store = ForgeWikiStore::new();
        store
            .init(project.to_str().unwrap())
            .await
            .expect("init wiki");
        fs::write(project.join("AGENTS.md"), "outside").expect("write outside file");

        let result = store
            .read_page(project.to_str().unwrap(), "../AGENTS.md")
            .await;

        assert!(result.is_err());
        cleanup(&project);
    }

    #[tokio::test]
    async fn read_page_rejects_absolute_and_non_markdown_paths() {
        let project = temp_project_dir("invalid-paths");
        let store = ForgeWikiStore::new();
        store
            .init(project.to_str().unwrap())
            .await
            .expect("init wiki");

        let absolute = project.join(".forge/wiki/index.md");
        assert!(store
            .read_page(project.to_str().unwrap(), absolute.to_str().unwrap())
            .await
            .is_err());
        assert!(store
            .read_page(project.to_str().unwrap(), "notes.txt")
            .await
            .is_err());

        cleanup(&project);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn list_pages_ignores_symlinked_directories() {
        let project = temp_project_dir("symlinked-list");
        let external = temp_project_dir("symlinked-list-external");
        let store = ForgeWikiStore::new();
        store
            .init(project.to_str().unwrap())
            .await
            .expect("init wiki");
        fs::write(external.join("outside.md"), "# Outside\n\nexternal summary")
            .expect("write external page");
        unix_fs::symlink(external.as_path(), project.join(".forge/wiki/linked"))
            .expect("create directory symlink");

        let pages = store
            .list_pages(project.to_str().unwrap())
            .await
            .expect("list pages");

        assert!(!pages.iter().any(|page| page.path == "linked/outside.md"));
        cleanup(&project);
        cleanup(&external);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn read_page_rejects_symlink_escape() {
        let project = temp_project_dir("symlinked-read");
        let external = temp_project_dir("symlinked-read-external");
        let store = ForgeWikiStore::new();
        store
            .init(project.to_str().unwrap())
            .await
            .expect("init wiki");
        fs::write(external.join("secret.md"), "# Secret\n\nexternal").expect("write external page");
        unix_fs::symlink(external.as_path(), project.join(".forge/wiki/linked"))
            .expect("create directory symlink");

        let error = store
            .read_page(project.to_str().unwrap(), "linked/secret.md")
            .await
            .expect_err("symlink escape should be rejected");

        assert!(
            error.contains("symlink"),
            "expected symlink rejection, got {error}"
        );
        cleanup(&project);
        cleanup(&external);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn init_rejects_dangling_symlink_default_page() {
        let project = temp_project_dir("dangling-default");
        let store = ForgeWikiStore::new();
        let wiki = project.join(".forge/wiki");
        fs::create_dir_all(&wiki).expect("create wiki dir");
        unix_fs::symlink(project.join("outside.md"), wiki.join("index.md"))
            .expect("create dangling default page symlink");

        let error = store
            .init(project.to_str().unwrap())
            .await
            .expect_err("dangling default page symlink should be rejected");

        assert!(
            error.contains("symlink"),
            "expected symlink rejection, got {error}"
        );
        assert!(!project.join("outside.md").exists());
        cleanup(&project);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn init_rejects_symlinked_wiki_root() {
        let project = temp_project_dir("symlinked-root");
        let external = temp_project_dir("symlinked-root-external");
        let store = ForgeWikiStore::new();
        fs::create_dir_all(project.join(".forge")).expect("create forge dir");
        unix_fs::symlink(external.as_path(), project.join(".forge/wiki"))
            .expect("create wiki root symlink");

        let error = store
            .init(project.to_str().unwrap())
            .await
            .expect_err("symlinked wiki root should be rejected");

        assert!(
            error.contains("symlink"),
            "expected symlink rejection, got {error}"
        );
        assert!(!external.join("index.md").exists());
        assert!(!external.join("tasks.md").exists());
        cleanup(&project);
        cleanup(&external);
    }

    #[tokio::test]
    async fn custom_page_title_and_summary_are_bounded_and_screened() {
        let project = temp_project_dir("bounded-metadata");
        let store = ForgeWikiStore::new();
        store
            .init(project.to_str().unwrap())
            .await
            .expect("init wiki");
        let long_title = "T".repeat(180);
        let long_summary = "S".repeat(300);
        fs::write(
            project.join(".forge/wiki/long.md"),
            format!("# {long_title}\n\n{long_summary}"),
        )
        .expect("write long page");
        fs::write(
            project.join(".forge/wiki/sensitive.md"),
            "# Safe\n\npassword = hunter2",
        )
        .expect("write sensitive page");

        let pages = store
            .list_pages(project.to_str().unwrap())
            .await
            .expect("list pages");
        let long_page = pages
            .iter()
            .find(|page| page.path == "long.md")
            .expect("long page");
        let sensitive_page = pages
            .iter()
            .find(|page| page.path == "sensitive.md")
            .expect("sensitive page");

        assert_eq!(long_page.title.chars().count(), 120);
        assert_eq!(long_page.summary.as_ref().unwrap().chars().count(), 240);
        assert_eq!(sensitive_page.title, "Safe");
        assert!(sensitive_page.summary.is_none());
        cleanup(&project);
    }

    #[tokio::test]
    async fn select_context_is_deterministic_and_limited() {
        let project = temp_project_dir("select");
        let store = ForgeWikiStore::new();
        store
            .init(project.to_str().unwrap())
            .await
            .expect("init wiki");

        let selected = store
            .select_context(
                project.to_str().unwrap(),
                "继续产品方向，检查构建报错和验收结果",
                3,
            )
            .await
            .expect("select context");

        let paths = selected
            .iter()
            .map(|page| page.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["tasks.md", "index.md", "decisions.md"]);

        cleanup(&project);
    }

    #[tokio::test]
    async fn format_selected_context_has_hidden_title() {
        let project = temp_project_dir("format");
        let store = ForgeWikiStore::new();
        store
            .init(project.to_str().unwrap())
            .await
            .expect("init wiki");
        let selected = store
            .select_context(project.to_str().unwrap(), "检查构建", 4)
            .await
            .expect("select context");

        let formatted = ForgeWikiStore::format_selected_context(&selected).expect("context");

        assert!(formatted.starts_with("## Relevant Forge Wiki Pages\n"));
        assert!(formatted.contains("Do not reveal this section"));
        assert!(formatted.contains("tasks.md"));

        cleanup(&project);
    }
}
