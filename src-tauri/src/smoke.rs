//! Opt-in navigation smoke test.
//!
//! The abort this guards against came from a wry main-pipe round trip that runs
//! on every page load, so simply watching the start page never covered it. The
//! remote-page load and the reload are the paths that actually matter: they are
//! what attach the toolbar to a page we do not own, and what a user hits when a
//! site misbehaves. CI asks the app to visit a throwaway URL and then asserts on
//! the log.

use std::sync::atomic::{AtomicBool, Ordering};

static CONSUMED: AtomicBool = AtomicBool::new(false);

/// The URL CI wants visited. Claimed on first read so a leftover trigger file
/// cannot send a real user's build off to a test page on every launch.
pub fn take_target() -> Option<String> {
    if CONSUMED.swap(true, Ordering::SeqCst) {
        return None;
    }
    read_trigger()
        .map(|target| target.trim().to_string())
        .filter(|target| !target.is_empty())
}

#[cfg(desktop)]
fn read_trigger() -> Option<String> {
    std::env::var("SEMIOS_SMOKE_URL").ok()
}

/// A release-signed APK cannot be read back by `run-as`, so the emulator
/// harness cannot hand us a file inside the app sandbox. App-specific external
/// storage needs no runtime permission and is still writable by adb, which
/// makes it the one channel that works without pulling in another plugin.
///
/// Every candidate is logged: this path differs across Android versions, and a
/// trigger that silently fails looks exactly like a navigation test that was
/// never wired up.
#[cfg(mobile)]
fn read_trigger() -> Option<String> {
    use std::path::PathBuf;

    const PACKAGE: &str = "app.semios.browser";
    const NAME: &str = "semios-smoke.txt";

    let mut roots: Vec<PathBuf> = Vec::new();
    match std::env::var("EXTERNAL_STORAGE") {
        Ok(root) => roots.push(PathBuf::from(root)),
        Err(_) => crate::logging::write("WARN", "smoke trigger: EXTERNAL_STORAGE is not set"),
    }
    roots.push(PathBuf::from("/sdcard"));
    roots.push(PathBuf::from("/storage/emulated/0"));

    for root in roots {
        let path = root.join("Android/data").join(PACKAGE).join("files").join(NAME);
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                crate::logging::write("INFO", &format!("smoke trigger found {}", path.display()));
                return Some(text);
            }
            Err(error) => crate::logging::write(
                "WARN",
                &format!("smoke trigger miss {} ({error})", path.display()),
            ),
        }
    }
    crate::logging::write("WARN", "smoke trigger not found on any candidate path");
    None
}
