use std::io::Read;
use std::path::{Path, PathBuf};

const FILE_REFERENCE_MAX_FILES: usize = 6;
const FILE_REFERENCE_MAX_BYTES: u64 = 80_000;
const FILE_REFERENCE_TOTAL_CHAR_LIMIT: usize = 120_000;

/// Build file-reference context string from @-mentions in the input text.
pub fn build_file_reference_context(working_dir: &Path, text: &str) -> Option<String> {
    build_file_reference_context_with_paths(working_dir, text, &[])
}

/// Build file-reference context with explicit additional paths.
pub fn build_file_reference_context_with_paths(
    working_dir: &Path,
    text: &str,
    explicit_references: &[String],
) -> Option<String> {
    let references = collect_file_reference_paths(text, explicit_references);
    if references.is_empty() {
        return None;
    }

    let workspace = working_dir.canonicalize().ok()?;
    let mut total_chars = 0usize;
    let mut parts = Vec::new();
    for reference in references.iter().take(FILE_REFERENCE_MAX_FILES) {
        let Some(item) = read_file_reference(&workspace, reference) else {
            continue;
        };
        let mut body = item.content.trim().to_string();
        if total_chars + body.chars().count() > FILE_REFERENCE_TOTAL_CHAR_LIMIT {
            let remaining = FILE_REFERENCE_TOTAL_CHAR_LIMIT.saturating_sub(total_chars);
            if remaining == 0 {
                break;
            }
            body = take_chars(&body, remaining);
            body.push_str("\n\n[truncated selected file context: total limit reached]");
        }
        total_chars += body.chars().count();
        parts.push(format!(
            "### @{}\nPath: {}\n\n```text\n{}\n```",
            item.display_path,
            item.display_path,
            sanitize_context_fence(&body)
        ));
        if total_chars >= FILE_REFERENCE_TOTAL_CHAR_LIMIT {
            break;
        }
    }

    if parts.is_empty() {
        return None;
    }

    Some(format!(
        "## User-selected file references\n\n\
        These files were explicitly selected by the user for this turn. Treat them as read-only project context.\n\n{}",
        parts.join("\n\n---\n\n")
    ))
}

/// Resolve file-reference paths that exist inside the workspace.
pub fn resolved_file_reference_paths_for_turn(
    working_dir: &Path,
    text: &str,
    explicit_references: &[String],
) -> Vec<String> {
    let references = collect_file_reference_paths(text, explicit_references);
    if references.is_empty() {
        return Vec::new();
    }

    let Some(workspace) = working_dir.canonicalize().ok() else {
        return Vec::new();
    };
    let mut resolved = Vec::new();
    for reference in references {
        let Some(item) = read_file_reference(&workspace, &reference) else {
            continue;
        };
        if !resolved.contains(&item.display_path) {
            resolved.push(item.display_path);
        }
    }
    resolved
}

fn collect_file_reference_paths(text: &str, explicit_references: &[String]) -> Vec<String> {
    let mut refs = extract_file_reference_paths(text);
    for raw in explicit_references {
        if let Some(reference) = normalize_file_reference(raw) {
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
    }
    refs
}

fn extract_file_reference_paths(text: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut chars = text.char_indices().peekable();

    while let Some((index, ch)) = chars.next() {
        if ch != '@' || is_embedded_at_sign(text, index) {
            continue;
        }

        let mut end = index + ch.len_utf8();
        while let Some(&(next_index, next_ch)) = chars.peek() {
            if is_file_reference_boundary(next_ch) {
                break;
            }
            chars.next();
            end = next_index + next_ch.len_utf8();
        }

        let raw = text[index + ch.len_utf8()..end].trim();
        if let Some(reference) = normalize_file_reference(raw) {
            if !refs.contains(&reference) {
                refs.push(reference);
            }
        }
    }

    refs
}

fn is_embedded_at_sign(text: &str, at_index: usize) -> bool {
    text[..at_index]
        .chars()
        .next_back()
        .is_some_and(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
}

fn is_file_reference_boundary(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            '@' | ','
                | ';'
                | '"'
                | '\''
                | '`'
                | '<'
                | '>'
                | '('
                | ')'
                | '['
                | ']'
                | '{'
                | '}'
                | '，'
                | '。'
                | '、'
                | '；'
                | '：'
                | '！'
                | '？'
                | '（'
                | '）'
                | '【'
                | '】'
                | '《'
                | '》'
        )
}

fn normalize_file_reference(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_matches(|ch: char| {
        matches!(
            ch,
            '.' | ',' | ';' | ':' | '，' | '。' | '；' | '：' | ')' | '）' | ']' | '】'
        )
    });
    if trimmed.is_empty() || trimmed == "@" || trimmed.len() > 240 {
        return None;
    }

    let without_line = strip_line_suffix(trimmed);
    if without_line.is_empty() || without_line.contains('\\') {
        return None;
    }

    Some(without_line.trim_start_matches("./").to_string())
}

fn strip_line_suffix(reference: &str) -> &str {
    if let Some((path, suffix)) = reference.rsplit_once(':') {
        if !path.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit()) {
            return path;
        }
    }
    reference
}

