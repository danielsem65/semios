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

/// Android gives a launched app no environment variables, a release-signed APK's
/// own files are unreachable from the harness, and scoped storage denies even the
/// app's own `Android/data` directory on current images. A system property is the
/// one channel left: `adb shell setprop` needs no root for custom properties and
/// bionic exports the getter to every process.
#[cfg(mobile)]
fn read_trigger() -> Option<String> {
    use std::ffi::{c_char, c_int};

    extern "C" {
        fn __system_property_get(name: *const c_char, value: *mut c_char) -> c_int;
    }

    let name = b"semios.smoke_url\0";
    // PROP_VALUE_MAX, from <sys/system_properties.h>.
    let mut value = vec![0 as c_char; 92];
    let len = unsafe { __system_property_get(name.as_ptr() as *const c_char, value.as_mut_ptr()) };
    if len <= 0 {
        crate::logging::write("INFO", "smoke trigger: semios.smoke_url is not set");
        return None;
    }
    let bytes = unsafe { std::slice::from_raw_parts(value.as_ptr() as *const u8, len as usize) };
    let text = String::from_utf8_lossy(bytes).trim().to_string();
    crate::logging::write("INFO", &format!("smoke trigger from property: {text}"));
    Some(text)
}
