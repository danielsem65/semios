//! Update checks against the project's own GitHub releases.
//!
//! The check runs in Rust rather than in the page on purpose. The toolbar is
//! injected into sites we do not control, and a foreign origin has no Tauri IPC
//! at all, so a check started from script would silently work on the start page
//! and nowhere else. Android cannot even be pushed an answer, so Rust owns the
//! whole lifecycle here: it asks GitHub what the newest tag is, compares that
//! with the running build, and publishes the verdict into the same snapshot the
//! toolbar already renders.
//!
//! `tauri-plugin-updater` was rejected on purpose: it is desktop only, so it
//! would leave Android without updates, and it wants a second signing scheme on
//! the release artifacts. GitHub Releases are already the distribution channel,
//! so this reads what is already published.

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
// The plugin crate re-exports reqwest, and depending on it rather than on
// reqwest directly means the TLS backend, HTTP/2 and system proxy support are
// the set the Tauri ecosystem settled on, on every platform we ship to. The
// plugin itself is never registered: nothing here goes through the webview, so
// the JavaScript side of it would be dead weight.
use tauri_plugin_http::reqwest;

use crate::logging;

const RELEASES: &str = "https://api.github.com/repos/danielsem65/semios/releases/latest";
const AGENT: &str = concat!("semios/", env!("CARGO_PKG_VERSION"));
/// GitHub rejects API calls without a user agent, and refuses to compare
/// anything that is not a release.
const CACHE: &str = "update-check.json";
/// Unauthenticated GitHub API calls are limited per IP address, and an app like
/// this is often several people behind one address. Checking once a day keeps a
/// launch from spending a shared budget that a developer might need.
const MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// What the toolbar needs to decide whether to offer an update. Serialized to
/// the page, so the field names are camel cased.
#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    /// A newer release exists and carries an installable artifact for this OS.
    pub available: bool,
    pub version: String,
    /// Release notes, so the user can judge the jump before installing.
    pub notes: String,
    /// The release page, for a manual install or to read the notes.
    pub url: String,
    /// The installable artifact itself: `.exe` on Windows, `.apk` on Android.
    pub asset: String,
    /// False until a check has actually run, so the toolbar can stay quiet
    /// while offline instead of claiming there is nothing to install.
    pub checked: bool,
}

pub struct UpdateState(pub Mutex<Update>);

/// Called once at startup. Deliberately never blocks the window: a browser that
/// will not open because GitHub is slow is a much worse bug than a missed update.
pub async fn check<R: Runtime>(app: &AppHandle<R>) {
    if crate::smoke::armed() {
        logging::write("INFO", "update check skipped: this is a smoke build");
        return;
    }

    let current = app.package_info().version.to_string();
    let cache = read_cache(app);
    if let Some(cache) = cache.as_ref() {
        if age(cache.at) < MAX_AGE {
            logging::write("INFO", &format!("update check skipped: cached {}", cache.update.version));
            apply(app, cache.update.clone());
            return;
        }
    }

    match fetch(&current).await {
        Ok(update) => {
            if update.available {
                logging::write("INFO", &format!("update available {}", update.version));
            } else {
                logging::write("INFO", "up to date");
            }
            write_cache(app, &update);
            apply(app, update);
        }
        Err(error) => {
            // Offline, rate limited, or GitHub moved something. Never fatal, and
            // never worth nagging the user about from the toolbar.
            logging::write("WARN", &format!("update check failed: {error}"));
            // A stale verdict is still worth showing: a release published after
            // the last successful check is exactly what the user wants to know.
            if let Some(cache) = cache {
                apply(app, cache.update);
            }
        }
    }
}

/// Windows can replace itself: fetch the installer and run it. Android cannot.
/// The platform has no supported way for an app to install a package over
/// itself, and the supported route wants `REQUEST_INSTALL_PACKAGES` plus a
/// FileProvider, so the APK is handed to the system instead. The browser
/// downloads it and the package installer takes over, which is the normal
/// sideload flow and needs no permission from us.
pub async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let update = current(app);
    if !update.available {
        return Err("no update is available".to_string());
    }
    if update.asset.is_empty() {
        return Err("this release has no artifact for this platform".to_string());
    }
    logging::write("INFO", &format!("installing {}", update.version));
    install_platform(app, &update.asset, &update.version).await
}

#[cfg(desktop)]
async fn install_platform<R: Runtime>(
    app: &AppHandle<R>,
    asset: &str,
    version: &str,
) -> Result<(), String> {
    let bytes = download(asset).await?;
    let path = std::env::temp_dir().join(format!("semios-setup-{version}.exe"));
    std::fs::write(&path, &bytes).map_err(|error| format!("could not save the installer: {error}"))?;
    logging::write("INFO", &format!("installer saved to {}", path.display()));

    // NSIS replaces the running install and asks the app to close, so step out
    // of its way rather than holding a lock on the files it is about to write.
    std::process::Command::new(&path)
        .spawn()
        .map_err(|error| format!("could not start the installer: {error}"))?;
    let _ = app.exit(0);
    Ok(())
}