struct FileReferenceContextItem {
    display_path: String,
    content: String,
}

fn read_file_reference(workspace: &Path, reference: &str) -> Option<FileReferenceContextItem> {
    let full_path = resolve_file_reference_path(workspace, reference)?;
    let metadata = std::fs::metadata(&full_path).ok()?;
    if !metadata.is_file() {
        return None;
    }

    let mut file = std::fs::File::open(&full_path).ok()?;
    let bytes_to_read = metadata.len().min(FILE_REFERENCE_MAX_BYTES);
    let mut bytes = Vec::with_capacity(bytes_to_read as usize);
    file.by_ref()
        .take(FILE_REFERENCE_MAX_BYTES)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.contains(&0) {
        return None;
    }

    let mut content = String::from_utf8(bytes).ok()?;
    if metadata.len() > FILE_REFERENCE_MAX_BYTES {
        content.push_str(&format!(
            "\n\n[truncated selected file: {} bytes omitted]",
            metadata.len().saturating_sub(FILE_REFERENCE_MAX_BYTES)
        ));
    }

    let display_path = full_path
        .strip_prefix(workspace)
        .unwrap_or(&full_path)
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");

    Some(FileReferenceContextItem {
        display_path,
        content,
    })
}

fn resolve_file_reference_path(workspace: &Path, reference: &str) -> Option<PathBuf> {
    let requested = reference.trim();
    if requested.is_empty() {
        return None;
    }

    let candidate = if let Some(src_path) = requested.strip_prefix("@/") {
        workspace.join("src").join(src_path)
    } else if Path::new(requested).is_absolute() {
        return None;
    } else {
        workspace.join(requested)
    };
    let canonical = candidate.canonicalize().ok()?;
    if !canonical.starts_with(workspace) {
        return None;
    }
    Some(canonical)
}

fn sanitize_context_fence(text: &str) -> String {
    text.replace("```", "` ` `")
}

fn take_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_user_selected_file_references_without_emails() {
        let refs = extract_file_reference_paths(
            "请看 @src/App.tsx、@package.json 和 me@test.com；不要把裸 @ 当成文件。",
        );

        assert_eq!(refs, vec!["src/App.tsx", "package.json"]);
    }

    #[test]
    fn file_reference_context_reads_workspace_files_only() {
        let nonce = uuid::Uuid::now_v7();
        let workspace = std::env::temp_dir().join(format!("forge-file-context-{nonce}"));
        let outside = std::env::temp_dir().join(format!("forge-outside-{nonce}.txt"));
        std::fs::create_dir_all(workspace.join("src")).expect("workspace");
        std::fs::write(workspace.join("src/app.ts"), "export const answer = 42;")
            .expect("workspace file");
        std::fs::write(&outside, "outside secret").expect("outside file");

        let context = build_file_reference_context(
            &workspace,
            &format!("请参考 @src/app.ts，也不要读 @{}", outside.display()),
        )
        .expect("context");

        assert!(context.contains("User-selected file references"));
        assert!(context.contains("@src/app.ts"));
        assert!(context.contains("export const answer = 42;"));
        assert!(!context.contains("outside secret"));

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_file(&outside);
    }

    #[test]
    fn file_reference_context_accepts_structured_paths() {
        let nonce = uuid::Uuid::now_v7();
        let workspace = std::env::temp_dir().join(format!("forge-structured-file-context-{nonce}"));
        std::fs::create_dir_all(workspace.join("src")).expect("workspace");
        std::fs::write(
            workspace.join("src/app.ts"),
            "export const source = 'structured';",
        )
        .expect("workspace file");

        let context = build_file_reference_context_with_paths(
            &workspace,
            "请参考这个文件",
            &["src/app.ts".to_string()],
        )
        .expect("context");

        assert!(context.contains("@src/app.ts"));
        assert!(context.contains("structured"));

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn turn_file_references_keep_only_resolved_workspace_files() {
        let nonce = uuid::Uuid::now_v7();
        let workspace = std::env::temp_dir().join(format!("forge-turn-file-refs-{nonce}"));
        let outside = std::env::temp_dir().join(format!("forge-turn-outside-{nonce}.txt"));
        std::fs::create_dir_all(workspace.join("src")).expect("workspace");
        std::fs::write(workspace.join("src/app.ts"), "export const source = 'ok';")
            .expect("workspace file");
        std::fs::write(&outside, "outside secret").expect("outside file");

        let references = resolved_file_reference_paths_for_turn(
            &workspace,
            &format!("请看 @src/app.ts 和 @{}", outside.display()),
            &["src/missing.ts".to_string(), "src/app.ts".to_string()],
        );

        assert_eq!(references, vec!["src/app.ts"]);

        let _ = std::fs::remove_dir_all(&workspace);
        let _ = std::fs::remove_file(&outside);
    }
}
