use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use tauri::State;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::state::{AppState, ManagedDevServer};

#[derive(Clone)]
struct RuntimeConfig {
    working_dir: std::path::PathBuf,
    has_package_json: bool,
    package_manager: String,
    dev_script: Option<String>,
    command: Option<String>,
    port: u16,
    url: String,
}

#[derive(serde::Serialize)]
pub struct ProjectRuntimeStatus {
    working_dir: String,
    has_package_json: bool,
    package_manager: String,
    dev_script: Option<String>,
    command: Option<String>,
    port: u16,
    url: String,
    running: bool,
    managed: bool,
    pid: Option<u32>,
    can_start: bool,
    can_stop: bool,
    can_open: bool,
    message: String,
    logs: Vec<String>,
}

#[tauri::command]
pub async fn get_project_runtime_status(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
) -> Result<ProjectRuntimeStatus, String> {
    project_runtime_status(&state, session_id.as_deref()).await
}

#[tauri::command]
pub async fn start_project_dev_server(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
) -> Result<ProjectRuntimeStatus, String> {
    let working_dir = runtime_working_dir(&state, session_id.as_deref()).await;
    let config = runtime_config(&working_dir);
    if config.dev_script.is_none() {
        return Ok(status_from_config(
            &config,
            ManagedSnapshot::none(),
            false,
            "当前项目没有 package.json 的 dev 脚本",
        ));
    }

    if port_is_open(config.port) {
        return project_runtime_status(&state, session_id.as_deref()).await;
    }

    if refresh_managed_server(&state, &config.working_dir)
        .await
        .is_some()
    {
        return project_runtime_status(&state, session_id.as_deref()).await;
    }

    let mut command = Command::new(&config.package_manager);
    match config.package_manager.as_str() {
        "npm" => {
            command.args(["run", "dev"]);
        }
        "pnpm" | "yarn" | "bun" => {
            command.arg("dev");
        }
        _ => {
            command.args(["run", "dev"]);
        }
    }

    command
        .current_dir(&config.working_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("BROWSER", "none");

    let mut child = command
        .spawn()
        .map_err(|e| format!("启动 dev 服务失败: {}", e))?;
    let logs = Arc::new(RwLock::new(vec![format!(
        "$ {}",
        config
            .command
            .clone()
            .unwrap_or_else(|| "npm run dev".to_string())
    )]));

    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(logs.clone(), "out", stdout);
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(logs.clone(), "err", stderr);
    }

    let managed = ManagedDevServer {
        child,
        working_dir: config.working_dir.clone(),
        port: config.port,
        url: config.url.clone(),
        command: config
            .command
            .clone()
            .unwrap_or_else(|| "npm run dev".to_string()),
        logs,
    };

    {
        let mut guard = state.dev_server.write().await;
        *guard = Some(managed);
    }

    tokio::time::sleep(Duration::from_millis(800)).await;
    project_runtime_status(&state, session_id.as_deref()).await
}

#[tauri::command]
pub async fn stop_project_dev_server(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
) -> Result<ProjectRuntimeStatus, String> {
    let mut server = {
        let mut guard = state.dev_server.write().await;
        guard.take()
    };

    if let Some(server) = server.as_mut() {
        let _ = server.child.kill().await;
        let _ = server.child.wait().await;
    }

    tokio::time::sleep(Duration::from_millis(300)).await;
    project_runtime_status(&state, session_id.as_deref()).await
}

#[tauri::command]
pub async fn open_project_preview(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
) -> Result<ProjectRuntimeStatus, String> {
    let status = project_runtime_status(&state, session_id.as_deref()).await?;
    if !status.running {
        return Err("预览服务还没有运行。请先启动预览。".into());
    }
    open_url(&status.url)?;
    Ok(status)
}

