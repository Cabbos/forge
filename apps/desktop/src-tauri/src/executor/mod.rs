pub mod files;
pub mod permission;
pub mod shell;

pub use files::FileExecutor;
pub use permission::PermissionGate;
pub use shell::ShellExecutor;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::protocol::events::StreamEvent;
use crate::protocol::BlockId;
use tauri::Emitter;

/// Unified executor that handles AI tool calls.
pub struct ToolExecutor {
    pub file: FileExecutor,
    pub shell: ShellExecutor,
    pub permission: Arc<Mutex<PermissionGate>>,
    pending_confirms: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
}

impl ToolExecutor {
    pub fn new(
        working_dir: PathBuf,
        pending_confirms: Arc<RwLock<HashMap<String, tokio::sync::oneshot::Sender<bool>>>>,
    ) -> Self {
        Self {
            file: FileExecutor::new(working_dir.clone()),
            shell: ShellExecutor::new(working_dir.clone()),
            permission: Arc::new(Mutex::new(PermissionGate::new(working_dir))),
            pending_confirms,
        }
    }

    /// Execute a tool call from the AI and emit result events.
    /// Returns the result string to feed back into the AI conversation.
    pub async fn execute(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: &serde_json::Value,
        app_handle: &tauri::AppHandle,
    ) -> String {
        let block_id = BlockId::new().to_string();
        let start = std::time::Instant::now();

        let result = match tool_name {
            "read_file" | "read" => {
                let path = get_str(tool_input, "path").unwrap_or("");
                match self.file.read_file(path) {
                    Ok(r) => format!("{}", r.content),
                    Err(e) => format!("Error: {}", e),
                }
            }
            "write_file" | "write_to_file" | "write" => {
                let path = get_str(tool_input, "path").unwrap_or("");
                let content = get_str(tool_input, "content").unwrap_or("");

                // Permission check
                {
                    let gate = self.permission.lock().await;
                    match gate.check_file_write(path) {
                        crate::executor::permission::PermissionDecision::Deny { reason } => {
                            return format!("Denied: {}", reason);
                        }
                        crate::executor::permission::PermissionDecision::Ask {
                            question,
                            kind,
                        } => {
                            // Create oneshot channel and wait for frontend response
                            let (tx, rx) = tokio::sync::oneshot::channel();
                            {
                                let mut confirms = self.pending_confirms.write().await;
                                confirms.insert(block_id.clone(), tx);
                            }
                            let _ = app_handle.emit(
                                "session-output",
                                StreamEvent::ConfirmAsk {
                                    session_id: session_id.to_string(),
                                    block_id: block_id.clone(),
                                    question,
                                    kind,
                                },
                            );
                            // Wait for user to click Yes/No (with 120s timeout)
                            match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
                                Ok(Ok(true)) => {
                                    self.pending_confirms.write().await.remove(&block_id);
                                }
                                Ok(Ok(false)) => {
                                    let mut confirms = self.pending_confirms.write().await;
                                    confirms.remove(&block_id);
                                    return "Denied by user".to_string();
                                }
                                Ok(Err(_)) | Err(_) => {
                                    let mut confirms = self.pending_confirms.write().await;
                                    confirms.remove(&block_id);
                                    return "Cancelled: timeout or session closed".to_string();
                                }
                            }
                        }
                        _ => {}
                    }
                }

                match self.file.write_file(path, content) {
                    Ok(wr) => {
                        let _ = app_handle.emit(
                            "session-output",
                            StreamEvent::DiffView {
                                session_id: session_id.to_string(),
                                block_id: block_id.clone(),
                                file_path: wr.path.clone(),
                                old_content: wr.old_content,
                                new_content: wr.new_content,
                            },
                        );
                        format!("File written: {}", wr.path)
                    }
                    Err(e) => format!("Error: {}", e),
                }
            }
            "run_shell" | "bash" | "execute_command" | "shell" => {
                let command = get_str(tool_input, "command").unwrap_or("");

                // Permission check for dangerous commands
                {
                    let gate = self.permission.lock().await;
                    match gate.check_shell_command(command) {
                        crate::executor::permission::PermissionDecision::Deny { reason } => {
                            return format!("Denied: {}", reason);
                        }
                        crate::executor::permission::PermissionDecision::Ask { question, kind } => {
                            let (tx, rx) = tokio::sync::oneshot::channel();
                            {
                                let mut confirms = self.pending_confirms.write().await;
                                confirms.insert(block_id.clone(), tx);
                            }
                            let _ = app_handle.emit(
                                "session-output",
                                StreamEvent::ConfirmAsk {
                                    session_id: session_id.to_string(),
                                    block_id: block_id.clone(),
                                    question,
                                    kind,
                                },
                            );
                            match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
                                Ok(Ok(true)) => {
                                    self.pending_confirms.write().await.remove(&block_id);
                                }
                                Ok(Ok(false)) => {
                                    self.pending_confirms.write().await.remove(&block_id);
                                    return "Denied by user".to_string();
                                }
                                Ok(Err(_)) | Err(_) => {
                                    self.pending_confirms.write().await.remove(&block_id);
                                    return "Cancelled".to_string();
                                }
                            }
                        }
                        _ => {}
                    }
                }

                match self.shell.execute(command).await {
                    Ok(sr) => {
                        let _ = app_handle.emit(
                            "session-output",
                            StreamEvent::ShellStart {
                                session_id: session_id.to_string(),
                                block_id: block_id.clone(),
                                command: sr.command.clone(),
                            },
                        );
                        if !sr.stdout.is_empty() {
                            let _ = app_handle.emit(
                                "session-output",
                                StreamEvent::ShellOutput {
                                    session_id: session_id.to_string(),
                                    block_id: block_id.clone(),
                                    content: sr.stdout.clone(),
                                },
                            );
                        }
                        if !sr.stderr.is_empty() {
                            let _ = app_handle.emit(
                                "session-output",
                                StreamEvent::ShellOutput {
                                    session_id: session_id.to_string(),
                                    block_id: block_id.clone(),
                                    content: sr.stderr.clone(),
                                },
                            );
                        }
                        let _ = app_handle.emit(
                            "session-output",
                            StreamEvent::ShellEnd {
                                session_id: session_id.to_string(),
                                block_id: block_id.clone(),
                                exit_code: sr.exit_code,
                            },
                        );
                        format!(
                            "Exit code: {}\nStdout:\n{}\nStderr:\n{}",
                            sr.exit_code, sr.stdout, sr.stderr
                        )
                    }
                    Err(e) => format!("Error: {}", e),
                }
            }
            "edit_file" | "edit" => {
                let path = get_str(tool_input, "path").unwrap_or("");
                let old_str = get_str(tool_input, "old_string").unwrap_or("");
                let new_str = get_str(tool_input, "new_string").unwrap_or("");
                // Permission check
                {
                    let gate = self.permission.lock().await;
                    match gate.check_file_write(path) {
                        crate::executor::permission::PermissionDecision::Deny { reason } => return format!("Denied: {}", reason),
                        crate::executor::permission::PermissionDecision::Ask { question, kind } => {
                            let (tx, rx) = tokio::sync::oneshot::channel();
                            { self.pending_confirms.write().await.insert(block_id.clone(), tx); }
                            let _ = app_handle.emit("session-output", StreamEvent::ConfirmAsk { session_id: session_id.to_string(), block_id: block_id.clone(), question, kind });
                            match tokio::time::timeout(std::time::Duration::from_secs(120), rx).await {
                                Ok(Ok(true)) => { self.pending_confirms.write().await.remove(&block_id); }
                                _ => { self.pending_confirms.write().await.remove(&block_id); return "Denied by user".to_string(); }
                            }
                        }
                        _ => {}
                    }
                }
                match self.file.edit_file(path, old_str, new_str) {
                    Ok(msg) => msg,
                    Err(e) => format!("Error: {}", e),
                }
            }
            "list_directory" | "ls" | "list" => {
                let path = get_str(tool_input, "path").unwrap_or("");
                match self.file.list_directory(path) {
                    Ok(listing) => listing,
                    Err(e) => format!("Error: {}", e),
                }
            }
            "search_files" | "glob" => {
                let pattern = get_str(tool_input, "pattern").unwrap_or("*");
                let path = get_str(tool_input, "path").unwrap_or("");
                let dir = if path.is_empty() {
                    self.file.working_dir().to_path_buf()
                } else {
                    std::path::PathBuf::from(path)
                };
                let results = simple_glob(&dir, pattern);
                if results.is_empty() { "No files matched".to_string() } else { results.join("\n") }
            }
            "search_content" | "grep" => {
                let pattern = get_str(tool_input, "pattern").unwrap_or("");
                match self.file.search_files(pattern) {
                    Ok(matches) => {
                        if matches.is_empty() { "No matches found".to_string() }
                        else { matches.iter().map(|m| format!("{}:{}: {}", m.file_path, m.line_number, m.line_content)).collect::<Vec<_>>().join("\n") }
                    }
                    Err(e) => format!("Error: {}", e),
                }
            }
            "web_search" => {
                let query = get_str(tool_input, "query").unwrap_or("");
                web_search(query).await
            }
            "web_fetch" => {
                let url = get_str(tool_input, "url").unwrap_or("");
                web_fetch(url).await
            }
            "ask_user" => {
                let question = get_str(tool_input, "question").unwrap_or("");
                let (tx, rx) = tokio::sync::oneshot::channel();
                { self.pending_confirms.write().await.insert(block_id.clone(), tx); }
                let _ = app_handle.emit("session-output", StreamEvent::ConfirmAsk {
                    session_id: session_id.to_string(),
                    block_id: block_id.clone(),
                    question: question.to_string(),
                    kind: "ask_user".to_string(),
                });
                match tokio::time::timeout(std::time::Duration::from_secs(300), rx).await {
                    Ok(Ok(true)) => {
                        self.pending_confirms.write().await.remove(&block_id);
                        "User approved".to_string()
                    }
                    Ok(Ok(false)) => {
                        self.pending_confirms.write().await.remove(&block_id);
                        "User declined".to_string()
                    }
                    _ => {
                        self.pending_confirms.write().await.remove(&block_id);
                        "No response from user".to_string()
                    }
                }
            }
            _ => format!("Unknown tool: {}", tool_name),
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let is_error = result.starts_with("Error:") || result.starts_with("Denied:");

