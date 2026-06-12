//! OS service management — install, uninstall, start, stop, status.
//!
//! Platform support:
//! - macOS: launchd plist at `~/Library/LaunchAgents/com.forge.gateway.plist`
//! - Linux: systemd user unit (deferred)
//! - Windows: service wrapper (deferred)

pub mod launchd;
