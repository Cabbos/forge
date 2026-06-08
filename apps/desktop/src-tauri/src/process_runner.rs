use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Notify};

use crate::consts::{PROCESS_LINE_DRAIN_INTERVAL, PROCESS_SHUTDOWN_GRACE};

#[derive(Debug, Clone)]
pub(crate) struct ProcessSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub envs: Vec<(String, String)>,
}

impl ProcessSpec {
    pub(crate) fn new(
        program: impl Into<String>,
        args: impl IntoIterator<Item = impl Into<String>>,
        cwd: PathBuf,
    ) -> Self {
        Self {
            program: program.into(),
            args: args.into_iter().map(Into::into).collect(),
            cwd,
            envs: Vec::new(),
        }
    }

    pub(crate) fn shell(command: impl Into<String>, cwd: PathBuf) -> Self {
        if cfg!(target_os = "windows") {
            Self::new("cmd.exe", ["/C".to_string(), command.into()], cwd)
        } else {
            Self::new("/bin/sh", ["-c".to_string(), command.into()], cwd)
        }
    }

    pub(crate) fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.envs.push((key.into(), value.into()));
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ProcessRunOptions {
    pub timeout: Duration,
    pub cancel: Option<Arc<Notify>>,
    pub output_limit: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ProcessOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub cancelled: bool,
}

pub(crate) async fn run_captured(
    spec: ProcessSpec,
    options: ProcessRunOptions,
) -> Result<ProcessOutput, String> {
    run_streaming(spec, options, |_line, _is_stderr| {}).await
}

pub(crate) async fn run_streaming<F>(
    spec: ProcessSpec,
    options: ProcessRunOptions,
    mut on_line: F,
) -> Result<ProcessOutput, String>
where
    F: FnMut(String, bool) + Send + 'static,
{
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (key, value) in &spec.envs {
        command.env(key, value);
    }
    configure_command_process_group(&mut command);

    let mut child = command
        .spawn()
        .map_err(|error| format!("Failed to execute command: {error}"))?;
    let pid = child
        .id()
        .ok_or_else(|| "Failed to get process PID".to_string())?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<(String, bool)>();
    if let Some(stdout) = stdout {
        tokio::spawn(forward_lines(stdout, false, line_tx.clone()));
    }
    if let Some(stderr) = stderr {
        tokio::spawn(forward_lines(stderr, true, line_tx.clone()));
    }
    drop(line_tx);

    let mut wait_task = tokio::spawn(async move {
        child
            .wait()
            .await
            .map(|status| status.code().unwrap_or(-1))
            .map_err(|error| format!("Failed to wait on process: {error}"))
    });

    let timeout = tokio::time::sleep(options.timeout);
    tokio::pin!(timeout);

    let mut output = ProcessOutput::default();
    loop {
        tokio::select! {
            line = line_rx.recv() => {
                if let Some((line, is_stderr)) = line {
                    append_line_capped(
                        if is_stderr { &mut output.stderr } else { &mut output.stdout },
                        &line,
                        options.output_limit,
                    );
                    on_line(line, is_stderr);
                } else if wait_task.is_finished() {
                    break;
                }
            }
            wait_result = &mut wait_task => {
                output.exit_code = Some(wait_result.map_err(|error| format!("Process task failed: {error}"))??);
                break;
            }
            _ = &mut timeout => {
                if let Err(error) = kill_process_group(pid) {
                    crate::app_log!("WARN", "[process_runner] timeout kill failed: {error}");
                }
                output.timed_out = true;
                finish_after_kill(wait_task).await;
                break;
            }
            _ = notified(options.cancel.as_ref()), if options.cancel.is_some() => {
                if let Err(error) = kill_process_group(pid) {
                    crate::app_log!("WARN", "[process_runner] cancel kill failed: {error}");
                }
                output.cancelled = true;
                finish_after_kill(wait_task).await;
                break;
            }
        }
    }

    drain_lines(
        &mut line_rx,
        &mut output,
        options.output_limit,
        &mut on_line,
    )
    .await;
    Ok(output)
}

pub(crate) fn configure_command_process_group(command: &mut Command) {
    #[cfg(unix)]
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

pub(crate) async fn kill_child_process_group(child: &mut tokio::process::Child) {
    if let Some(pid) = child.id() {
        if let Err(error) = kill_process_group(pid) {
            crate::app_log!(
                "WARN",
                "[process_runner] failed to kill process group: {error}"
            );
        }
    } else {
        let _ = child.kill().await;
    }
}

pub(crate) fn kill_process_group(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let pid_i32 = i32::try_from(pid).map_err(|_| format!("PID out of range: {pid}"))?;
        let rc = unsafe { libc::kill(-pid_i32, libc::SIGKILL) };
        if rc == 0 {
            return Ok(());
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ESRCH) {
            return Ok(());
        }
        Err(format!("failed to kill process group {pid}: {error}"))
    }
    #[cfg(not(unix))]
    {
        let output = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .output()
            .map_err(|error| format!("failed to run taskkill for {pid}: {error}"))?;
        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            Err(if stderr.is_empty() {
                format!("taskkill failed for process {pid}")
            } else {
                stderr
            })
        }
    }
}

async fn notified(cancel: Option<&Arc<Notify>>) {
    if let Some(cancel) = cancel {
        cancel.notified().await;
    } else {
        std::future::pending::<()>().await;
    }
}

async fn finish_after_kill(wait_task: tokio::task::JoinHandle<Result<i32, String>>) {
    let _ = tokio::time::timeout(PROCESS_SHUTDOWN_GRACE, wait_task).await;
}

