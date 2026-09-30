#!/usr/bin/env bash
# Boots Semios on an emulator, then records whether it survived startup.
# Never exits before the diagnostics are collected: a crashed app is the
# interesting result, and the logcat around the crash is the whole point.
set -u

PKG="app.semios.browser"
OUT="smoke-logs"
mkdir -p "$OUT"

APK="$(find src-tauri/gen/android/app/build/outputs/apk -path '*release*' -name '*.apk' | head -1)"
echo "APK: $APK" | tee "$OUT/result.txt"
if [ -z "$APK" ]; then
  echo "no release APK was produced" | tee -a "$OUT/result.txt"
  exit 1
fi

echo "installing..."
adb install -r -t "$APK" > "$OUT/install.txt" 2>&1
INSTALL_RC=$?
cat "$OUT/install.txt"
echo "install exit: $INSTALL_RC" | tee -a "$OUT/result.txt"
if [ "$INSTALL_RC" -ne 0 ]; then
  echo "RESULT: APK did not install; startup was never tested" | tee -a "$OUT/result.txt"
  exit 1
fi

adb logcat -c
adb shell am force-stop "$PKG" || true

# The app reads this to walk a remote page load and a reload once the start page
# settles. A system property is the only channel that survives a release-signed
# APK: no env vars are inherited, run-as is denied, and scoped storage refuses
# even the app's own external directory. Verify it took, otherwise the
# navigation half of this test would quietly never run.
SMOKE_URL="https://example.com/"
PROP="semios.smoke_url"
adb shell setprop "$PROP" "$SMOKE_URL" >/dev/null 2>&1 || true
TRIGGER_OK="$(adb shell getprop "$PROP" 2>/dev/null | tr -d '\r\n ')"
if [ "$TRIGGER_OK" != "$SMOKE_URL" ]; then
  echo "RESULT: could not arm the navigation smoke trigger (got '$TRIGGER_OK')" | tee "$OUT/result.txt"
  exit 1
fi
# Never leave the trigger behind, including on the failure paths below.
trap 'adb shell setprop semios.smoke_url "" >/dev/null 2>&1 || true' EXIT
echo "navigation trigger armed: $TRIGGER_OK"

echo "launching..."
adb shell monkey -p "$PKG" -c android.intent.category.LAUNCHER 1 > "$OUT/launch.txt" 2>&1
cat "$OUT/launch.txt"

# The abort, when it happens, lands well after the window is up, so sample the
# app's own log while it is still alive rather than only after the fact. The
# window is long enough for the start page, a remote page, and a reload.
for _ in $(seq 1 12); do
  sleep 5
  PID_NOW="$(adb shell pidof "$PKG" 2>/dev/null | tr -d '\r')"
  if [ -z "$PID_NOW" ]; then
    break
  fi
  adb shell run-as "$PKG" cat cache/semios.log > "$OUT/app-log.txt" 2>/dev/null || true
done

sleep 5
PID="$(adb shell pidof "$PKG" 2>/dev/null | tr -d '\r')"
echo "pid: '$PID'" | tee -a "$OUT/result.txt"

adb shell dumpsys activity activities > "$OUT/activity.txt" 2>&1
adb shell dumpsys window > "$OUT/window.txt" 2>&1
adb logcat -d -v threadtime > "$OUT/logcat-full.txt" 2>&1

{
  echo "=== app log (sampled while alive) ==="
  cat "$OUT/app-log.txt" 2>/dev/null || echo "(unavailable: release-signed app, run-as denied)"
  echo
  echo "=== semios lines ==="
  grep -iE "semios" "$OUT/logcat-full.txt" | tail -80
  echo
  echo "=== fatal / crash / panic / anr ==="
  grep -iE "FATAL EXCEPTION|beginning of crash|panicked at|force finishing|ANR in|has died|libc.*Fatal signal|Abort message" "$OUT/logcat-full.txt" | tail -80
  echo
  echo "=== rust / tauri / webview ==="
  grep -iE "RustStdout|RustStderr|tauri|chromium:|WebViewFactory|Cr_Iface" "$OUT/logcat-full.txt" | tail -120
} | tee "$OUT/summary.txt"

adb exec-out screencap -p > "$OUT/screen.png" 2>/dev/null || true

if [ -z "$PID" ]; then
  echo "RESULT: app is not running (crashed during startup)" | tee -a "$OUT/result.txt"
  exit 1
fi

# The trigger was armed and read, so the log has to show the whole walk. These
# are the paths a bare startup never reaches, and the ones that used to abort.
# The toolbar cannot report in from a foreign origin and mobile has no eval, so
# unlike desktop this cannot ask the page what it sees; it proves the remote
# page survives a load and a reload, and that nothing was drawn blank.
if ! grep -q 'smoke complete: remote page loaded and reloaded' "$OUT/logcat-full.txt"; then
  echo "RESULT: remote page did not survive a load and a reload" | tee -a "$OUT/result.txt"
  exit 1
fi

# A live process is not a working browser: the app can survive startup while
# rendering nothing at all. Count distinct colours on a coarse grid so a blank
# window fails instead of passing on a PID alone. A blank start page measures
# around 15; a rendered one is in the hundreds.
if [ -s "$OUT/screen.png" ]; then
  COLORS="$(python3 .github/scripts/count-colors.py "$OUT/screen.png" 2>/dev/null || echo 0)"
  echo "distinct colours: $COLORS" | tee -a "$OUT/result.txt"
  if [ "$COLORS" -lt 40 ] 2>/dev/null; then
    echo "RESULT: app is alive but the window is blank ($COLORS colours)" | tee -a "$OUT/result.txt"
    exit 1
  fi
fi

echo "RESULT: app is running (pid $PID)" | tee -a "$OUT/result.txt"
