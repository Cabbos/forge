use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use tauri::Emitter;

use crate::ipc::workspace::resolve_bound_working_dir;
use crate::state::AppState;

const MIN_TERMINAL_ROWS: u16 = 2;
const MAX_TERMINAL_ROWS: u16 = 200;
const MIN_TERMINAL_COLS: u16 = 10;
const MAX_TERMINAL_COLS: u16 = 400;
const MAX_TERMINAL_INPUT_BYTES: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct WorkspaceTerminalInfo {
    pub terminal_id: String,
    pub task_id: String,
    pub working_dir: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct WorkspaceTerminalOutput {
    pub terminal_id: String,
    pub task_id: String,
    pub chunk: String,
    pub exited: bool,
}

type OutputEmitter = Arc<dyn Fn(WorkspaceTerminalOutput) + Send + Sync + 'static>;

struct WorkspaceTerminal {
    info: WorkspaceTerminalInfo,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl WorkspaceTerminal {
    fn close(mut self) {
        let _ = self.child.kill();
        drop(self.writer);
        let _ = self.child.wait();
    }
}

#[derive(Default)]
pub struct WorkspaceTerminalStore {
    terminals: Mutex<HashMap<String, WorkspaceTerminal>>,
}

impl WorkspaceTerminalStore {
    pub fn start(
        &self,
        task_id: &str,
        working_dir: &Path,
        rows: u16,
        cols: u16,
        emit: OutputEmitter,
    ) -> Result<WorkspaceTerminalInfo, String> {
        let task_id = clean_task_id(task_id)?;
        self.remove_and_close(&task_id)?;

        let size = clamp_pty_size(rows, cols);
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(size)
            .map_err(|error| format!("无法创建临时终端: {error}"))?;
        let mut command = shell_command();
        command.cwd(working_dir);
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");
        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("无法启动临时终端: {error}"))?;
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("无法读取临时终端: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("无法写入临时终端: {error}"))?;
        drop(pair.slave);

        let terminal_id = uuid::Uuid::now_v7().to_string();
        let info = WorkspaceTerminalInfo {
            terminal_id: terminal_id.clone(),
            task_id: task_id.clone(),
            working_dir: working_dir.to_string_lossy().to_string(),
        };
        spawn_output_reader(reader, info.clone(), emit);

        let terminal = WorkspaceTerminal {
            info: info.clone(),
            master: pair.master,
            writer,
            child,
        };
        self.terminals
            .lock()
            .map_err(|_| "临时终端状态不可用".to_string())?
            .insert(task_id, terminal);
        Ok(info)
    }

    pub fn write(&self, task_id: &str, terminal_id: &str, data: &str) -> Result<(), String> {
        if data.len() > MAX_TERMINAL_INPUT_BYTES {
            return Err("单次终端输入过长".to_string());
        }
        let mut terminals = self
            .terminals
            .lock()
            .map_err(|_| "临时终端状态不可用".to_string())?;
        let terminal = terminal_for_request(&mut terminals, task_id, terminal_id)?;
        terminal
            .writer
            .write_all(data.as_bytes())
            .and_then(|_| terminal.writer.flush())
            .map_err(|error| format!("无法写入临时终端: {error}"))
    }

    pub fn resize(
        &self,
        task_id: &str,
        terminal_id: &str,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        let mut terminals = self
            .terminals
            .lock()
            .map_err(|_| "临时终端状态不可用".to_string())?;
        let terminal = terminal_for_request(&mut terminals, task_id, terminal_id)?;
        terminal
            .master
            .resize(clamp_pty_size(rows, cols))
            .map_err(|error| format!("无法调整临时终端大小: {error}"))
    }

    pub fn close(&self, task_id: &str, terminal_id: &str) -> Result<(), String> {
        let terminal = {
            let mut terminals = self
                .terminals
                .lock()
                .map_err(|_| "临时终端状态不可用".to_string())?;
            let terminal = terminal_for_request(&mut terminals, task_id, terminal_id)?;
            let key = terminal.info.task_id.clone();
            terminals.remove(&key)
        };
        if let Some(terminal) = terminal {
            terminal.close();
        }
        Ok(())
    }

    fn remove_and_close(&self, task_id: &str) -> Result<(), String> {
        let terminal = self
            .terminals
            .lock()
            .map_err(|_| "临时终端状态不可用".to_string())?
            .remove(task_id);
        if let Some(terminal) = terminal {
            terminal.close();
        }
        Ok(())
    }

    #[cfg(test)]
    fn contains_task(&self, task_id: &str) -> bool {
        self.terminals
            .lock()
            .map(|terminals| terminals.contains_key(task_id))
            .unwrap_or(false)
    }
}

impl Drop for WorkspaceTerminalStore {
    fn drop(&mut self) {
        if let Ok(terminals) = self.terminals.get_mut() {
            for (_, terminal) in terminals.drain() {
                terminal.close();
            }
        }
    }
}

#[tauri::command]
pub async fn start_workspace_terminal(
    app: tauri::AppHandle,
    state: tauri::State<'_, Arc<AppState>>,
    task_id: String,
    session_id: Option<String>,
    working_dir: Option<String>,
    rows: Option<u16>,
    cols: Option<u16>,
) -> Result<WorkspaceTerminalInfo, String> {
    let working_dir =
        resolve_bound_working_dir(&state, session_id.as_deref(), working_dir.as_deref()).await?;
    let store = Arc::clone(&state.workspace_terminals);
    let output_app = app.clone();
    let emitter: OutputEmitter = Arc::new(move |output| {
        let _ = output_app.emit("work-panel-terminal-output", output);
    });
    tokio::task::spawn_blocking(move || {
        store.start(
            &task_id,
            &working_dir,
            rows.unwrap_or(24),
            cols.unwrap_or(80),
            emitter,
        )
    })
    .await
    .map_err(|error| format!("启动临时终端的任务失败: {error}"))?
}

#[tauri::command]
pub fn write_workspace_terminal(
    state: tauri::State<'_, Arc<AppState>>,
    task_id: String,
    terminal_id: String,
    data: String,
) -> Result<(), String> {
    state
        .workspace_terminals
        .write(&task_id, &terminal_id, &data)
}

#[tauri::command]
pub fn resize_workspace_terminal(
    state: tauri::State<'_, Arc<AppState>>,
    task_id: String,
    terminal_id: String,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    state
        .workspace_terminals
        .resize(&task_id, &terminal_id, rows, cols)
}

#[tauri::command]
pub fn close_workspace_terminal(
    state: tauri::State<'_, Arc<AppState>>,
    task_id: String,
    terminal_id: String,
) -> Result<(), String> {
    state.workspace_terminals.close(&task_id, &terminal_id)
}

fn terminal_for_request<'a>(
    terminals: &'a mut HashMap<String, WorkspaceTerminal>,
    task_id: &str,
    terminal_id: &str,
) -> Result<&'a mut WorkspaceTerminal, String> {
    let terminal = terminals
        .get_mut(task_id)
        .ok_or_else(|| "这个任务没有正在运行的临时终端".to_string())?;
    if terminal.info.terminal_id != terminal_id || terminal.info.task_id != task_id {
        return Err("临时终端不属于当前任务".to_string());
    }
    Ok(terminal)
}

