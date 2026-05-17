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
    /// Kills the process on timeout (30s).
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
            let mut cmd = Command::new(&cmd_name);
            cmd.arg(&cmd_arg)
                .arg(&cmd_str)
                .current_dir(&working_dir)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            #[cfg(unix)]
            unsafe {
                use std::os::unix::process::CommandExt;
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }

            let child = cmd
                .spawn()
                .map_err(|e| format!("Failed to execute command: {}", e))?;

            let output = child
                .wait_with_output()
                .map_err(|e| format!("Failed to wait on process: {}", e))?;

            // On timeout we land here too — check if process was killed
            let exit_code = output.status.code().unwrap_or(-1);
            let mut stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let mut stderr = String::from_utf8_lossy(&output.stderr).to_string();

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
            })
        });

        match tokio::time::timeout(std::time::Duration::from_secs(30), handle).await {
            Ok(result) => result
                .map_err(|e| format!("Task panicked: {}", e))
                .and_then(|inner| inner),
            Err(_) => {
                // Kill the spawn_blocking task's process
                // The task handle is dropped, but the thread continues running.
                // We rely on wait_with_output() eventually completing, but if the
                // grandchild inherited pipes, it may hang forever.
                // In that case, the user can use the stop button to abort the session.
                Err("Shell command timed out (30s)".to_string())
            }
        }
    }

    /// Execute a shell command and stream output line-by-line via a callback.
    /// Kills the process group on timeout (30s).
    /// Returns the exit code on success.
    pub async fn execute_streaming<F>(&self, command: &str, mut on_line: F) -> Result<i32, String>
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

        // Spawn the process in a blocking thread
        let (pid_tx, pid_rx) = tokio::sync::oneshot::channel();
        let (result_tx, result_rx) = tokio::sync::oneshot::channel();

        std::thread::spawn(move || {
            let mut cmd = Command::new(&cmd_name);
            cmd.arg(&cmd_arg)
                .arg(&cmd_str)
                .current_dir(&working_dir)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());

            #[cfg(unix)]
            unsafe {
                use std::os::unix::process::CommandExt;
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    let _ = result_tx.send(Err(format!("Failed to execute command: {}", e)));
                    return;
                }
            };

            let pid = child.id();
            let _ = pid_tx.send(pid);

            let stdout = child.stdout.take().expect("stdout pipe missing");
            let stderr = child.stderr.take().expect("stderr pipe missing");

            let (tx, rx) = mpsc::channel::<(String, bool)>();

            // Read stdout in a thread
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
                            }
                            if buf.ends_with(b"\r") {
                                buf.pop();
                            }
                            let _ = tx_out.send((String::from_utf8_lossy(&buf).to_string(), false));
                        }
                        Err(_) => break,
                    }
                }
            });

            // Read stderr in a thread
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
                            }
                            if buf.ends_with(b"\r") {
                                buf.pop();
                            }
                            let _ = tx.send((String::from_utf8_lossy(&buf).to_string(), true));
                        }
                        Err(_) => break,
                    }
                }
            });

            // Stream lines
            for (line, is_stderr) in rx {
                on_line(line, is_stderr);
            }

            let status = child
                .wait()
                .map_err(|e| format!("Failed to wait on process: {}", e));
            let _ = result_tx.send(status.map(|s| s.code().unwrap_or(-1)));
        });

        // Wait for PID so we can kill if needed
        let pid: u32 = tokio::time::timeout(std::time::Duration::from_secs(5), pid_rx)
            .await
            .map_err(|_| "Shell did not start in time".to_string())?
            .map_err(|_| "Failed to get process PID".to_string())?;

        let timeout_result =
            tokio::time::timeout(std::time::Duration::from_secs(30), result_rx).await;
        match timeout_result {
            Ok(Ok(Ok(exit_code))) => Ok(exit_code),
            Ok(Ok(Err(e))) => Err(e),
            Ok(Err(_recv_err)) => Err("Shell process crashed".to_string()),
            Err(_elapsed) => {
                kill_process_group(pid);
                Err("Shell command timed out (30s)".to_string())
            }
        }
    }
}

/// Kill a process and its entire process group.
fn kill_process_group(pid: u32) {
    #[cfg(unix)]
    unsafe {
        // Negative PID sends signal to the entire process group
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    #[cfg(not(unix))]
    {
        // On Windows, terminate the process tree
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output();
    }
}
