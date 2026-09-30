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
mod smoke;
mod update;

const MAIN: &str = "main";
const BRIDGE_SCHEME: &str = "semios";
const HOME_URL: &str = "https://duckduckgo.com/";
const OVERLAY: &str = include_str!("../overlay/inject.js");

/// One tab, with its own history. History has to live per tab or going back in
/// a tab walks the tab you happened to visit before it.
struct Tab {
    id: u64,
    history: Vec<String>,
    index: usize,
    title: String,
    loading: bool,
}

impl Tab {
    fn new(id: u64) -> Self {
        Tab {
            id,
            history: Vec::new(),
            index: 0,
            title: String::new(),
            loading: false,
        }
    }

    fn url(&self) -> Option<&str> {
        self.history.get(self.index).map(String::as_str)
    }

    fn push(&mut self, url: &str) {
        if self.url() == Some(url) {
            return;
        }
        self.history.truncate(self.index);
        self.history.push(url.to_string());
        self.index = self.history.len() - 1;
    }
}

/// Tabs share one webview. Switching a tab navigates the live page to that
/// tab's URL, so exactly one document is ever in memory. That costs a reload on
/// switch and buys working tabs on Android, where every webview window is its
/// own Activity and an in-window tab strip is not something the platform can
/// express at all.
struct Tabs {
    tabs: Vec<Tab>,
    active: usize,
    next_id: u64,
    /// The first document this window ever loaded is our own start page, and a
    /// new tab opens on it. It is captured rather than hardcoded because the
    /// app's own scheme is not the same on every platform.
    start_url: Option<String>,
}

impl Default for Tabs {
    fn default() -> Self {
        Tabs {
            tabs: vec![Tab::new(1)],
            active: 0,
            next_id: 2,
            start_url: None,
        }
    }
}

struct TabsState(Mutex<Tabs>);

/// How far the optional navigation smoke test has got. Zero everywhere except
/// in CI, which is what keeps the release builds inert.
#[derive(Default)]
struct Smoke {
    stage: u8,
    loads: u8,
    target: Option<String>,
}

struct SmokeState(Mutex<Smoke>);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TabInfo {
    id: u64,
    title: String,
    url: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    url: String,
    loading: bool,
    can_go_back: bool,
    can_go_forward: bool,
    platform: String,
    update: update::Update,
    tabs: Vec<TabInfo>,
    active_tab: u64,
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
        .manage(TabsState(Mutex::new(Tabs::default())))
        .manage(SmokeState(Mutex::new(Smoke::default())))
        .manage(update::UpdateState(Mutex::new(update::Update::default())))
        .invoke_handler(tauri::generate_handler![
            browser_state,
            browser_command,
            browser_log
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            let nav = handle.clone();
            let titled = handle.clone();

            #[allow(unused_mut)]
            let mut builder = WebviewWindowBuilder::new(&handle, MAIN, WebviewUrl::App("index.html".into()))
                .title("Semios")
                .initialization_script(OVERLAY)
                .on_document_title_changed(move |_webview, title| {
                    logging::write("DEBUG", &format!("document title {title}"));
                    set_title(&titled, &title);
                })
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
                    // The payload already carries the URL. Asking the webview for
                    // it round-trips through wry's Android main pipe, which is torn
                    // down around the first page commit, and that round trip is
                    // what aborted the process on every single startup.
                    let url = payload.url().to_string();
                    match payload.event() {
                        PageLoadEvent::Started => {
                            logging::write("INFO", &format!("page load started {url}"));
                            remember_start(&app, &url);
                            set_loading(&app, true)
                        }
                        PageLoadEvent::Finished => {
                            logging::write("INFO", &format!("page load finished {url}"));
                            set_loading(&app, false);
                            smoke_step(&app, &url);
                            publish(&app);
                        }
                    }
                });

            // A link that asks for a new window means a new tab. wry reports it
            // here on desktop, and only on desktop: the Android and iOS backends
            // have no equivalent, which is why the injected script also catches
            // the cases it can see. Returning Deny stops wry opening a second
            // OS window behind our back.
            //
            // Clipboard access rides along because the context menu copies a
            // link from inside a page we do not own, and on Windows and Linux
            // that is refused unless the webview opts in. macOS has no such
            // switch and is always permitted.
            #[cfg(desktop)]
            {
                let popup = handle.clone();
                builder = builder
                    .enable_clipboard_access(true)
                    .on_new_window(move |url, _features| {
                        tab_new(&popup, Some(url.as_str()));
                        tauri::webview::NewWindowResponse::Deny
                    });
            }

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