fn clean_task_id(task_id: &str) -> Result<String, String> {
    let task_id = task_id.trim();
    if task_id.is_empty() || task_id.len() > 256 {
        return Err("任务标识无效".to_string());
    }
    Ok(task_id.to_string())
}

fn clamp_pty_size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows: rows.clamp(MIN_TERMINAL_ROWS, MAX_TERMINAL_ROWS),
        cols: cols.clamp(MIN_TERMINAL_COLS, MAX_TERMINAL_COLS),
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn shell_command() -> CommandBuilder {
    #[cfg(windows)]
    let shell = std::env::var_os("COMSPEC").unwrap_or_else(|| "cmd.exe".into());
    #[cfg(not(windows))]
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
    CommandBuilder::new(shell)
}

fn spawn_output_reader(
    mut reader: Box<dyn Read + Send>,
    info: WorkspaceTerminalInfo,
    emit: OutputEmitter,
) {
    std::thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(read) => emit(WorkspaceTerminalOutput {
                    terminal_id: info.terminal_id.clone(),
                    task_id: info.task_id.clone(),
                    chunk: String::from_utf8_lossy(&buffer[..read]).into_owned(),
                    exited: false,
                }),
                Err(_) => break,
            }
        }
        emit(WorkspaceTerminalOutput {
            terminal_id: info.terminal_id,
            task_id: info.task_id,
            chunk: String::new(),
            exited: true,
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::time::Duration;

    fn temp_workspace(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-workspace-terminal-{label}-{}-{}",
            std::process::id(),
            uuid::Uuid::now_v7()
        ));
        std::fs::create_dir_all(&path).expect("workspace");
        path
    }

    #[test]
    fn workspace_terminal_writes_output_and_closes_task_handle() {
        let workspace = temp_workspace("lifecycle");
        let store = WorkspaceTerminalStore::default();
        let (sender, receiver) = mpsc::channel();
        let info = store
            .start(
                "task-1",
                &workspace,
                24,
                80,
                Arc::new(move |output| {
                    let _ = sender.send(output);
                }),
            )
            .expect("start terminal");

        store
            .write(
                "task-1",
                &info.terminal_id,
                "printf 'forge-terminal-ready\\n'\n",
            )
            .expect("write command");

        let mut output = String::new();
        for _ in 0..20 {
            let event = receiver
                .recv_timeout(Duration::from_millis(250))
                .expect("terminal output");
            output.push_str(&event.chunk);
            if output.contains("forge-terminal-ready") {
                break;
            }
        }
        assert!(output.contains("forge-terminal-ready"));
        assert!(store.contains_task("task-1"));

        store
            .close("task-1", &info.terminal_id)
            .expect("close terminal");
        assert!(!store.contains_task("task-1"));
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn workspace_terminal_rejects_cross_task_access() {
        let workspace = temp_workspace("scope");
        let store = WorkspaceTerminalStore::default();
        let info = store
            .start("task-1", &workspace, 24, 80, Arc::new(|_| {}))
            .expect("start terminal");

        let error = store
            .write("task-2", &info.terminal_id, "echo blocked\n")
            .expect_err("cross-task write should fail");
        assert!(error.contains("没有正在运行"));

        store
            .close("task-1", &info.terminal_id)
            .expect("close terminal");
        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn workspace_terminal_clamps_resize_dimensions() {
        assert_eq!(
            clamp_pty_size(0, 0),
            PtySize {
                rows: 2,
                cols: 10,
                pixel_width: 0,
                pixel_height: 0
            }
        );
        assert_eq!(
            clamp_pty_size(u16::MAX, u16::MAX),
            PtySize {
                rows: 200,
                cols: 400,
                pixel_width: 0,
                pixel_height: 0
            }
        );
    }
}