        let _ = app_handle.emit(
            "session-output",
            StreamEvent::ToolCallResult {
                session_id: session_id.to_string(),
                block_id,
                result: result.clone(),
                is_error,
                duration_ms,
            },
        );

        result
    }
}

fn get_str<'a>(val: &'a serde_json::Value, key: &str) -> Option<&'a str> {
    val.get(key)?.as_str()
}

fn browser_headers() -> reqwest::header::HeaderMap {
    use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT, ACCEPT, ACCEPT_LANGUAGE};
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"));
    headers.insert(ACCEPT, HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"));
    headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    headers
}

/// Search DuckDuckGo Lite and return structured results (no API key required, no scraping blocks).
async fn web_search(query: &str) -> String {
    let url = format!(
        "https://lite.duckduckgo.com/lite/?q={}",
        urlencoding(query)
    );
    let client = reqwest::Client::new();
    match client
        .get(&url)
        .headers(browser_headers())
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(resp) => {
            let html = resp.text().await.unwrap_or_default();
            let mut results = Vec::new();

            // DuckDuckGo Lite returns results in <a> tags with class "result-link" and snippets in <td> with class "result-snippet"
            // Each result row: <tr> with class "result-snippet" containing title link + snippet
            for block in html.split("class=\"result-snippet\"").skip(1) {
                // Extract title + URL from the preceding result-link <a>
                let title_and_link = html
                    .split("class=\"result-link\"")
                    .filter(|s| s.contains(block) || html.split("class=\"result-snippet\"").position(|b| b.as_ptr() == block.as_ptr()).unwrap_or(0) > 0)
                    .last();

                let title = title_and_link
                    .and_then(|s| s.split('>').nth(1))
                    .and_then(|s| s.split("</a").next())
                    .map(|s| strip_html(s.trim()))
                    .unwrap_or_default();

                let title = if title.is_empty() {
                    // Fallback: try finding any <a> with href in this block
                    block.split("href=\"").nth(1)
                        .and_then(|s| s.split('\"').next())
                        .unwrap_or("")
                } else {
                    &title
                };

                // Simpler approach: extract all links + snippets from lite results
                // Each result is: <a rel="nofollow" href="URL">Title</a> ... <td class="result-snippet">Snippet</td>
            }

            // Simpler parsing: find all result-link <a> tags
            let mut titles_and_urls: Vec<(String, String)> = Vec::new();
            for link_block in html.split("class=\"result-link\"").skip(1) {
                let url = link_block
                    .split("href=\"")
                    .nth(1)
                    .and_then(|s| s.split('\"').next())
                    .map(|s| s.replace("&amp;", "&"))
                    .unwrap_or_default();
                let title = link_block
                    .split('>')
                    .nth(1)
                    .and_then(|s| s.split("</a").next())
                    .map(|s| strip_html(s.trim()))
                    .unwrap_or_default();
                if !title.is_empty() && !url.is_empty() {
                    titles_and_urls.push((title, url));
                }
            }

            // Find all snippets
            let mut snippets: Vec<String> = Vec::new();
            for snip_block in html.split("class=\"result-snippet\"").skip(1) {
                let snip = snip_block
                    .split('>')
                    .nth(1)
                    .and_then(|s| s.split("</td").next())
                    .map(|s| strip_html(s.trim()))
                    .unwrap_or_default();
                snippets.push(snip);
            }

            // Pair titles with snippets
            for (i, (title, url)) in titles_and_urls.iter().enumerate() {
                let snip = snippets.get(i).map(|s| s.as_str()).unwrap_or("");
                results.push(format!("- {} ({})\n  {}", title, url, snip));
                if results.len() >= 5 {
                    break;
                }
            }

            if results.is_empty() {
                format!("No results found for: {}", query)
            } else {
                results
                    .iter()
                    .enumerate()
                    .map(|(i, r)| format!("{}. {}", i + 1, r))
                    .collect::<Vec<_>>()
                    .join("\n\n")
            }
        }
        Err(e) => format!("Search failed: {}", e),
    }
}