async fn drain_lines<F>(
    line_rx: &mut mpsc::UnboundedReceiver<(String, bool)>,
    output: &mut ProcessOutput,
    output_limit: usize,
    on_line: &mut F,
) where
    F: FnMut(String, bool),
{
    while let Ok(Some((line, is_stderr))) =
        tokio::time::timeout(PROCESS_LINE_DRAIN_INTERVAL, line_rx.recv()).await
    {
        append_line_capped(
            if is_stderr {
                &mut output.stderr
            } else {
                &mut output.stdout
            },
            &line,
            output_limit,
        );
        on_line(line, is_stderr);
    }
}

async fn forward_lines<R>(pipe: R, is_stderr: bool, tx: mpsc::UnboundedSender<(String, bool)>)
where
    R: AsyncRead + Unpin,
{
    let mut reader = BufReader::new(pipe);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) => break,
            Ok(_) => {
                if buf.ends_with(b"\n") {
                    buf.pop();
                }
                if buf.ends_with(b"\r") {
                    buf.pop();
                }
                let _ = tx.send((String::from_utf8_lossy(&buf).to_string(), is_stderr));
            }
            Err(_) => break,
        }
    }
}

fn append_line_capped(buf: &mut String, line: &str, max_bytes: usize) {
    if max_bytes == 0 || buf.len() >= max_bytes {
        return;
    }
    let extra = line.len() + usize::from(!buf.is_empty());
    if buf.len() + extra <= max_bytes {
        if !buf.is_empty() {
            buf.push('\n');
        }
        buf.push_str(line);
        return;
    }

    let remaining = max_bytes.saturating_sub(buf.len());
    if remaining > 0 {
        if !buf.is_empty() && remaining > 1 {
            buf.push('\n');
            let take = remaining - 1;
            buf.push_str(&line[..line.floor_char_boundary(take)]);
        } else if buf.is_empty() {
            buf.push_str(&line[..line.floor_char_boundary(remaining)]);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::sync::Notify;

    use super::*;

    #[tokio::test]
    #[cfg(unix)]
    async fn captured_timeout_kills_process_group() {
        let root = temp_workspace("captured-timeout");
        let marker = root.join("marker");

        let output = run_captured(
            ProcessSpec::shell("sleep 5; echo should-not-run > marker", root.clone()),
            ProcessRunOptions {
                timeout: Duration::from_millis(150),
                cancel: None,
                output_limit: 1024,
            },
        )
        .await
        .expect("process should start");

        assert!(output.timed_out);
        assert!(!marker.exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn captured_timeout_kills_background_child_processes() {
        let root = temp_workspace("captured-timeout-background");
        let marker = root.join("marker");

        let output = run_captured(
            ProcessSpec::shell("(sleep 1; echo leaked-child > marker) & wait", root.clone()),
            ProcessRunOptions {
                timeout: Duration::from_millis(150),
                cancel: None,
                output_limit: 1024,
            },
        )
        .await
        .expect("process should start");

        assert!(output.timed_out);
        tokio::time::sleep(Duration::from_millis(1_200)).await;
        assert!(
            !marker.exists(),
            "timeout must kill the whole process group, not just the shell parent"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn streaming_cancel_kills_process_group() {
        let root = temp_workspace("streaming-cancel");
        let marker = root.join("marker");
        let cancel = Arc::new(Notify::new());
        let cancel_for_task = cancel.clone();

        let task = tokio::spawn(async move {
            run_streaming(
                ProcessSpec::shell("sleep 5; echo should-not-run > marker", root.clone()),
                ProcessRunOptions {
                    timeout: Duration::from_secs(10),
                    cancel: Some(cancel_for_task),
                    output_limit: 1024,
                },
                |_line, _is_stderr| {},
            )
            .await
            .map(|output| (output, root))
        });

        tokio::time::sleep(Duration::from_millis(150)).await;
        cancel.notify_waiters();

        let (output, root) = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("cancel should complete quickly")
            .expect("join task")
            .expect("process should start");

        assert!(output.cancelled);
        assert!(!marker.exists());

        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn streaming_cancel_kills_background_child_processes_after_they_spawn() {
        let root = temp_workspace("streaming-cancel-background");
        let marker = root.join("marker");
        let spawned = root.join("spawned");
        let cancel = Arc::new(Notify::new());
        let cancel_for_task = cancel.clone();

        let task = tokio::spawn(async move {
            run_streaming(
                ProcessSpec::shell(
                    "(echo spawned > spawned; sleep 1; echo leaked-child > marker) & wait",
                    root.clone(),
                ),
                ProcessRunOptions {
                    timeout: Duration::from_secs(10),
                    cancel: Some(cancel_for_task),
                    output_limit: 1024,
                },
                |_line, _is_stderr| {},
            )
            .await
            .map(|output| (output, root))
        });

        wait_for_path(&spawned, Duration::from_secs(2)).await;
        cancel.notify_waiters();

        let (output, root) = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("cancel should complete quickly")
            .expect("join task")
            .expect("process should start");

        assert!(output.cancelled);
        tokio::time::sleep(Duration::from_millis(1_200)).await;
        assert!(
            !marker.exists(),
            "cancel must kill the background child, not only the shell parent"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(unix)]
    fn kill_process_group_rejects_out_of_range_pid() {
        let error = kill_process_group(u32::MAX).expect_err("oversized pid should be rejected");

        assert!(error.contains("PID out of range"));
    }

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-process-runner-{name}-{}",
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&path).expect("temp workspace");
        path
    }

    async fn wait_for_path(path: &std::path::Path, timeout: Duration) {
        let started = std::time::Instant::now();
        while started.elapsed() < timeout {
            if path.exists() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("timed out waiting for {}", path.display());
    }
}