#[cfg(mobile)]
async fn install_platform<R: Runtime>(
    app: &AppHandle<R>,
    asset: &str,
    _version: &str,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;

    app.opener()
        .open_url(asset.to_string(), None::<String>)
        .map_err(|error| format!("could not hand the download to the system: {error}"))?;
    logging::write("INFO", "download handed to the system installer");
    Ok(())
}

async fn fetch(current: &str) -> Result<Update, String> {
    let body = get(RELEASES).await?;
    let release: serde_json::Value =
        serde_json::from_str(&body).map_err(|error| format!("unreadable release: {error}"))?;

    let tag = release
        .get("tag_name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    if tag.is_empty() {
        return Err("release has no tag".to_string());
    }
    let asset = pick_asset(&release);
    let available = is_newer(tag, current) && asset.is_some();

    Ok(Update {
        available,
        version: tag.trim_start_matches(['v', 'V']).to_string(),
        notes: release
            .get("body")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        url: release
            .get("html_url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string(),
        asset: asset.unwrap_or_default(),
        checked: true,
    })
}

/// Built per call rather than cached: this runs at most twice per launch, and a
/// client kept alive for the life of the process would outlive its only purpose.
fn client(timeout: Duration) -> Result<reqwest::Client, String> {
    // `build` is fallible where `new` panics, and a browser must not die because
    // a TLS backend refused to start.
    reqwest::Client::builder()
        .user_agent(AGENT)
        .timeout(timeout)
        .build()
        .map_err(|error| error.to_string())
}

async fn get(url: &str) -> Result<String, String> {
    let response = client(Duration::from_secs(20))?
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("{url} replied {status}"));
    }
    response.text().await.map_err(|error| error.to_string())
}

#[cfg(desktop)]
async fn download(url: &str) -> Result<Vec<u8>, String> {
    // Generous: this is a whole installer over whatever connection the user has.
    let response = client(Duration::from_secs(600))?
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("download failed with {status}"));
    }
    response.bytes().await.map(|bytes| bytes.to_vec()).map_err(|error| error.to_string())
}

/// Find the artifact that can actually be installed on this platform. A release
/// that published only the other platform's file is not an update as far as
/// this build is concerned, so requiring a match is what keeps `available`
/// honest.
fn pick_asset(release: &serde_json::Value) -> Option<String> {
    let wanted: &[&str] = if cfg!(target_os = "android") {
        &[".apk"]
    } else if cfg!(target_os = "windows") {
        &[".exe"]
    } else {
        return None;
    };
    release
        .get("assets")
        .and_then(serde_json::Value::as_array)?
        .iter()
        .filter_map(|asset| {
            let name = asset.get("name").and_then(serde_json::Value::as_str)?;
            let url = asset.get("browser_download_url").and_then(serde_json::Value::as_str)?;
            Some((name.to_ascii_lowercase(), url.to_string()))
        })
        .find(|(name, _)| wanted.iter().any(|suffix| name.ends_with(*suffix)))
        .map(|(_, url)| url)
}

/// Compare `tag` against the running version. Only the numeric core is
/// compared, and it is padded so `0.2` beats `0.1.9`. A pre-release suffix is
/// ignored, which is the safe direction to be wrong in: it can offer an update
/// that is not strictly newer, never the reverse.
fn is_newer(tag: &str, current: &str) -> bool {
    fn core(version: &str) -> Vec<u64> {
        let trimmed = version.trim().trim_start_matches(['v', 'V']);
        let numeric = trimmed.split(['-', '+']).next().unwrap_or_default();
        numeric
            .split('.')
            .map(|part| part.parse::<u64>().unwrap_or_default())
            .collect()
    }
    let (tag, current) = (core(tag), core(current));
    for index in 0..tag.len().max(current.len()) {
        let left = tag.get(index).copied().unwrap_or_default();
        let right = current.get(index).copied().unwrap_or_default();
        if left != right {
            return left > right;
        }
    }
    false
}

fn age(stamp: u64) -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    Duration::from_secs(now.saturating_sub(stamp))
}

#[derive(Serialize, Deserialize)]
struct Record {
    at: u64,
    #[serde(flatten)]
    update: Update,
}

fn cache_path<R: Runtime>(app: &AppHandle<R>) -> Option<std::path::PathBuf> {
    Some(app.path().app_config_dir().ok()?.join(CACHE))
}

fn read_cache<R: Runtime>(app: &AppHandle<R>) -> Option<Record> {
    let path = cache_path(app)?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn write_cache<R: Runtime>(app: &AppHandle<R>, update: &Update) {
    let Some(path) = cache_path(app) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default();
    if let Ok(raw) = serde_json::to_string(&Record { at, update: update.clone() }) {
        let _ = std::fs::write(path, raw);
    }
}

fn current<R: Runtime>(app: &AppHandle<R>) -> Update {
    app.state::<UpdateState>()
        .0
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone()
}

fn apply<R: Runtime>(app: &AppHandle<R>, update: Update) {
    {
        let state = app.state::<UpdateState>();
        let mut slot = state.0.lock().unwrap_or_else(|error| error.into_inner());
        *slot = update;
    }
    // Only reachable when the check finished after the window was already up,
    // which is the only case where the toolbar has not seen the answer yet.
    crate::publish(app);
}
