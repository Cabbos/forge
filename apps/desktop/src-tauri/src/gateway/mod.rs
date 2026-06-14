//! Forge Gateway — background daemon that keeps the runtime alive
//! independent of the desktop app.
//!
//! The gateway listens on a Unix domain socket at `~/.forge/gateway.sock`
//! and speaks a newline-delimited JSON protocol.  Clients (desktop app,
//! CLI) connect to manage sessions, query health, and receive events.

pub mod client;
pub mod protocol;
pub mod runner;
pub mod server;
pub mod webhook;
