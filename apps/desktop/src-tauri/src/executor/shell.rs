use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;

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

    /// Execute a shell command and stream output line-by-line via a callback.
    /// `on_line(line, is_stderr)` is called for each line as it arrives
    /// from interleaved stdout/stderr reader threads.
    /// Uses spawn_blocking + tokio timeout to prevent hangs (30s).
    /// Returns the exit code on success.
    pub async fn execute_streaming<F>(
        &self,
        command: &str,
        mut on_line: F,
    ) -> Result<i32, String>
    where
        F: FnMut(String, bool) + Send + 'static,
    {
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
            let mut child = Command::new(&cmd_name)
                .arg(&cmd_arg)
                .arg(&cmd_str)
                .current_dir(&working_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to execute command: {}", e))?;

            let stdout = child.stdout.take().expect("stdout pipe missing");
            let stderr = child.stderr.take().expect("stderr pipe missing");

            let (tx, rx) = mpsc::channel::<(String, bool)>();

            // Spawn thread to read stdout lines
            let tx_out = tx.clone();
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stdout);
                let mut buf = Vec::new();
                loop {
                    buf.clear();
                    match reader.read_until(b'\n', &mut buf) {
                        Ok(0) => break,
                        Ok(_) => {
                            if buf.ends_with(b"\n") {
                                buf.pop();
                                if buf.ends_with(b"\r") {
                                    buf.pop();
                                }
                            }
                            let line = String::from_utf8_lossy(&buf).to_string();
                            let _ = tx_out.send((line, false));
                        }
                        Err(_) => break,
                    }
                }
            });

            // Spawn thread to read stderr lines (interleaved via channel)
            std::thread::spawn(move || {
                let mut reader = BufReader::new(stderr);
                let mut buf = Vec::new();
                loop {
                    buf.clear();
                    match reader.read_until(b'\n', &mut buf) {
                        Ok(0) => break,
                        Ok(_) => {
                            if buf.ends_with(b"\n") {
                                buf.pop();
                                if buf.ends_with(b"\r") {
                                    buf.pop();
                                }
                            }
                            let line = String::from_utf8_lossy(&buf).to_string();
                            let _ = tx.send((line, true));
                        }
                        Err(_) => break,
                    }
                }
            });

            // Process lines as they arrive (interleaved stdout/stderr)
            for (line, is_stderr) in rx {
                on_line(line, is_stderr);
            }

            let status = child
                .wait()
                .map_err(|e| format!("Failed to wait on child process: {}", e))?;
            Ok(status.code().unwrap_or(-1)) as Result<i32, String>
        });

        tokio::time::timeout(std::time::Duration::from_secs(30), handle)
            .await
            .map_err(|_| "Shell command timed out (30s)".to_string())?
            .map_err(|e| format!("Task panicked: {}", e))
            .and_then(|inner| inner)
    }

}
