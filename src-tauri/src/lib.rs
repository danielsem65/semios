use std::sync::Mutex;

use serde::Serialize;
#[cfg(desktop)]
use tauri::Emitter;
use tauri::{
    webview::PageLoadEvent, AppHandle, Manager, Runtime, Webview, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_opener::OpenerExt;
use url::Url;

use percent_encoding::percent_decode_str;

mod logging;

const MAIN: &str = "main";
const BRIDGE_SCHEME: &str = "semios";
const HOME_URL: &str = "https://duckduckgo.com/";
const OVERLAY: &str = include_str!("../overlay/inject.js");

#[derive(Default)]
struct Session {
    history: Vec<String>,
    index: usize,
    loading: bool,
}

struct SessionState(Mutex<Session>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    url: String,
    loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
    platform: String,
}

#[tauri::command]
async fn browser_state(app: AppHandle<tauri::Wry>) -> Snapshot {
    snapshot(&app)
}

#[tauri::command]
async fn browser_command(app: AppHandle<tauri::Wry>, action: String, arg: Option<String>) {
    dispatch(&app, &action, arg.as_deref());
}

/// Diagnostics sink for the injected script and the start page. A blank window
/// with no console is otherwise impossible to explain after the fact.
#[tauri::command]
async fn browser_log(level: String, message: String) {
    logging::write(&level, &message);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    logging::init();
    logging::write("INFO", &format!("run start platform={}", platform()));

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(SessionState(Mutex::new(Session::default())))
        .invoke_handler(tauri::generate_handler![
            browser_state,
            browser_command,
            browser_log
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let nav = handle.clone();

            #[allow(unused_mut)]
            let mut builder = WebviewWindowBuilder::new(&handle, MAIN, WebviewUrl::App("index.html".into()))
                .title("Semios")
                .initialization_script(OVERLAY)
                .on_navigation(move |url| {
                    logging::write("INFO", &format!("navigation {}", url));
                    if url.scheme() == BRIDGE_SCHEME {
                        bridge(&nav, url);
                        return false;
                    }
                    record(&nav, url);
                    true
                })
                .on_page_load(|window, payload| {
                    let app = window.app_handle().clone();
                    let url = webview(&app)
                        .and_then(|webview| webview.url().ok())
                        .map(|url| url.to_string())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    match payload.event() {
                        PageLoadEvent::Started => {
                            logging::write("INFO", &format!("page load started {url}"));
                            set_loading(&app, true)
                        }
                        PageLoadEvent::Finished => {
                            logging::write("INFO", &format!("page load finished {url}"));
                            set_loading(&app, false);
                            publish(&app);
                        }
                    }
                });

            // Sizing and resizing are desktop only. Android and iOS own their layout,
            // and asking for a 1280x840 window on a phone is not a valid request.
            #[cfg(desktop)]
            {
                builder = builder
                    .inner_size(1280.0, 840.0)
                    .min_inner_size(360.0, 480.0)
                    .resizable(true);
            }

            match builder.build() {
                Ok(_) => logging::write("INFO", "window built"),
                Err(error) => {
                    // A window without the toolbar beats a process that cannot start at all.
                    logging::write("ERROR", &format!("window build failed ({error})"));
                    eprintln!("semios: window build failed ({error}), retrying without extras");
                    if handle.get_webview_window(MAIN).is_none() {
                        let fallback =
                            WebviewWindowBuilder::new(&handle, MAIN, WebviewUrl::App("index.html".into()));
                        match fallback.build() {
                            Ok(_) => logging::write("WARN", "fallback window built without toolbar"),
                            Err(fallback_error) => logging::write(
                                "ERROR",
                                &format!("fallback window build failed ({fallback_error})"),
                            ),
                        }
                    }
                }
            }

            Ok(())
        });

    if let Err(error) = app.run(tauri::generate_context!()) {
        // Never panic: on Android a panic here kills the process behind the splash screen.
        logging::write("ERROR", &format!("run failed: {error}"));
        eprintln!("semios: run failed: {error}");
    } else {
        logging::write("INFO", "run finished");
    }
}

fn webview<R: Runtime>(app: &AppHandle<R>) -> Option<Webview<R>> {
    app.get_webview_window(MAIN)
        .map(|window| window.as_ref().clone())
}

/// On desktop the webview accepts scripts from any thread. On Android, wry
/// drops the receiver end of its main pipe once the first page commits, so
/// every later `eval` aborts the process with `SendError(..)` regardless of the
/// calling thread. Mobile therefore never evaluates: the toolbar keeps its own
/// state in the page and is driven from there instead.
#[cfg(desktop)]
fn eval<R: Runtime>(app: &AppHandle<R>, script: &str) {
    if let Some(webview) = webview(app) {
        if let Err(error) = webview.eval(script) {
            logging::write("WARN", &format!("eval failed: {error}"));
        }
    }
}

#[cfg(mobile)]
fn eval<R: Runtime>(_app: &AppHandle<R>, script: &str) {
    logging::write("WARN", &format!("eval skipped on mobile: {script}"));
}

fn set_loading<R: Runtime>(app: &AppHandle<R>, loading: bool) {
    let state = app.state::<SessionState>();
    let mut session = state.0.lock().unwrap_or_else(|error| error.into_inner());
    session.loading = loading;
}

fn record<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    let target = url.as_str().to_string();
    let state = app.state::<SessionState>();
    let mut session = state.0.lock().unwrap_or_else(|error| error.into_inner());
    if session.history.get(session.index) == Some(&target) {
        return;
    }
    let index = session.index;
    session.history.truncate(index);
    session.history.push(target);
    session.index = session.history.len() - 1;
}

