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
</head>
<body>
  <main>
    <h1>Forge Gateway Dashboard</h1>
    <pre id="snapshot">Loading /api/dashboard ...</pre>
  </main>
  <script>
    fetch('/api/dashboard')
      .then((response) => response.json())
      .then((snapshot) => {
        document.getElementById('snapshot').textContent = JSON.stringify(snapshot, null, 2);
      })
      .catch((error) => {
        document.getElementById('snapshot').textContent = String(error);
      });
  </script>
</body>
</html>"#
}

fn http_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.as_bytes().len()
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
    fn dashboard_root_request_returns_minimal_html_shell() {
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
