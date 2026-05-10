use crate::protocol::commands::ToolType;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::Emitter;

/// Represents a single CLI session with a PTY and child process.
pub struct CliSession {
    pub id: String,
    pub tool_type: ToolType,
    pub working_dir: String,
    pub status: Arc<Mutex<SessionStatus>>,
    pub(crate) pty_master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    pub(crate) pty_writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    pub(crate) child: Arc<Mutex<Option<Box<dyn portable_pty::Child + Send + Sync>>>>,
    pub(crate) running: Arc<AtomicBool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    Starting,
    Running,
    Stopped,
    Error(String),
}

impl CliSession {
    /// Spawn a new CLI session.
    pub fn spawn(
        id: String,
        tool_type: ToolType,
        working_dir: &str,
        tool_path: Option<&str>,
        app_handle: tauri::AppHandle,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();

        let (program, args): (String, Vec<String>) = match &tool_type {
            ToolType::Claude => (
                tool_path.unwrap_or("claude").to_string(),
                vec![],
            ),
            ToolType::Codex => (
                tool_path.unwrap_or("codex").to_string(),
                vec![],
            ),
            ToolType::Hermes => (
                tool_path.unwrap_or("hermes").to_string(),
                vec![],
            ),
            ToolType::Bash => {
                #[cfg(not(target_os = "windows"))]
                {
                    (String::from("/bin/bash"), vec!["--login".to_string()])
                }
                #[cfg(target_os = "windows")]
                {
                    (String::from("cmd.exe"), vec![])
                }
            }
        };

        let mut cmd = CommandBuilder::new(&program);
        cmd.args(&args);
        cmd.cwd(working_dir);
        cmd.env("TERM", "xterm-256color");
        cmd.env("COLORTERM", "truecolor");

        let pty_pair = pty_system
            .openpty(PtySize {
                rows: 40,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {}", e))?;

        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        drop(pty_pair.slave);

        let master = pty_pair.master;

        // Get reader and writer from the master PTY
        let reader: Box<dyn Read + Send> = master
            .try_clone_reader()
            .map_err(|e| format!("Failed to get PTY reader: {}", e))?;

        let writer: Box<dyn Write + Send> = master
            .take_writer()
            .map_err(|e| format!("Failed to get PTY writer: {}", e))?;

        let pty_master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>> =
            Arc::new(Mutex::new(Some(master)));
        let pty_writer: Arc<Mutex<Option<Box<dyn Write + Send>>>> =
            Arc::new(Mutex::new(Some(writer)));
        let child_handle: Arc<Mutex<Option<Box<dyn portable_pty::Child + Send + Sync>>>> =
            Arc::new(Mutex::new(Some(child)));
        let running = Arc::new(AtomicBool::new(true));

        let session = CliSession {
            id: id.clone(),
            tool_type: tool_type.clone(),
            working_dir: working_dir.to_string(),
            status: Arc::new(Mutex::new(SessionStatus::Starting)),
            pty_master,
            pty_writer,
            child: child_handle,
            running: running.clone(),
        };

        // Start the reader thread with the Read handle
        let session_id = id.clone();
        let reader_app_handle = app_handle.clone();

        std::thread::spawn(move || {
            crate::pty::reader::read_loop(session_id, reader, running, reader_app_handle);
        });

        let agent_type = format!("{:?}", session.tool_type).to_lowercase();
        let _ = app_handle.emit(
            "session-output",
            crate::protocol::events::StreamEvent::SessionStarted {
                session_id: session.id.clone(),
                agent_type: agent_type.clone(),
                model: agent_type,
            },
        );

        *session.status.lock().unwrap() = SessionStatus::Running;

        Ok(session)
    }

    /// Write input text to the PTY.
    pub fn write_input(&self, text: &str) -> Result<(), String> {
        let mut writer = self.pty_writer.lock().unwrap();
        match writer.as_mut() {
            Some(w) => {
                w.write_all(text.as_bytes())
                    .map_err(|e| format!("Failed to write to PTY: {}", e))?;
                Ok(())
            }
            None => Err("PTY writer not available".to_string()),
        }
    }

    /// Resize the PTY.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), String> {
        let mut master = self.pty_master.lock().unwrap();
        match master.as_mut() {
            Some(m) => m
                .resize(PtySize {
                    rows,
                    cols,
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| format!("Failed to resize PTY: {}", e)),
            None => Err("PTY master not available".to_string()),
        }
    }

    /// Kill the session.
    pub fn kill(&self, app_handle: &tauri::AppHandle) -> Result<(), String> {
        self.running.store(false, Ordering::SeqCst);

        let mut child = self.child.lock().unwrap();
        if let Some(c) = child.as_mut() {
            let _ = c.try_wait();
            let _ = c.kill();
        }
        *child = None;

        *self.status.lock().unwrap() = SessionStatus::Stopped;

        let _ = app_handle.emit(
            "session-output",
            crate::protocol::events::StreamEvent::SessionStopped {
                session_id: self.id.clone(),
                reason: "killed".to_string(),
            },
        );

        Ok(())
    }

    /// Send a signal via control character.
    pub fn send_signal(&self, signal: &str) -> Result<(), String> {
        match signal {
            "interrupt" => self.write_input("\x03")?,
            "terminate" => self.write_input("\x04")?,
            _ => return Err(format!("Unknown signal: {}", signal)),
        }
        Ok(())
    }
}

impl Drop for CliSession {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
    }
}
