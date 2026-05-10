use std::path::PathBuf;

/// Result of running a shell command.
#[derive(Debug, Clone)]
pub struct ShellResult {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Shell executor that runs commands in the working directory.
pub struct ShellExecutor {
    working_dir: PathBuf,
}

impl ShellExecutor {
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    /// Execute a shell command and capture stdout/stderr.
    /// Uses spawn_blocking + tokio timeout to prevent hangs.
    /// Truncates output to 100KB.
    pub async fn execute(&self, command: &str) -> Result<ShellResult, String> {
        let shell = if cfg!(target_os = "windows") {
            ("cmd.exe", "/C")
        } else {
            ("/bin/sh", "-c")
        };

        let working_dir = self.working_dir.clone();
        let cmd_name = shell.0.to_string();
        let cmd_arg = shell.1.to_string();
        let cmd_str = command.to_string();

        let handle = tokio::task::spawn_blocking(move || {
            let output = std::process::Command::new(&cmd_name)
                .arg(&cmd_arg)
                .arg(&cmd_str)
                .current_dir(&working_dir)
                .output()
                .map_err(|e| format!("Failed to execute command: {}", e))?;

            let exit_code = output.status.code().unwrap_or(-1);
            let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

            // Truncate output to 100KB
            const MAX_OUTPUT: usize = 100 * 1024;
            if stdout.len() > MAX_OUTPUT {
                let truncated = stdout.len() - MAX_OUTPUT;
                stdout.truncate(MAX_OUTPUT);
                stdout.push_str(&format!("\n... (truncated {} bytes)", truncated));
            }
            if stderr.len() > MAX_OUTPUT {
                let truncated = stderr.len() - MAX_OUTPUT;
                stderr.truncate(MAX_OUTPUT);
                stderr.push_str(&format!("\n... (truncated {} bytes)", truncated));
            }

            Ok(ShellResult {
                command: cmd_str,
                stdout,
                stderr,
                exit_code,
            }) as Result<ShellResult, String>
        });

        tokio::time::timeout(std::time::Duration::from_secs(30), handle)
            .await
            .map_err(|_| "Shell command timed out (30s)".to_string())?
            .map_err(|e| format!("Task panicked: {}", e))
            .and_then(|inner| inner)
    }
}
