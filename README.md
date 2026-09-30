# Semios

The lightest, fastest web browser.

Built with Tauri 2 and the native platform webview. No bundled Chromium, no
JavaScript framework, no runtime dependency downloads.

## Status

| Platform | State |
| --- | --- |
| Windows | Building on every push (NSIS installer) |
| Android | Building on every push (signed APK) |
| iOS | Planned |
| macOS | Planned |

## Install

Android (signed, universal APK, works on any device):

- https://github.com/danielsem65/semios/releases/latest/download/semios-android.apk

Windows (NSIS installer):

- https://github.com/danielsem65/semios/releases/latest/download/semios-windows-setup.exe

Android may ask you to allow installs from your browser the first time.

## Features

- Single address bar with automatic search fallback (DuckDuckGo).
- Back, forward, reload/stop, and home.
- Scroll-aware toolbar that hides while reading and returns on scroll up.
- Keyboard shortcuts: `Ctrl+L` focus, `Ctrl+R` reload, `Alt+Left` / `Alt+Right`
  history, `Esc` stop.
- Native window controls on desktop, native Back handling on Android.
- Dark mode via the system preference.

## Architecture

Semios uses one webview for the whole app. The same toolbar code runs in two
places:

- **Start page** — a small app-origin page that talks to Rust over Tauri IPC.
- **Remote pages** — the toolbar is injected into every page by a
  `initialization_script` bundle, mounted into a Shadow DOM so page CSS cannot
  reach it.

Remote pages never get Tauri IPC. They reach Rust only through the
`semios://` URL scheme, which is intercepted in the Rust `on_navigation` hook
and cancelled. Rust pushes browser state back into the page with
`webview.eval("window.__semios.update(...)")`.

```
src/                  frontend
  controller.ts       Snapshot + Controller contracts
  ipc-controller.ts   start page controller (Tauri IPC)
  overlay-controller.ts  remote page controller (URL scheme + eval)
  toolbar.ts          shared UI, Shadow DOM
  overlay.ts          injection entry
  main.ts             start page entry
src-tauri/
  src/lib.rs          window creation, history, navigation, bridge
  overlay/inject.js   generated injection bundle (not committed)
```

## Development

Requires Rust, Node.js, and the Android SDK for mobile.

```sh
npm install
npm run tauri icon app-icon.png   # generates src-tauri/icons
npm run tauri dev
```

Mobile:

```sh
npm run tauri android init
npm run tauri android dev
```

## Builds

Push to `main` or open a pull request:

- [Windows](.github/workflows/windows.yml) — NSIS installer
- [Android](.github/workflows/android.yml) — signed APK

Push a `v*` tag to publish a GitHub Release:

```sh
git tag v0.2.0
git push origin v0.2.0
```

Android signing uses three repository secrets: `ANDROID_KEY_ALIAS`,
`ANDROID_KEY_PASSWORD`, and `ANDROID_KEY_BASE64` (the base64 of the release
keystore). The keystore itself is never stored in this repository. Because
`src-tauri/gen/android` is generated, each workflow writes
`keystore.properties` and patches the generated
`app/build.gradle.kts` via [scripts/patch-android-signing.mjs](scripts/patch-android-signing.mjs).
`apksigner verify` runs on every Android build, so an unsigned APK can never be
published by accident.

## License

[MIT](LICENSE)