fn snapshot<R: Runtime>(app: &AppHandle<R>) -> Snapshot {
    let state = app.state::<SessionState>();
    let session = state.0.lock().unwrap_or_else(|error| error.into_inner());
    let url = webview(app)
        .and_then(|webview| webview.url().ok())
        .map(|url| url.to_string())
        .or_else(|| session.history.get(session.index).cloned())
        .unwrap_or_else(|| HOME_URL.to_string());

    Snapshot {
        url,
        loading: session.loading,
        can_go_back: session.index > 0,
        can_go_forward: session.index + 1 < session.history.len(),
        platform: platform().to_string(),
    }
}

fn publish<R: Runtime>(app: &AppHandle<R>) {
    // `emit` is not free of eval: on a webview target Tauri delivers the event
    // by running a dispatch script, so emitting on Android aborts the process
    // just as a direct eval would. The toolbar there reads state from the page
    // itself, so mobile only needs the bookkeeping.
    #[cfg(desktop)]
    {
        let state = snapshot(app);
        let _ = app.emit("semios-state", &state);
        if let Ok(json) = serde_json::to_string(&state) {
            let script = String::from("window.__semios && window.__semios.update(") + &json + ")";
            eval(app, &script);
        }
    }
    #[cfg(mobile)]
    let _ = app;
}

fn dispatch<R: Runtime>(app: &AppHandle<R>, action: &str, arg: Option<&str>) {
    logging::write("INFO", &format!("command {action} arg={}", arg.unwrap_or("-")));
    match action {
        "go" => {
            if let Some(target) = arg {
                set_loading(app, true);
                navigate(app, target);
            }
        }
        "home" => {
            set_loading(app, true);
            navigate(app, HOME_URL);
        }
        "back" => eval(app, "window.history.back()"),
        "forward" => eval(app, "window.history.forward()"),
        "reload" => {
            set_loading(app, true);
            #[cfg(desktop)]
            eval(app, "window.location.reload()");
            // Re-navigating is the only reload available without eval, and it
            // still has to clear loading if the current URL is unreadable.
            #[cfg(mobile)]
            match webview(app).and_then(|webview| webview.url().ok()) {
                Some(url) => navigate(app, url.as_str()),
                None => set_loading(app, false),
            }
        }
        "stop" => {
            set_loading(app, false);
            eval(app, "window.stop()");
        }
        "external" => {
            if let Some(target) = arg {
                let _ = app.opener().open_url(target.to_string(), None::<String>);
            }
        }
        "close" => {
            let _ = app.exit(0);
            return;
        }
        _ => {}
    }
    publish(app);
}

fn navigate<R: Runtime>(app: &AppHandle<R>, target: &str) {
    let Ok(url) = Url::parse(target) else {
        logging::write("WARN", &format!("navigate rejected unparsable target {target}"));
        return;
    };

    logging::write("INFO", &format!("navigate to {url}"));

    if url.scheme() == "http" || url.scheme() == "https" {
        if let Some(webview) = webview(app) {
            let _ = webview.navigate(url);
        }
        return;
    }

    let _ = app.opener().open_url(url.to_string(), None::<String>);
}

fn bridge<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    let Some(action) = url.host_str() else {
        return;
    };
    let raw = url.path().trim_start_matches('/');
    let arg = if raw.is_empty() {
        None
    } else {
        Some(
            percent_decode_str(raw)
                .decode_utf8_lossy()
                .into_owned(),
        )
    };

    match action {
        "close" => {
            let _ = app.exit(0);
        }
        "external" => {
            if let Some(target) = arg {
                let _ = app.opener().open_url(target, None::<String>);
            }
        }
        _ => {}
    }
}

fn platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        "ios" => "ios",
        "android" => "android",
        "linux" => "linux",
        _ => "unknown",
    }
}
