use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::redaction::{global_redactor, PersistentLogRedactor};

static LOG_MUTEX: Mutex<()> = Mutex::new(());

fn log_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    home.join(".forge").join("app.log")
}

fn ensure_dir() {
    if let Some(parent) = log_path().parent() {
        let _ = fs::create_dir_all(parent);
    }
}

pub fn log(level: &str, message: &str) {
    let _guard = LOG_MUTEX.lock().unwrap();
    let redactor = global_redactor();
    let path = log_path();
    let redacted = match write_plain_log(&path, &redactor, level, message) {
        Ok(redacted) => redacted,
        Err(PlainLogError::Redaction) => {
            eprintln!("Forge log entry suppressed: redaction failed");
            return;
        }
        Err(PlainLogError::Persistence) => {
            eprintln!("Forge log entry suppressed: persistence failed");
            return;
        }
    };

    // Also write to the structured log store, which redacts independently.
    crate::log_store::log_event(level, "app", &redacted, None);
}

#[derive(Debug)]
enum PlainLogError {
    Redaction,
    Persistence,
}

fn write_plain_log(
    path: &Path,
    redactor: &PersistentLogRedactor,
    level: &str,
    message: &str,
) -> Result<String, PlainLogError> {
    let redacted = redactor
        .redact_text(message)
        .map_err(|_| PlainLogError::Redaction)?;
    ensure_dir();
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|_| PlainLogError::Persistence)?;
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    writeln!(file, "[{secs}] {level} {redacted}").map_err(|_| PlainLogError::Persistence)?;
    Ok(redacted)
}

pub fn log_path_str() -> String {
    log_path().to_string_lossy().to_string()
}

/// Log panics to file
pub fn setup_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        log("PANIC", &format!("{:?}", info));
    }));
}

#[macro_export]
macro_rules! app_log {
    ($level:expr, $($arg:tt)*) => {
        $crate::logger::log($level, &format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redaction::PersistentLogRedactor;

    fn temp_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("forge-plain-log-{name}-{nanos}.log"))
    }

    #[test]
    fn plain_log_redacts_before_persistence() {
        let path = temp_path("redacted");
        let redactor = PersistentLogRedactor::new();
        redactor.register_secret("forge-plain-secret");

        write_plain_log(
            &path,
            &redactor,
            "INFO",
            "Authorization: Bearer forge-plain-secret",
        )
        .expect("write redacted log");

        let persisted = fs::read_to_string(&path).expect("read plain log");
        assert!(!persisted.contains("forge-plain-secret"));
        assert!(persisted.contains("Authorization: [redacted]"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn plain_redaction_error_suppresses_persistence() {
        let path = temp_path("redaction-error");
        let redactor = PersistentLogRedactor::new();
        redactor.set_fail_for_test(true);

        let result = write_plain_log(&path, &redactor, "INFO", "must never reach disk");

        assert!(result.is_err());
        assert!(!path.exists());
    }
}
