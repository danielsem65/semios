use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

/// Resolve the log file and start a fresh one for this launch. A stale log from
/// a previous run is worse than no log at all, so it is truncated here.
pub fn init() {
    let path = resolve_path();
    if let Some(path) = path.as_ref() {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(path, b"");
    }
    let _ = LOG_PATH.set(path);

    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        write("PANIC", &info.to_string());
        // Android aborts inside wry, where the file and line alone do not say
        // which of our calls reached the main pipe. The frames do, and a
        // release-signed app cannot read its own log file from a test harness,
        // so the backtrace also goes to stderr where logcat collects it.
        let trace = format!("{}", std::backtrace::Backtrace::force_capture());
        write("PANIC", &trace);
        eprintln!("semios panic backtrace:\n{trace}");
        previous(info);
    }));
}

fn resolve_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("SEMIOS_LOG") {
        if !explicit.trim().is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }
    if cfg!(target_os = "windows") {
        if let Ok(base) = std::env::var("LOCALAPPDATA") {
            return Some(PathBuf::from(base).join("Semios").join("semios.log"));
        }
    }
    if cfg!(target_os = "macos") {
        if let Ok(home) = std::env::var("HOME") {
            return Some(
                PathBuf::from(home)
                    .join("Library")
                    .join("Logs")
                    .join("semios.log"),
            );
        }
    }
    if cfg!(target_os = "linux") {
        if let Ok(home) = std::env::var("HOME") {
            return Some(
                PathBuf::from(home)
                    .join(".local")
                    .join("state")
                    .join("semios")
                    .join("semios.log"),
            );
        }
    }
    // Android does not always export HOME, but the app cache dir is always writable.
    Some(std::env::temp_dir().join("semios.log"))
}

pub fn write(level: &str, message: &str) {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or_default();
    let line = format!("{millis}\t{level}\t{}\n", message.replace('\n', " "));
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
    // On Android a release-signed app cannot hand its log file to a test
    // harness, so stderr is the only channel that reaches logcat. Mirror
    // everything there and let the smoke test assert on it.
    if cfg!(target_os = "android") {
        eprintln!("semios {level}: {message}");
    }
}