            // After the window exists, and off the critical path. The verdict
            // reaches the toolbar through the ordinary snapshot, so a slow or
            // unreachable GitHub costs nothing but a quiet log line.
            let checker = handle.clone();
            tauri::async_runtime::spawn(async move {
                update::check(&checker).await;
            });

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
    let state = app.state::<TabsState>();
    let mut tabs = state.0.lock().unwrap_or_else(|error| error.into_inner());
    let active = tabs.active;
    if let Some(tab) = tabs.tabs.get_mut(active) {
        tab.loading = loading;
    }
}

fn set_title<R: Runtime>(app: &AppHandle<R>, title: &str) {
    if title.is_empty() {
        return;
    }
    let state = app.state::<TabsState>();
    let mut tabs = state.0.lock().unwrap_or_else(|error| error.into_inner());
    let active = tabs.active;
    if let Some(tab) = tabs.tabs.get_mut(active) {
        tab.title = title.to_string();
    }
}

fn record<R: Runtime>(app: &AppHandle<R>, url: &Url) {
    let target = url.as_str().to_string();
    let state = app.state::<TabsState>();
    let mut tabs = state.0.lock().unwrap_or_else(|error| error.into_inner());
    let active = tabs.active;
    if let Some(tab) = tabs.tabs.get_mut(active) {
        tab.push(&target);
    }
}

/// The very first document this window loads is our own start page, so that is
/// what a new tab opens on. Captured rather than hardcoded because the app's own
/// URL differs by platform and by scheme configuration.
fn remember_start<R: Runtime>(app: &AppHandle<R>, url: &str) {
    let state = app.state::<TabsState>();
    let mut tabs = state.0.lock().unwrap_or_else(|error| error.into_inner());
    if tabs.start_url.is_none() {
        tabs.start_url = Some(url.to_string());
    }
}

fn tab_url(tabs: &Tabs) -> String {
    tabs.tabs
        .get(tabs.active)
        .and_then(|tab| tab.url().map(str::to_string))
        .unwrap_or_else(|| HOME_URL.to_string())
}

/// Opens a tab beside the active one and makes it active. A new tab with no
/// target lands on the start page, which is what the plus button asks for.
fn tab_new<R: Runtime>(app: &AppHandle<R>, target: Option<&str>) {
    let state = app.state::<TabsState>();
    let (id, url) = {
        let mut tabs = state.0.lock().unwrap_or_else(|error| error.into_inner());
        let id = tabs.next_id;
        tabs.next_id += 1;
        let url = match target {
            Some(raw) => raw.to_string(),
            None => tabs
                .start_url
                .clone()
                .unwrap_or_else(|| HOME_URL.to_string()),
        };
        let at = (tabs.active + 1).min(tabs.tabs.len());
        let mut tab = Tab::new(id);
        tab.push(&url);
        tabs.tabs.insert(at, tab);
        tabs.active = at;
        (id, url)
    };
    logging::write("INFO", &format!("new tab {id} at {url}"));
    set_loading(app, true);
    navigate(app, &url);
}

fn tab_select<R: Runtime>(app: &AppHandle<R>, id: u64) {
    let state = app.state::<TabsState>();
    let url = {
        let mut tabs = state.0.lock().unwrap_or_else(|error| error.into_inner());
        let Some(at) = tabs.tabs.iter().position(|tab| tab.id == id) else {
            logging::write("WARN", &format!("select ignored: no tab {id}"));
            return;
        };
        if at == tabs.active {
            return;
        }
        tabs.active = at;
        // The stored title belongs to the page we are leaving behind, so drop it
        // and let the new document name the tab.
        tabs.tabs[at].title.clear();
        tab_url(&tabs)
    };
    logging::write("INFO", &format!("select tab {id}"));
    set_loading(app, true);
    navigate(app, &url);
}

fn tab_close<R: Runtime>(app: &AppHandle<R>, id: u64) {
    let state = app.state::<TabsState>();
    let next = {
        let mut tabs = state.0.lock().unwrap_or_else(|error| error.into_inner());
        let Some(at) = tabs.tabs.iter().position(|tab| tab.id == id) else {
            logging::write("WARN", &format!("close ignored: no tab {id}"));
            return;
        };
        if tabs.tabs.len() == 1 {
            // Closing the last tab is not a reason to exit. Reset it onto the
            // start page, which is what a browser does with its final tab.
            let url = tabs
                .start_url
                .clone()
                .unwrap_or_else(|| HOME_URL.to_string());
            let fresh = tabs.next_id;
            tabs.next_id += 1;
            let mut tab = Tab::new(fresh);
            tab.push(&url);
            tabs.tabs[0] = tab;
            tabs.active = 0;
            url
        } else {
            tabs.tabs.remove(at);
            // Prefer whichever tab slid into this one's place.
            let at = at.min(tabs.tabs.len() - 1);
            tabs.active = at;
            tab_url(&tabs)
        }
    };
    logging::write("INFO", &format!("closed tab {id}"));
    set_loading(app, true);
    navigate(app, &next);
}

