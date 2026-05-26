use std::time::Duration;

pub(crate) const ASK_USER_TIMEOUT: Duration = Duration::from_secs(300);
pub(crate) const CONFIRM_TIMEOUT: Duration = Duration::from_secs(120);

pub(crate) const SHELL_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const SHELL_OUTPUT_LIMIT: usize = 100 * 1024;
pub(crate) const SEARCH_TIMEOUT: Duration = Duration::from_secs(8);

pub(crate) const PROCESS_SHUTDOWN_GRACE: Duration = Duration::from_secs(2);
pub(crate) const PROCESS_LINE_DRAIN_INTERVAL: Duration = Duration::from_millis(50);

pub(crate) const HTTP_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const WEB_SEARCH_TIMEOUT: Duration = Duration::from_secs(10);
pub(crate) const WEB_FETCH_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const AGENT_API_TIMEOUT: Duration = Duration::from_secs(600);

pub(crate) const MCP_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) const AGENT_OVERFLOW_RETRY_DELAY: Duration = Duration::from_secs(2);
pub(crate) const AGENT_LOOP_SETTLE_DELAY: Duration = Duration::from_millis(50);

pub(crate) const DEV_SERVER_STARTUP_GRACE: Duration = Duration::from_millis(800);
pub(crate) const DEV_SERVER_STOP_GRACE: Duration = Duration::from_millis(300);
pub(crate) const DEV_SERVER_PORT_CONNECT_TIMEOUT: Duration = Duration::from_millis(180);
