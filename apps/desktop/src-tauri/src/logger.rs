use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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
    ensure_dir();
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(log_path()) {
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{}] {} {}", secs, level, message);
    }
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