fn snapshot<R: Runtime>(app: &AppHandle<R>) -> Snapshot {
    // Read before the tabs lock so only one mutex is ever held here.
    let update = app
        .state::<update::UpdateState>()
        .0
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let state = app.state::<TabsState>();
    let tabs = state.0.lock().unwrap_or_else(|error| error.into_inner());
    // Reading the webview's URL is a main-pipe round trip, so it is desktop
    // only. Mobile reports the URL it recorded while navigating, which stays
    // correct because every navigation goes through the bridge.
    #[cfg(desktop)]
    let live = webview(app)
        .and_then(|webview| webview.url().ok())
        .map(|url| url.to_string());
    #[cfg(mobile)]
    let live: Option<String> = None;

    let current = tabs.tabs.get(tabs.active);
    let list = tabs
        .tabs
        .iter()
        .map(|tab| TabInfo {
            id: tab.id,
            title: tab_title(tab),
            url: tab.url().unwrap_or_default().to_string(),
        })
        .collect();

    Snapshot {
        url: live
            .or_else(|| current.and_then(|tab| tab.url().map(str::to_string)))
            .unwrap_or_else(|| HOME_URL.to_string()),
        loading: current.map(|tab| tab.loading).unwrap_or(false),
        can_go_back: current.map(|tab| tab.index > 0).unwrap_or(false),
        can_go_forward: current
            .map(|tab| tab.index + 1 < tab.history.len())
            .unwrap_or(false),
        platform: platform().to_string(),
        update,
        tabs: list,
        active_tab: current.map(|tab| tab.id).unwrap_or_default(),
    }
}

/// A tab with no title yet still needs something readable, so the host stands
/// in. This is also what Android shows, where a remote page cannot receive a
/// title push.
fn tab_title(tab: &Tab) -> String {
    if !tab.title.is_empty() {
        return tab.title.clone();
    }
    tab.url()
        .and_then(|raw| Url::parse(raw).ok())
        .and_then(|url| url.host_str().map(str::to_string))
        .unwrap_or_else(|| "New tab".to_string())
}

pub(crate) fn publish<R: Runtime>(app: &AppHandle<R>) {
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

/// Downloading an installer is async and can take a while, so it is never run on
/// the caller's task. Shared by the IPC command and the injected overlay: the
/// start page can invoke a command, a remote page can only navigate the bridge,
/// and both have to reach the same place.
fn install_update<R: Runtime>(app: &AppHandle<R>) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = update::install(&handle).await {
            logging::write("ERROR", &format!("update install failed: {error}"));
        }
    });
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
            // Re-navigating is the only reload available without eval.
            #[cfg(mobile)]
            navigate(app, &snapshot(app).url);
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
        "update" => install_update(app),
        "newtab" => tab_new(app, arg),
        "selecttab" => {
            if let Some(id) = arg.and_then(|raw| raw.parse::<u64>().ok()) {
                tab_select(app, id);
            }
        }
        "closetab" => {
            if let Some(id) = arg.and_then(|raw| raw.parse::<u64>().ok()) {
                tab_close(app, id);
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

/// Walks CI through a remote page load, a reload, and a second tab, once the
/// start page has settled. Every stage hangs off a finished page load, so
/// without a trigger from the harness this never does anything at all.
fn smoke_step<R: Runtime>(app: &AppHandle<R>, url: &str) {
    let state = app.state::<SmokeState>();
    let mut smoke = state.0.lock().unwrap_or_else(|error| error.into_inner());
    if smoke.target.is_none() {
        smoke.target = smoke::take_target();
    }
    let Some(target) = smoke.target.clone() else {
        return;
    };

    match smoke.stage {
        // The start page is up and its toolbar has mounted, so there is a window
        // to send somewhere we do not own.
        0 => {
            smoke.stage = 1;
            logging::write("INFO", &format!("smoke navigating to {target}"));
            drop(smoke);
            navigate(app, &target);
        }
        // A remote page finished loading, which means the toolbar had to attach
        // to a document we did not build. Now reload it: desktop evals, mobile
        // re-navigates, and neither path is covered by a plain startup.
        1 if url == target => {
            smoke.stage = 2;
            logging::write("INFO", "smoke reloading the remote page");
            drop(smoke);
            dispatch(app, "reload", None);
        }
        // The reload lands on the same URL, so the stage alone cannot tell the
        // reload's load from the one before it. Count finishes instead.
        2 if url == target => {
            smoke.loads += 1;
            if smoke.loads == 1 {
                smoke.stage = 3;
                logging::write("INFO", "smoke opening a second tab");
                drop(smoke);
                tab_new(app, Some(&target));
            }
        }
        // The second tab reached the same page through the tab machinery rather
        // than a plain navigation, and its toolbar is the one that answers.
        3 if url == target => {
            smoke.stage = 4;
            logging::write(
                "INFO",
                "smoke complete: remote page loaded and reloaded; second tab opened",
            );
            probe_remote_page(app);
        }
        _ => {}
    }
}

/// Asks the loaded site what it can actually see, and sends the answer back over
/// the bridge. The injected script cannot report in from a foreign origin, so
/// this is the only way to learn whether the toolbar is on screen, whether the
/// page underneath it is still clickable, and whether the tab list rendered at
/// all. Desktop only, because it needs eval.
#[cfg(desktop)]
fn probe_remote_page<R: Runtime>(app: &AppHandle<R>) {
    const PROBE: &str = r#"(function(){
var host=document.getElementById('semios-overlay');
var root=host&&host.shadowRoot;
var bar=root?root.querySelector('.bar'):null;
var list=root?root.querySelector('.tablist'):null;
var chromePx=host?Math.round(host.getBoundingClientRect().height):0;
var inset=Math.round(parseFloat(getComputedStyle(document.documentElement).paddingTop)||0);
var hit=document.elementFromPoint(Math.round(innerWidth/2),chromePx+16);
var blocked=!!(hit&&hit.closest&&hit.closest('#semios-overlay'));
var out='bar='+(bar?'yes':'no')+';list='+(list?'yes':'no')+';chromePx='+chromePx+';inset='+inset+';blocked='+(blocked?'yes':'no')+';hit='+(hit?hit.tagName:'none');
location.href='semios://probe/'+encodeURIComponent(out);})()"#;
    eval(app, PROBE);
}

