use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Notify;

use crate::consts::{SHELL_COMMAND_TIMEOUT, SHELL_OUTPUT_LIMIT};
use crate::process_runner::{run_captured, run_streaming, ProcessRunOptions, ProcessSpec};

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
        let output = run_captured(
            ProcessSpec::shell(command, self.working_dir.clone()),
            ProcessRunOptions {
                timeout: SHELL_COMMAND_TIMEOUT,
                cancel: None,
                output_limit: SHELL_OUTPUT_LIMIT,
            },
        )
        .await?;

        if output.timed_out {
            return Err("Shell command timed out (30s)".to_string());
        }

        Ok(ShellResult {
            command: command.to_string(),
            stdout: output.stdout,
            stderr: output.stderr,
            exit_code: output.exit_code.unwrap_or(-1),
        })
    }

    /// Execute a shell command and stream output line-by-line via a callback.
    /// Kills the process group on timeout (30s).
    /// Returns the exit code on success.
    pub async fn execute_streaming<F>(&self, command: &str, mut on_line: F) -> Result<i32, String>
    where
        F: FnMut(String, bool) + Send + 'static,
    {
        self.execute_streaming_inner(command, None, move |line, is_stderr| {
            on_line(line, is_stderr)
        })
        .await
    }

    /// Execute a shell command and stop the process group when the cancel token fires.
    pub async fn execute_streaming_with_cancel<F>(
        &self,
        command: &str,
        cancel: Arc<Notify>,
        mut on_line: F,
    ) -> Result<i32, String>
    where
        F: FnMut(String, bool) + Send + 'static,
    {
        self.execute_streaming_inner(command, Some(cancel), move |line, is_stderr| {
            on_line(line, is_stderr)
        })
        .await
    }

    async fn execute_streaming_inner<F>(
        &self,
        command: &str,
        cancel: Option<Arc<Notify>>,
        on_line: F,
    ) -> Result<i32, String>
    where
        F: FnMut(String, bool) + Send + 'static,
    {
        let output = run_streaming(
            ProcessSpec::shell(command, self.working_dir.clone()),
            ProcessRunOptions {
                timeout: SHELL_COMMAND_TIMEOUT,
                cancel,
                output_limit: SHELL_OUTPUT_LIMIT,
            },
            on_line,
        )
        .await?;

        if output.cancelled {
            return Err("Shell command cancelled".to_string());
        }

        if output.timed_out {
            return Err("Shell command timed out (30s)".to_string());
        }

        Ok(output.exit_code.unwrap_or(-1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Notify;

    #[tokio::test]
    #[cfg(unix)]
    async fn execute_streaming_with_cancel_kills_process_group() {
        let root = std::env::temp_dir().join(format!(
            "forge-shell-cancel-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).expect("temp root");
        let marker = root.join("marker");
        let executor = ShellExecutor::new(root.clone());
        let cancel = Arc::new(Notify::new());
        let cancel_for_task = cancel.clone();

        let task = tokio::spawn(async move {
            executor
                .execute_streaming_with_cancel(
                    "sleep 5; echo should-not-run > marker",
                    cancel_for_task,
                    |_line, _is_stderr| {},
                )
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        cancel.notify_waiters();

        let result = tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .expect("cancel should finish quickly")
            .expect("join task");

        assert_eq!(result, Err("Shell command cancelled".to_string()));
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        assert!(!marker.exists());

        let _ = std::fs::remove_dir_all(root);
    }
}