async fn project_runtime_status(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> Result<ProjectRuntimeStatus, String> {
    let working_dir = runtime_working_dir(state, session_id).await;
    let config = runtime_config(&working_dir);
    let managed = refresh_managed_server(state, &config.working_dir)
        .await
        .unwrap_or_else(ManagedSnapshot::none);
    let running = managed.running || port_is_open(config.port);

    let message = if managed.running {
        "预览服务由应用启动，正在运行"
    } else if running {
        "检测到预览地址已经在运行"
    } else if config.dev_script.is_some() {
        "可以启动项目预览"
    } else if config.has_package_json {
        "当前项目没有 dev 脚本"
    } else {
        "当前项目没有 package.json"
    };

    Ok(status_from_config(&config, managed, running, message))
}

#[derive(Clone)]
struct ManagedSnapshot {
    running: bool,
    pid: Option<u32>,
    port: Option<u16>,
    url: Option<String>,
    command: Option<String>,
    logs: Vec<String>,
}

impl ManagedSnapshot {
    fn none() -> Self {
        Self {
            running: false,
            pid: None,
            port: None,
            url: None,
            command: None,
            logs: Vec::new(),
        }
    }
}

async fn refresh_managed_server(
    state: &Arc<AppState>,
    working_dir: &std::path::Path,
) -> Option<ManagedSnapshot> {
    let mut guard = state.dev_server.write().await;
    let server = guard.as_mut()?;
    if server.working_dir != working_dir {
        return None;
    }
    let logs = server.logs.read().await.clone();

    match server.child.try_wait() {
        Ok(Some(status)) => {
            crate::app_log!("INFO", "[project_runtime] dev server exited: {}", status);
            *guard = None;
            None
        }
        Ok(None) => Some(ManagedSnapshot {
            running: true,
            pid: server.child.id(),
            port: Some(server.port),
            url: Some(server.url.clone()),
            command: Some(server.command.clone()),
            logs,
        }),
        Err(error) => {
            crate::app_log!(
                "WARN",
                "[project_runtime] unable to inspect dev server: {}",
                error
            );
            *guard = None;
            None
        }
    }
}

fn status_from_config(
    config: &RuntimeConfig,
    managed: ManagedSnapshot,
    running: bool,
    message: &str,
) -> ProjectRuntimeStatus {
    let port = managed.port.unwrap_or(config.port);
    let url = managed.url.unwrap_or_else(|| config.url.clone());
    let command = managed.command.clone().or_else(|| config.command.clone());

    ProjectRuntimeStatus {
        working_dir: config.working_dir.to_string_lossy().to_string(),
        has_package_json: config.has_package_json,
        package_manager: config.package_manager.clone(),
        dev_script: config.dev_script.clone(),
        command,
        port,
        url,
        running,
        managed: managed.running,
        pid: managed.pid,
        can_start: config.dev_script.is_some() && !running,
        can_stop: managed.running,
        can_open: running,
        message: message.to_string(),
        logs: managed.logs,
    }
}

fn runtime_config(working_dir: &std::path::Path) -> RuntimeConfig {
    let package_json_path = working_dir.join("package.json");
    let package_json = std::fs::read_to_string(&package_json_path).ok();
    let dev_script = package_json
        .as_deref()
        .and_then(|content| serde_json::from_str::<serde_json::Value>(content).ok())
        .and_then(|json| {
            json.get("scripts")?
                .get("dev")?
                .as_str()
                .map(str::to_string)
        });
    let port = dev_script
        .as_deref()
        .and_then(extract_port)
        .or_else(|| vite_config_port(working_dir))
        .unwrap_or(5173);
    let package_manager = detect_package_manager(working_dir);
    let command = dev_script.as_ref().map(|_| match package_manager.as_str() {
        "npm" => "npm run dev".to_string(),
        "pnpm" => "pnpm dev".to_string(),
        "yarn" => "yarn dev".to_string(),
        "bun" => "bun dev".to_string(),
        other => format!("{} run dev", other),
    });

    RuntimeConfig {
        working_dir: working_dir.to_path_buf(),
        has_package_json: package_json.is_some(),
        package_manager,
        dev_script,
        command,
        port,
        url: format!("http://localhost:{}", port),
    }
}

fn detect_package_manager(working_dir: &std::path::Path) -> String {
    if working_dir.join("pnpm-lock.yaml").exists() {
        "pnpm".into()
    } else if working_dir.join("yarn.lock").exists() {
        "yarn".into()
    } else if working_dir.join("bun.lockb").exists() || working_dir.join("bun.lock").exists() {
        "bun".into()
    } else {
        "npm".into()
    }
}

fn vite_config_port(working_dir: &std::path::Path) -> Option<u16> {
    for name in [
        "vite.config.ts",
        "vite.config.js",
        "vite.config.mts",
        "vite.config.mjs",
    ] {
        if let Ok(content) = std::fs::read_to_string(working_dir.join(name)) {
            if let Some(port) = extract_config_port(&content) {
                return Some(port);
            }
        }
    }
    None
}

async fn runtime_working_dir(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> std::path::PathBuf {
    if let Some(session_id) = session_id {
        if let Some(session) = state.sessions.read().await.get(session_id).cloned() {
            return session.harness.working_dir.clone();
        }
    }
    state.harness.working_dir.clone()
}

fn spawn_log_reader<R>(logs: Arc<RwLock<Vec<String>>>, label: &'static str, reader: R)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let mut guard = logs.write().await;
            guard.push(format!("[{}] {}", label, line));
            if guard.len() > 200 {
                let overflow = guard.len() - 200;
                guard.drain(0..overflow);
            }
        }
    });
}

fn extract_port(script: &str) -> Option<u16> {
    let re = Regex::new(r"--port(?:=|\s+)(\d{2,5})").ok()?;
    re.captures(script)?.get(1)?.as_str().parse().ok()
}

fn extract_config_port(content: &str) -> Option<u16> {
    let re = Regex::new(r"(?m)\bport\s*:\s*(\d{2,5})").ok()?;
    re.captures(content)?.get(1)?.as_str().parse().ok()
}

fn port_is_open(port: u16) -> bool {
    let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    TcpStream::connect_timeout(&addr, Duration::from_millis(180)).is_ok()
}

fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let output = std::process::Command::new("open").arg(url).output();

    #[cfg(target_os = "windows")]
    let output = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .output();

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    let output = std::process::Command::new("xdg-open").arg(url).output();

    let output = output.map_err(|e| format!("无法打开预览地址: {}", e))?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() {
            "无法打开预览地址".into()
        } else {
            stderr
        })
    }
}
