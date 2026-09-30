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

/// True in the CI builds, so anything that would add nondeterminism to a smoke
/// run can opt out. The desktop harness hands the target over in the
/// environment; the mobile one bakes it in, because there is no channel a
/// release-signed APK can be handed one through.
pub fn armed() -> bool {
    #[cfg(desktop)]
    {
        std::env::var("SEMIOS_SMOKE_URL").is_ok()
    }
    #[cfg(mobile)]
    {
        option_env!("SEMIOS_SMOKE_URL").is_some()
    }
}

#[cfg(desktop)]
fn read_trigger() -> Option<String> {
    std::env::var("SEMIOS_SMOKE_URL").ok()
}

/// Android has no environment channel a harness can reach: a launched app
/// inherits nothing from the shell, `run-as` is refused on a release-signed APK,
/// scoped storage denies even the app's own `Android/data` directory, and
/// `adb shell setprop` is refused for custom properties. So resolve the target
/// when the APK is compiled instead. `option_env!` is fixed up by rustc, which
/// keeps the release-signed, minified artifact intact and needs no permission at
/// runtime. Only the smoke workflow exports the variable, so shipped APKs have
/// no trigger baked in and this stays inert.
#[cfg(mobile)]
fn read_trigger() -> Option<String> {
    let target = option_env!("SEMIOS_SMOKE_URL")?;
    crate::logging::write("INFO", &format!("smoke trigger baked into this build: {target}"));
    Some(target.to_string())
}
