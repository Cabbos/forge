use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use regex::Regex;
use tauri::State;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::RwLock;

use crate::ipc::workspace::resolve_bound_working_dir;
use crate::process_runner::{configure_command_process_group, kill_child_process_group};
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

#[derive(Clone)]
struct PortOccupancy {
    open: bool,
    owner_pid: Option<u32>,
    owner_working_dir: Option<std::path::PathBuf>,
}

#[derive(Clone)]
struct RuntimeAvailability {
    running: bool,
    blocked_by_port: bool,
    message: String,
}

#[derive(serde::Serialize)]
pub struct ProjectRuntimeStatus {
    pub(crate) working_dir: String,
    pub(crate) has_package_json: bool,
    pub(crate) package_manager: String,
    pub(crate) dev_script: Option<String>,
    pub(crate) command: Option<String>,
    pub(crate) port: u16,
    pub(crate) url: String,
    pub(crate) running: bool,
    pub(crate) managed: bool,
    pub(crate) pid: Option<u32>,
    pub(crate) can_start: bool,
    pub(crate) can_stop: bool,
    pub(crate) can_open: bool,
    pub(crate) message: String,
    pub(crate) logs: Vec<String>,
}

#[tauri::command]
pub async fn get_project_runtime_status(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<ProjectRuntimeStatus, String> {
    project_runtime_status_for_request(&state, session_id.as_deref(), working_dir.as_deref()).await
}

#[tauri::command]
pub async fn start_project_dev_server(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<ProjectRuntimeStatus, String> {
    let working_dir =
        runtime_working_dir_or_explicit(&state, session_id.as_deref(), working_dir.as_deref())
            .await?;
    let config = runtime_config(&working_dir);
    if config.dev_script.is_none() {
        let availability = RuntimeAvailability {
            running: false,
            blocked_by_port: false,
            message: "当前项目没有 package.json 的 dev 脚本".to_string(),
        };
        return Ok(status_from_config(
            &config,
            ManagedSnapshot::none(),
            availability,
        ));
    }

    if inspect_port_occupancy(config.port).open {
        return project_runtime_status_for_path(&state, working_dir).await;
    }

    if refresh_managed_server(&state, &config.working_dir)
        .await
        .is_some()
    {
        return project_runtime_status_for_path(&state, working_dir).await;
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
    configure_command_process_group(&mut command);

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
    project_runtime_status_for_path(&state, config.working_dir.clone()).await
}

#[tauri::command]
pub async fn stop_project_dev_server(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<ProjectRuntimeStatus, String> {
    let working_dir =
        runtime_working_dir_or_explicit(&state, session_id.as_deref(), working_dir.as_deref())
            .await?;
    let mut server = {
        let mut guard = state.dev_server.write().await;
        guard.take()
    };

    if let Some(server) = server.as_mut() {
        kill_child_process_group(&mut server.child).await;
        let _ = server.child.wait().await;
    }

    tokio::time::sleep(Duration::from_millis(300)).await;
    project_runtime_status_for_path(&state, working_dir).await
}

#[tauri::command]
pub async fn open_project_preview(
    state: State<'_, Arc<AppState>>,
    session_id: Option<String>,
    working_dir: Option<String>,
) -> Result<ProjectRuntimeStatus, String> {
    let status =
        project_runtime_status_for_request(&state, session_id.as_deref(), working_dir.as_deref())
            .await?;
    if !status.running {
        return Err("预览服务还没有运行。请先启动预览。".into());
    }
    open_url(&status.url)?;
    Ok(status)
}

pub(crate) async fn project_runtime_status_for_session(
    state: &Arc<AppState>,
    session_id: Option<&str>,
) -> Result<ProjectRuntimeStatus, String> {
    let working_dir = resolve_bound_working_dir(state, session_id, None).await?;
    project_runtime_status_for_path(state, working_dir).await
}

async fn project_runtime_status_for_request(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<ProjectRuntimeStatus, String> {
    let working_dir = runtime_working_dir_or_explicit(state, session_id, working_dir).await?;
    project_runtime_status_for_path(state, working_dir).await
}

async fn project_runtime_status_for_path(
    state: &Arc<AppState>,
    working_dir: std::path::PathBuf,
) -> Result<ProjectRuntimeStatus, String> {
    let config = runtime_config(&working_dir);
    let managed = refresh_managed_server(state, &config.working_dir)
        .await
        .unwrap_or_else(ManagedSnapshot::none);
    let occupancy = inspect_port_occupancy(config.port);
    let availability = runtime_availability(&config, &managed, &occupancy);

    Ok(status_from_config(&config, managed, availability))
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
    availability: RuntimeAvailability,
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
        running: availability.running,
        managed: managed.running,
        pid: managed.pid,
        can_start: config.dev_script.is_some()
            && !availability.running
            && !availability.blocked_by_port,
        can_stop: managed.running,
        can_open: availability.running,
        message: availability.message,
        logs: managed.logs,
    }
}

fn runtime_availability(
    config: &RuntimeConfig,
    managed: &ManagedSnapshot,
    occupancy: &PortOccupancy,
) -> RuntimeAvailability {
    if managed.running {
        return RuntimeAvailability {
            running: true,
            blocked_by_port: false,
            message: "预览服务由应用启动，正在运行".to_string(),
        };
    }

    if config.dev_script.is_none() {
        let message = if config.has_package_json {
            "当前项目没有 dev 脚本"
        } else {
            "当前项目没有 package.json"
        };
        return RuntimeAvailability {
            running: false,
            blocked_by_port: false,
            message: message.to_string(),
        };
    }

    if occupancy.open {
        if let Some(owner_working_dir) = occupancy.owner_working_dir.as_deref() {
            if same_workspace(owner_working_dir, &config.working_dir) {
                return RuntimeAvailability {
                    running: true,
                    blocked_by_port: false,
                    message: "检测到当前项目预览已经在运行".to_string(),
                };
            }

            let owner_label = owner_working_dir
                .file_name()
                .and_then(|name| name.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| owner_working_dir.to_string_lossy().to_string());
            return RuntimeAvailability {
                running: false,
                blocked_by_port: true,
                message: format!("端口 {} 已被其他项目占用：{}", config.port, owner_label),
            };
        }

        return RuntimeAvailability {
            running: true,
            blocked_by_port: false,
            message: "检测到预览地址已经在运行".to_string(),
        };
    }

    RuntimeAvailability {
        running: false,
        blocked_by_port: false,
        message: "可以启动项目预览".to_string(),
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

async fn runtime_working_dir_or_explicit(
    state: &Arc<AppState>,
    session_id: Option<&str>,
    working_dir: Option<&str>,
) -> Result<std::path::PathBuf, String> {
    resolve_bound_working_dir(state, session_id, working_dir).await
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

fn inspect_port_occupancy(port: u16) -> PortOccupancy {
    if !port_is_open(port) {
        return PortOccupancy {
            open: false,
            owner_pid: None,
            owner_working_dir: None,
        };
    }

    if let Some((pid, working_dir)) = inspect_port_owner(port) {
        return PortOccupancy {
            open: true,
            owner_pid: Some(pid),
            owner_working_dir: working_dir,
        };
    }

    PortOccupancy {
        open: true,
        owner_pid: None,
        owner_working_dir: None,
    }
}

fn inspect_port_owner(port: u16) -> Option<(u32, Option<std::path::PathBuf>)> {
    #[cfg(unix)]
    {
        let output = std::process::Command::new("lsof")
            .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-Fp"])
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let pid = stdout
            .lines()
            .find_map(|line| line.strip_prefix('p'))
            .and_then(|value| value.parse::<u32>().ok())?;
        Some((pid, inspect_process_working_dir(pid)))
    }

    #[cfg(not(unix))]
    {
        let _ = port;
        None
    }
}

#[cfg(unix)]
fn inspect_process_working_dir(pid: u32) -> Option<std::path::PathBuf> {
    let output = std::process::Command::new("lsof")
        .args(["-nP", "-a", "-p", &pid.to_string(), "-d", "cwd", "-Fn"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| line.strip_prefix('n'))
        .filter(|path| !path.trim().is_empty())
        .map(std::path::PathBuf::from)
}

fn same_workspace(left: &std::path::Path, right: &std::path::Path) -> bool {
    comparable_path(left) == comparable_path(right)
}

fn comparable_path(path: &std::path::Path) -> std::path::PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_workspace(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "forge-runtime-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn config_for(path: &std::path::Path) -> RuntimeConfig {
        RuntimeConfig {
            working_dir: path.to_path_buf(),
            has_package_json: true,
            package_manager: "npm".to_string(),
            dev_script: Some("vite".to_string()),
            command: Some("npm run dev".to_string()),
            port: 5173,
            url: "http://localhost:5173".to_string(),
        }
    }

    fn config_without_dev(path: &std::path::Path) -> RuntimeConfig {
        RuntimeConfig {
            working_dir: path.to_path_buf(),
            has_package_json: true,
            package_manager: "npm".to_string(),
            dev_script: None,
            command: None,
            port: 5173,
            url: "http://localhost:5173".to_string(),
        }
    }

    #[test]
    fn runtime_does_not_treat_other_project_port_as_current_preview() {
        let project = temp_workspace("project");
        let other = temp_workspace("other");
        let config = config_for(&project);
        let managed = ManagedSnapshot::none();
        let occupancy = PortOccupancy {
            open: true,
            owner_pid: Some(42),
            owner_working_dir: Some(other.clone()),
        };

        let availability = runtime_availability(&config, &managed, &occupancy);
        let status = status_from_config(&config, managed, availability);

        assert!(!status.running);
        assert!(!status.can_open);
        assert!(!status.can_start);
        assert!(status.message.contains("5173"));
        assert!(status.message.contains("other"));

        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(other);
    }

    #[test]
    fn runtime_treats_same_project_port_as_running_preview() {
        let project = temp_workspace("project");
        let config = config_for(&project);
        let managed = ManagedSnapshot::none();
        let occupancy = PortOccupancy {
            open: true,
            owner_pid: Some(42),
            owner_working_dir: Some(project.clone()),
        };

        let availability = runtime_availability(&config, &managed, &occupancy);
        let status = status_from_config(&config, managed, availability);

        assert!(status.running);
        assert!(status.can_open);
        assert!(!status.can_start);
        assert_eq!(status.message, "检测到当前项目预览已经在运行");

        let _ = std::fs::remove_dir_all(project);
    }

    #[test]
    fn runtime_without_dev_script_ignores_unrelated_default_port() {
        let project = temp_workspace("project");
        let other = temp_workspace("other");
        let config = config_without_dev(&project);
        let managed = ManagedSnapshot::none();
        let occupancy = PortOccupancy {
            open: true,
            owner_pid: Some(42),
            owner_working_dir: Some(other.clone()),
        };

        let availability = runtime_availability(&config, &managed, &occupancy);
        let status = status_from_config(&config, managed, availability);

        assert!(!status.running);
        assert!(!status.can_open);
        assert!(!status.can_start);
        assert_eq!(status.message, "当前项目没有 dev 脚本");

        let _ = std::fs::remove_dir_all(project);
        let _ = std::fs::remove_dir_all(other);
    }

    #[tokio::test]
    async fn runtime_request_requires_session_or_explicit_workspace() {
        let workspace = temp_workspace("missing-workspace-binding");
        let state = std::sync::Arc::new(crate::state::AppState::new(std::sync::Arc::new(
            crate::harness::Harness::new(workspace.clone()),
        )));

        let error = runtime_working_dir_or_explicit(&state, None, None)
            .await
            .expect_err("missing workspace should fail");

        assert!(error.contains("工作空间"));

        let _ = std::fs::remove_dir_all(workspace);
    }
}