#[cfg(mobile)]
fn probe_remote_page<R: Runtime>(_app: &AppHandle<R>) {
    logging::write("WARN", "smoke probe skipped on mobile: no eval available");
}

fn navigate<R: Runtime>(app: &AppHandle<R>, target: &str) {
    let Ok(url) = Url::parse(target) else {
        logging::write("WARN", &format!("navigate rejected unparsable target {target}"));
        return;
    };

    logging::write("INFO", &format!("navigate to {url}"));

    if url.scheme() == "http" || url.scheme() == "https" || is_own_origin(app, &url) {
        if let Some(webview) = webview(app) {
            let _ = webview.navigate(url);
        }
        return;
    }

    let _ = app.opener().open_url(url.to_string(), None::<String>);
}

/// The start page is ours, not a site. On Windows and Android Tauri serves it
/// from an `http://<scheme>.localhost` origin, but on macOS and Linux it is a
/// `tauri://` one, which would otherwise be handed to the system browser the
/// moment a new tab asked to open the start page.
fn is_own_origin<R: Runtime>(app: &AppHandle<R>, url: &Url) -> bool {
    let state = app.state::<TabsState>();
    let tabs = state.0.lock().unwrap_or_else(|error| error.into_inner());
    let Some(start) = tabs.start_url.as_deref() else {
        return false;
    };
    let Ok(start) = Url::parse(start) else {
        return false;
    };
    url.scheme() == start.scheme() && url.host_str() == start.host_str()
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
        // The start page can invoke a command and a remote page can only
        // navigate the bridge, but both have to land in the same place, so the
        // shared actions are routed through the one dispatcher.
        "close" | "external" | "update" | "newtab" | "selecttab" | "closetab" => {
            dispatch(app, action, arg.as_deref());
        }
        "probe" => {
            logging::write("INFO", &format!("smoke probe {}", arg.as_deref().unwrap_or("-")));
            // The geometry above was measured by the page itself. The tab list
            // is not read in the same breath, because it is painted from a state
            // push that is still queued behind that measurement: asking here
            // would report zero tabs on a working build. One round trip later
            // the paint has happened, so this answer is not a race.
            #[cfg(desktop)]
            report_rendered_tabs(app);
        }
        "tabs" => {
            logging::write("INFO", &format!("smoke tabs {}", arg.as_deref().unwrap_or("-")));
        }
        _ => {}
    }
}

/// Second half of the probe: reads the tab list back out of the rendered DOM,
/// which by now reflects the snapshot the toolbar was given.
#[cfg(desktop)]
fn report_rendered_tabs<R: Runtime>(app: &AppHandle<R>) {
    const REPORT: &str = r#"(function(){
var host=document.getElementById('semios-overlay');
var list=host&&host.shadowRoot?host.shadowRoot.querySelector('.tablist'):null;
var n=list?list.querySelectorAll('.tab').length:0;
var picked=list?list.querySelector('.tab[aria-selected=true]'):null;
location.href='semios://tabs/'+n+'/'+(picked?encodeURIComponent(picked.textContent.trim()):'none');})()"#;
    eval(app, REPORT);
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
