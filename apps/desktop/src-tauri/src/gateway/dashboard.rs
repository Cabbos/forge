//! Local read-only HTTP dashboard served by the gateway.

use std::sync::Arc;

use crate::gateway::protocol::{GatewayReply, GatewayRequest};
use crate::gateway::server::{dispatch, GatewayState};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;

/// Default loopback TCP port for the local dashboard.
pub const DASHBOARD_PORT: u16 = 2022;

pub fn dashboard_response_for_request_line(
    state: Arc<GatewayState>,
    request_line: &str,
) -> Result<String, String> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default();
    let path = parts.next().unwrap_or_default();

    if method != "GET" {
        return Ok(http_response(
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            "method not allowed",
        ));
    }

    match path {
        "/" => Ok(http_response(
            "200 OK",
            "text/html; charset=utf-8",
            dashboard_html(),
        )),
        "/api/dashboard" => {
            let body = dashboard_snapshot_json(state)?;
            Ok(http_response("200 OK", "application/json", &body))
        }
        _ => Ok(http_response(
            "404 Not Found",
            "text/plain; charset=utf-8",
            "not found",
        )),
    }
}

pub async fn handle_dashboard_stream<S>(state: Arc<GatewayState>, stream: S) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();
    let request_line = lines
        .next_line()
        .await
        .map_err(|error| format!("read dashboard request: {error}"))?
        .unwrap_or_default();
    let response = dashboard_response_for_request_line(state, &request_line)?;
    writer
        .write_all(response.as_bytes())
        .await
        .map_err(|error| format!("write dashboard response: {error}"))?;
    Ok(())
}

pub async fn accept_dashboard_connection_once(
    state: Arc<GatewayState>,
    listener: &TcpListener,
) -> Result<(), String> {
    let (stream, _peer) = listener
        .accept()
        .await
        .map_err(|error| format!("dashboard accept: {error}"))?;
    handle_dashboard_stream(state, stream).await
}

pub async fn serve(state: Arc<GatewayState>) -> Result<(), String> {
    let addr = format!("127.0.0.1:{DASHBOARD_PORT}");
    let listener = TcpListener::bind(&addr)
        .await
        .map_err(|error| format!("bind dashboard http: {error}"))?;

    log::info!("Gateway dashboard listening on http://{addr}");

    loop {
        let (stream, peer) = listener
            .accept()
            .await
            .map_err(|error| format!("dashboard accept: {error}"))?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(error) = handle_dashboard_stream(state, stream).await {
                log::debug!("dashboard connection from {peer} failed: {error}");
            }
        });
    }
}

fn dashboard_snapshot_json(state: Arc<GatewayState>) -> Result<String, String> {
    let reply = dispatch(
        &state,
        GatewayRequest {
            id: "dashboard-http".to_string(),
            method: "dashboard_snapshot".to_string(),
            params: None,
        },
    );
    let GatewayReply::Ok(response) = reply else {
        return Err("dashboard snapshot request failed".to_string());
    };
    serde_json::to_string(&response.result).map_err(|error| format!("serialize dashboard: {error}"))
}

fn dashboard_html() -> &'static str {
    r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Forge Gateway Dashboard</title>
  <style>
    :root {
      color-scheme: light dark;
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      background: #f5f6f8;
      color: #202124;
    }
    body {
      margin: 0;
      min-height: 100vh;
      background: #f5f6f8;
    }
    main {
      width: min(1180px, calc(100vw - 32px));
      margin: 0 auto;
      padding: 28px 0 40px;
    }
    header {
      display: flex;
      justify-content: space-between;
      gap: 20px;
      align-items: flex-start;
      margin-bottom: 20px;
    }
    h1 {
      margin: 0;
      font-size: 26px;
      font-weight: 680;
    }
    h2 {
      margin: 0 0 10px;
      font-size: 15px;
      font-weight: 650;
    }
    .muted {
      color: #666d75;
      font-size: 13px;
    }
    .status {
      display: inline-flex;
      align-items: center;
      gap: 8px;
      border: 1px solid #c8cec8;
      border-radius: 999px;
      padding: 6px 10px;
      background: #ffffffb8;
      font-size: 13px;
    }
    .status::before {
      content: "";
      width: 8px;
      height: 8px;
      border-radius: 50%;
      background: #2f8f46;
    }
    .status[data-ok="false"]::before {
      background: #b54708;
    }
    .grid {
      display: grid;
      grid-template-columns: repeat(12, 1fr);
      gap: 12px;
    }
    section {
      grid-column: span 6;
      border: 1px solid #d6dad4;
      border-radius: 8px;
      background: #fffffff2;
      padding: 14px;
      min-width: 0;
    }
    section.full {
      grid-column: 1 / -1;
    }
    .metrics {
      display: grid;
      grid-template-columns: repeat(4, minmax(0, 1fr));
      gap: 10px;
    }
    .metric {
      border: 1px solid #e1e4df;
      border-radius: 6px;
      padding: 10px;
      background: #f8faf9;
    }
    .metric strong {
      display: block;
      font-size: 20px;
      margin-bottom: 2px;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      font-size: 13px;
    }
    th, td {
      text-align: left;
      padding: 8px 6px;
      border-bottom: 1px solid #eceee9;
      vertical-align: top;
    }
    th {
      color: #555c63;
      font-weight: 600;
    }
    code {
      font-family: "SFMono-Regular", Consolas, monospace;
      font-size: 12px;
      word-break: break-all;
    }
    .empty {
      color: #737a82;
      font-size: 13px;
      padding: 10px 0 2px;
    }
    @media (max-width: 760px) {
      header {
        flex-direction: column;
      }
      section {
        grid-column: 1 / -1;
      }
      .metrics {
        grid-template-columns: repeat(2, minmax(0, 1fr));
      }
    }
  </style>