/// Fetch a URL and return cleaned text content.
async fn web_fetch(url_str: &str) -> String {
    let url = if !url_str.starts_with("http") { format!("https://{}", url_str) } else { url_str.to_string() };
    let client = reqwest::Client::new();
    match client.get(&url).headers(browser_headers()).timeout(std::time::Duration::from_secs(30)).send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let content_type = resp.headers().get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("").to_string();
            let body = resp.text().await.unwrap_or_default();

            let text = if content_type.contains("text/html") {
                // Strip HTML, extract meaningful text
                let cleaned = strip_html(&body);
                // Remove excessive blank lines
                let lines: Vec<&str> = cleaned.lines().filter(|l| !l.trim().is_empty()).collect();
                lines.join("\n")
            } else {
                body
            };

            let text = text.chars().take(8000).collect::<String>();
            if text.len() >= 8000 {
                format!("HTTP {} — {}\n\n{}... [truncated]", status, url, text)
            } else {
                format!("HTTP {} — {}\n\n{}", status, url, text)
            }
        }
        Err(e) => format!("Fetch failed: {}", e),
    }
}

fn strip_html(html: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(c),
            _ => {}
        }
    }
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn urlencoding(s: &str) -> String {
    s.chars().map(|c| match c {
        ' ' => "+".to_string(),
        c if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' => c.to_string(),
        c => format!("%{:02X}", c as u8),
    }).collect()
}

