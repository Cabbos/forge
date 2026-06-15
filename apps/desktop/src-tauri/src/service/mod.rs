//! OS service management — install, uninstall, start, stop, status.
//!
//! Platform support:
//! - macOS: launchd plist at `~/Library/LaunchAgents/com.forge.gateway.plist`
//! - Linux: systemd user unit at `~/.config/systemd/user/forge-gateway.service`
//! - Windows: service wrapper command plan

pub mod launchd;
pub mod systemd;
pub mod windows;
