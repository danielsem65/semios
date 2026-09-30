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
#[cfg(mobile)]
fn read_trigger() -> Option<String> {
    const PACKAGE: &str = "app.semios.browser";
    std::env::var("EXTERNAL_STORAGE").ok().and_then(|root| {
        std::fs::read_to_string(
            std::path::PathBuf::from(root)
                .join("Android/data")
                .join(PACKAGE)
                .join("files/semios-smoke.txt"),
        )
        .ok()
    })
}