/// Simple recursive glob — supports * (match any chars in filename) and ** (match any dirs).
fn simple_glob(base: &std::path::Path, pattern: &str) -> Vec<String> {
    let mut results = Vec::new();
    let _ = walk_glob(base, base, pattern, &mut results);
    results.sort();
    results
}

fn walk_glob(root: &std::path::Path, dir: &std::path::Path, pattern: &str, results: &mut Vec<String>) -> Result<(), ()> {
    let entries = std::fs::read_dir(dir).map_err(|_| ())?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if name.starts_with('.') || name == "target" || name == "node_modules" { continue; }
        if path.is_dir() {
            walk_glob(root, &path, pattern, results)?;
        } else {
            let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().to_string();
            // Simple * matching
            if simple_match(&rel, pattern) {
                results.push(rel);
            }
        }
    }
    Ok(())
}

fn simple_match(name: &str, pattern: &str) -> bool {
    if pattern == "*" || pattern == "**" { return true; }
    if !pattern.contains('*') { return name.contains(pattern); }
    // **/ — match any directory prefix (check this before prefix*/suffix*)
    if let Some(suffix) = pattern.strip_prefix("**/") {
        return name.ends_with(suffix) || name.contains(&format!("/{}", suffix));
    }
    // <prefix>/**
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return name.starts_with(prefix);
    }
    // *.ext (check before prefix* to avoid false match)
    if let Some(ext) = pattern.strip_prefix("*.") {
        return name.ends_with(&format!(".{}", ext));
    }
    // prefix* (strip trailing *)
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    // *suffix (strip leading *)
    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }
    false
}
