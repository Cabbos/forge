use std::path::{Path, PathBuf};

pub(crate) fn resolve_workspace_file_path(
    working_dir: &Path,
    path: &str,
) -> Result<PathBuf, String> {
    let requested_path = path.trim();
    if requested_path.is_empty() {
        return Err("请选择当前项目内的文件。".to_string());
    }

    let candidate = if let Some(src_path) = requested_path.strip_prefix("@/") {
        working_dir.join("src").join(src_path)
    } else if Path::new(requested_path).is_absolute() {
        PathBuf::from(requested_path)
    } else {
        working_dir.join(requested_path)
    };

    let workspace_root = canonical_or_lexical_path(working_dir);
    let resolved = canonical_or_lexical_path(&candidate);
    if !resolved.starts_with(&workspace_root) {
        return Err("路径不在当前项目内，请选择当前项目里的文件。".to_string());
    }

    Ok(resolved)
}

fn canonical_or_lexical_path(path: &Path) -> PathBuf {
    path.canonicalize()
        .unwrap_or_else(|_| lexical_normalize_path(path))
}

fn lexical_normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(target_os = "macos")]
pub(crate) fn open_file_macos(path_str: &str, line: Option<u32>) -> Result<(), String> {
    let location = if let Some(l) = line {
        format!("{}:{}", path_str, l)
    } else {
        path_str.to_string()
    };

    let mut attempts: Vec<(String, Vec<String>)> = Vec::new();

    for cli in [
        "code",
        "/usr/local/bin/code",
        "/opt/homebrew/bin/code",
        "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
        "cursor",
        "/usr/local/bin/cursor",
        "/opt/homebrew/bin/cursor",
        "/Applications/Cursor.app/Contents/Resources/app/bin/cursor",
    ] {
        attempts.push((cli.to_string(), vec!["-g".into(), location.clone()]));
    }

    for env_name in ["VISUAL", "EDITOR"] {
        if let Ok(editor) = std::env::var(env_name) {
            let editor = editor.trim();
            if editor.is_empty() {
                continue;
            }
            let mut parts = editor.split_whitespace();
            if let Some(program) = parts.next() {
                let mut args = parts.map(str::to_string).collect::<Vec<_>>();
                args.push("-g".into());
                args.push(location.clone());
                attempts.push((program.to_string(), args));
            }
        }
    }

    let mut app_names = vec![
        "Visual Studio Code".to_string(),
        "Code".to_string(),
        "Cursor".to_string(),
    ];
    if let Ok(editor) = std::env::var("EDITOR") {
        let editor = editor.trim();
        if !editor.is_empty() && !app_names.iter().any(|name| name == editor) {
            app_names.insert(0, editor.to_string());
        }
    }

    for app_name in app_names {
        attempts.push((
            "open".to_string(),
            vec![
                "-a".into(),
                app_name,
                "--args".into(),
                "-g".into(),
                location.clone(),
            ],
        ));
    }

    attempts.push(("open".to_string(), vec![path_str.to_string()]));

    let mut errors = Vec::new();
    for (program, args) in attempts {
        match run_open_command(&program, &args) {
            Ok(()) => {
                crate::app_log!(
                    "INFO",
                    "[open_file] opened via {} {}",
                    program,
                    args.join(" ")
                );
                return Ok(());
            }
            Err(error) => errors.push(error),
        }
    }

    let message = format!("Failed to open file: {}", errors.join(" | "));
    crate::app_log!("WARN", "[open_file] {}", message);
    Err(message)
}

#[cfg(target_os = "macos")]
fn run_open_command(program: &str, args: &[String]) -> Result<(), String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("{} {} ({})", program, args.join(" "), e))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let detail = if stderr.is_empty() {
        format!("exit status {}", output.status)
    } else {
        stderr
    };
    Err(format!("{} {} ({})", program, args.join(" "), detail))
}