</head>
<body>
  <main>
    <header>
      <div>
        <h1>Forge Gateway Dashboard</h1>
        <div class="muted" id="generated">Loading /api/dashboard ...</div>
      </div>
      <div class="status" id="gateway-status" data-ok="false">Loading</div>
    </header>
    <div class="grid">
      <section class="full">
        <h2>Runtime</h2>
        <div class="metrics" id="runtime-metrics"></div>
      </section>
      <section>
        <h2>Runtime Tasks</h2>
        <div id="runtime-tasks"></div>
      </section>
      <section>
        <h2>Sessions</h2>
        <div id="sessions"></div>
      </section>
      <section>
        <h2>Queued Triggers</h2>
        <div id="triggers"></div>
      </section>
      <section>
        <h2>Recent Runs</h2>
        <div id="runs"></div>
      </section>
      <section class="full">
        <h2>Session Inputs</h2>
        <div id="session-inputs"></div>
      </section>
      <section class="full">
        <h2>Event Log</h2>
        <div id="event-log"></div>
      </section>
    </div>
  </main>
  <script>
    const text = (value, fallback = '-') => {
      if (value === null || value === undefined || value === '') return fallback;
      return String(value);
    };

    const time = (value) => {
      if (!value) return '-';
      return new Date(value).toLocaleString();
    };

    const setText = (id, value) => {
      document.getElementById(id).textContent = value;
    };

    const metric = (label, value) => {
      const node = document.createElement('div');
      node.className = 'metric';
      const strong = document.createElement('strong');
      strong.textContent = text(value, '0');
      const caption = document.createElement('span');
      caption.className = 'muted';
      caption.textContent = label;
      node.append(strong, caption);
      return node;
    };

    const empty = (message) => {
      const node = document.createElement('div');
      node.className = 'empty';
      node.textContent = message;
      return node;
    };

    const table = (headers, rows) => {
      if (!rows.length) return empty('No records.');
      const table = document.createElement('table');
      const thead = document.createElement('thead');
      const headRow = document.createElement('tr');
      headers.forEach((header) => {
        const th = document.createElement('th');
        th.textContent = header.label;
        headRow.appendChild(th);
      });
      thead.appendChild(headRow);
      const tbody = document.createElement('tbody');
      rows.forEach((row) => {
        const tr = document.createElement('tr');
        headers.forEach((header) => {
          const td = document.createElement('td');
          const value = header.value(row);
          if (header.code) {
            const code = document.createElement('code');
            code.textContent = text(value);
            td.appendChild(code);
          } else {
            td.textContent = text(value);
          }
          tr.appendChild(td);
        });
        tbody.appendChild(tr);
      });
      table.append(thead, tbody);
      return table;
    };

    const replace = (id, node) => {
      const target = document.getElementById(id);
      target.replaceChildren(node);
    };

    const render = (snapshot) => {
      const status = snapshot.status || {};
      const badge = document.getElementById('gateway-status');
      badge.dataset.ok = String(Boolean(snapshot.ok));
      badge.textContent = snapshot.ok ? 'Healthy' : 'Needs attention';
      setText('generated', `Generated ${time(snapshot.generated_at_ms)} - ${text(status.message)}`);

      const metrics = document.getElementById('runtime-metrics');
      metrics.replaceChildren(
        metric('uptime seconds', status.uptime_seconds),
        metric('active sessions', status.active_sessions),
        metric('pending triggers', status.pending_triggers),
        metric('pending inputs', status.pending_session_inputs)
      );

      replace('runtime-tasks', table([
        { label: 'Task', value: (row) => row.name, code: true },
        { label: 'Running', value: (row) => row.running ? 'yes' : 'no' },
        { label: 'Last start', value: (row) => time(row.last_started_at_ms) },
        { label: 'Last error', value: (row) => row.last_error || '-' }
      ], status.runtime_tasks || []));

      replace('sessions', table([
        { label: 'Session', value: (row) => row.session_id, code: true },
        { label: 'Model', value: (row) => `${text(row.provider)}/${text(row.model)}` },
        { label: 'Workspace', value: (row) => row.workspace_path, code: true },
        { label: 'Last seen', value: (row) => time(row.last_seen_at_ms || row.created_at_ms) }
      ], snapshot.sessions || []));

      replace('triggers', table([
        { label: 'Trigger', value: (row) => row.id, code: true },
        { label: 'Message', value: (row) => row.message },
        { label: 'Model', value: (row) => row.model || row.provider || '-' },
        { label: 'Received', value: (row) => time(row.received_at_ms) }
      ], snapshot.queued_triggers || []));

      replace('runs', table([
        { label: 'Run', value: (row) => row.id, code: true },
        { label: 'Status', value: (row) => row.status },
        { label: 'Message', value: (row) => row.message },
        { label: 'Session', value: (row) => row.session_id || '-', code: true }
      ], snapshot.recent_runs || []));

      replace('session-inputs', table([
        { label: 'Input', value: (row) => row.input_id, code: true },
        { label: 'Session', value: (row) => row.session_id, code: true },
        { label: 'Message', value: (row) => row.message_preview },
        { label: 'Completed', value: (row) => time(row.completed_at_ms) }
      ], snapshot.recent_session_inputs || []));

      replace('event-log', table([
        { label: 'Kind', value: (row) => row.kind },
        { label: 'Id', value: (row) => row.id, code: true },
        { label: 'Message', value: (row) => row.message },
        { label: 'At', value: (row) => time(row.at_ms) },
        { label: 'Session', value: (row) => row.session_id || '-', code: true }
      ], snapshot.event_log || []));
    };

    fetch('/api/dashboard')
      .then((response) => response.json())
      .then(render)
      .catch((error) => {
        setText('generated', String(error));
        setText('gateway-status', 'Unavailable');
      });
  </script>
