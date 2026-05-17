use std::path::{Path, PathBuf};

const MAX_TEXT_FILE_BYTES: u64 = 2_000_000;
const MAX_WRITE_FILE_BYTES: usize = 2_000_000;
const MAX_LIST_ENTRIES: usize = 300;
const MAX_SEARCH_MATCHES: usize = 200;

/// Result of reading a file.
#[derive(Debug, Clone)]
pub struct FileReadResult {
    pub path: String,
    pub content: String,
    pub line_count: usize,
}

/// Result of writing a file, with diff data for the frontend.
#[derive(Debug, Clone)]
pub struct FileWriteResult {
    pub path: String,
    pub old_content: String,
    pub new_content: String,
}

/// A search match in a file.
#[derive(Debug, Clone)]
pub struct SearchMatch {
    pub file_path: String,
    pub line_number: usize,
    pub line_content: String,
}

/// File I/O executor, scoped to a working directory.
pub struct FileExecutor {
    working_dir: PathBuf,
}

impl FileExecutor {
    pub fn working_dir(&self) -> &PathBuf {
        &self.working_dir
    }
}

impl FileExecutor {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    /// Read a file. Path can be absolute or relative to working_dir.
    pub fn read_file(&self, path: &str) -> Result<FileReadResult, String> {
        let resolved = self.resolve(path)?;
        ensure_plain_text_size(&resolved)?;
        let content =
            std::fs::read_to_string(&resolved).map_err(|e| format!("Read error: {}", e))?;
        let line_count = content.lines().count();
        Ok(FileReadResult {
            path: resolved.to_string_lossy().to_string(),
            content,
            line_count,
        })
    }

    /// Write content to a file. Returns old and new content for diff display.
    pub fn write_file(&self, path: &str, content: &str) -> Result<FileWriteResult, String> {
        let resolved = self.resolve(path)?;
        if content.len() > MAX_WRITE_FILE_BYTES {
            return Err(format!(
                "Refusing to write {} bytes through the AI file tool; limit is {} bytes.",
                content.len(),
                MAX_WRITE_FILE_BYTES
            ));
        }

        // Ensure parent directory exists
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent dir: {}", e))?;
        }

        let old_content = if resolved.exists() {
            ensure_plain_text_size(&resolved)?;
            std::fs::read_to_string(&resolved).unwrap_or_default()
        } else {
            String::new()
        };

        std::fs::write(&resolved, content).map_err(|e| format!("Write error: {}", e))?;

        Ok(FileWriteResult {
            path: resolved.to_string_lossy().to_string(),
            old_content,
            new_content: content.to_string(),
        })
    }

    /// Search for a pattern in files under the working directory.
    /// Only searches files with common text extensions to avoid binary files.
    pub fn search_files(&self, pattern: &str) -> Result<Vec<SearchMatch>, String> {
        let mut results = Vec::new();
        let regex =
            regex::Regex::new(pattern).map_err(|e| format!("Invalid regex pattern: {}", e))?;

        self.walk_files(&self.working_dir.clone(), &regex, &mut results)?;
        Ok(results)
    }

    fn walk_files(
        &self,
        dir: &Path,
        regex: &regex::Regex,
        results: &mut Vec<SearchMatch>,
    ) -> Result<(), String> {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()),
        };

        for entry in entries.flatten() {
            if results.len() >= MAX_SEARCH_MATCHES {
                return Ok(());
            }
            let path = entry.path();
            if path.is_dir() {
                // Skip common non-source directories
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                if name.starts_with('.') || name == "target" || name == "node_modules" {
                    continue;
                }
                self.walk_files(&path, regex, results)?;
            } else if path.is_file() {
                if is_oversized_file(&path) {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    for (i, line) in content.lines().enumerate() {
                        if regex.is_match(line) {
                            results.push(SearchMatch {
                                file_path: path.to_string_lossy().to_string(),
                                line_number: i + 1,
                                line_content: line.to_string(),
                            });
                            if results.len() >= MAX_SEARCH_MATCHES {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// List files and directories at the given path.
    pub fn list_directory(&self, path: &str) -> Result<String, String> {
        let dir = if path.is_empty() {
            self.working_dir.clone()
        } else {
            self.resolve(path)?
        };
        let mut entries: Vec<String> = Vec::new();
        let mut truncated = false;
        let iter = std::fs::read_dir(&dir).map_err(|e| format!("Cannot read directory: {}", e))?;
        for entry in iter.flatten() {
            if entries.len() >= MAX_LIST_ENTRIES {
                truncated = true;
                break;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let ft = entry
                .file_type()
                .map(|t| if t.is_dir() { "/" } else { "" })
                .unwrap_or("");
            entries.push(format!("{}{}", name, ft));
        }
        entries.sort();
        if truncated {
            entries.push(format!(
                "... truncated to first {} entries",
                MAX_LIST_ENTRIES
            ));
        }
        Ok(entries.join("\n"))
    }

    /// Edit a file by replacing old_string with new_string.
    /// Returns the updated content or an error if old_string not found.
    pub fn edit_file(&self, path: &str, old_str: &str, new_str: &str) -> Result<String, String> {
        let resolved = self.resolve(path)?;
        ensure_plain_text_size(&resolved)?;
        if new_str.len() > MAX_WRITE_FILE_BYTES {
            return Err(format!(
                "Refusing to insert {} bytes through the AI edit tool; limit is {} bytes.",
                new_str.len(),
                MAX_WRITE_FILE_BYTES
            ));
        }
        let content =
            std::fs::read_to_string(&resolved).map_err(|e| format!("Read error: {}", e))?;
        if !content.contains(old_str) {
            return Err("old_string not found in file".to_string());
        }
        let updated = content.replacen(old_str, new_str, 1);
        std::fs::write(&resolved, &updated).map_err(|e| format!("Write error: {}", e))?;
        Ok(format!("File edited: {}", resolved.to_string_lossy()))
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, String> {
        let p = std::path::Path::new(path);
        let resolved = if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.working_dir.join(p)
        };
        // Canonicalize existing path; for new files, canonicalize parent + append filename
        let canonical = resolved.canonicalize().or_else(|_| {
            resolved
                .parent()
                .and_then(|parent| {
                    let parent_canon = parent.canonicalize().ok()?;
                    let filename = resolved.file_name()?;
                    Some(parent_canon.join(filename))
                })
                .ok_or_else(|| format!("Path error: cannot resolve {}", resolved.display()))
        })?;
        let work_canon = self
            .working_dir
            .canonicalize()
            .unwrap_or_else(|_| self.working_dir.clone());
        if !canonical.starts_with(&work_canon) {
            return Err("Access denied: outside working directory".to_string());
        }
        Ok(canonical)
    }
}

fn ensure_plain_text_size(path: &Path) -> Result<(), String> {
    if is_oversized_file(path) {
        return Err(format!(
            "File is too large for the AI file tool: {} (limit {} bytes)",
            path.display(),
            MAX_TEXT_FILE_BYTES
        ));
    }
    Ok(())
}

fn is_oversized_file(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() > MAX_TEXT_FILE_BYTES)
        .unwrap_or(false)
}