</body>
</html>"#
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    use crate::gateway::server::{GatewayDashboardSnapshot, GatewayState};

    #[test]
    fn dashboard_api_request_returns_snapshot_json() {
        let state = Arc::new(GatewayState::new_with_session_registry_path(
            tempfile::tempdir()
                .expect("tempdir")
                .path()
                .join("gateway-sessions.json"),
        ));

        let response = super::dashboard_response_for_request_line(
            Arc::clone(&state),
            "GET /api/dashboard HTTP/1.1",
        )
        .expect("response");

        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("content-type: application/json\r\n"));
        let body = response.split("\r\n\r\n").nth(1).expect("body");
        let snapshot: GatewayDashboardSnapshot =
            serde_json::from_str(body).expect("dashboard snapshot json");
        assert!(snapshot.ok);
    }

    #[test]
    fn dashboard_root_request_returns_readable_dashboard_shell() {
        let state = Arc::new(GatewayState::new_with_session_registry_path(
            tempfile::tempdir()
                .expect("tempdir")
                .path()
                .join("gateway-sessions.json"),
        ));

        let response =
            super::dashboard_response_for_request_line(state, "GET / HTTP/1.1").expect("response");

        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("content-type: text/html; charset=utf-8\r\n"));
        assert!(response.contains("Forge Gateway Dashboard"));
        assert!(response.contains("/api/dashboard"));
        assert!(response.contains("Runtime Tasks"));
        assert!(response.contains("Sessions"));
        assert!(response.contains("Queued Triggers"));
        assert!(response.contains("Event Log"));
    }

    #[test]
    fn dashboard_rejects_non_get_requests() {
        let state = Arc::new(GatewayState::new_with_session_registry_path(
            tempfile::tempdir()
                .expect("tempdir")
                .path()
                .join("gateway-sessions.json"),
        ));

        let response =
            super::dashboard_response_for_request_line(state, "POST /api/dashboard HTTP/1.1")
                .expect("response");

        assert!(response.starts_with("HTTP/1.1 405 Method Not Allowed\r\n"));
    }

    #[tokio::test]
    async fn dashboard_stream_writes_http_response() {
        let state = Arc::new(GatewayState::new_with_session_registry_path(
            tempfile::tempdir()
                .expect("tempdir")
                .path()
                .join("gateway-sessions.json"),
        ));
        let (mut client, server) = tokio::io::duplex(4096);

        let server_task = tokio::spawn(super::handle_dashboard_stream(state, server));
        client
            .write_all(b"GET /api/dashboard HTTP/1.1\r\nhost: localhost\r\n\r\n")
            .await
            .expect("write request");
        client.shutdown().await.expect("shutdown write");

        let mut response = String::new();
        client
            .read_to_string(&mut response)
            .await
            .expect("read response");
        server_task.await.expect("server task").expect("server ok");

        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("content-type: application/json\r\n"));
    }

    #[tokio::test]
    async fn dashboard_listener_accepts_one_http_request() {
        let state = Arc::new(GatewayState::new_with_session_registry_path(
            tempfile::tempdir()
                .expect("tempdir")
                .path()
                .join("gateway-sessions.json"),
        ));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("local addr");

        let server_task =
            tokio::spawn(
                async move { super::accept_dashboard_connection_once(state, &listener).await },
            );
        let mut client = TcpStream::connect(addr).await.expect("connect dashboard");
        client
            .write_all(b"GET / HTTP/1.1\r\nhost: localhost\r\n\r\n")
            .await
            .expect("write request");
        client.shutdown().await.expect("shutdown write");

        let mut response = String::new();
        client
            .read_to_string(&mut response)
            .await
            .expect("read response");
        server_task.await.expect("server task").expect("server ok");

        assert!(response.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(response.contains("Forge Gateway Dashboard"));
    }
}
