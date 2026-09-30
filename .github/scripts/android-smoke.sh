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

echo "launching..."
adb shell monkey -p "$PKG" -c android.intent.category.LAUNCHER 1 > "$OUT/launch.txt" 2>&1
cat "$OUT/launch.txt"

# The abort, when it happens, lands well after the window is up, so sample the
# app's own log while it is still alive rather than only after the fact.
for _ in $(seq 1 6); do
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

# A live process is not a working browser: the app survived startup while
# rendering nothing at all. Count distinct colours on a coarse grid so a blank
# window fails instead of passing on a PID alone. A blank start page measures
# around 15; a rendered one is in the hundreds.
if [ -s "$OUT/screen.png" ]; then
  COLORS="$(python3 "$OUT/count-colors.py" "$OUT/screen.png" 2>/dev/null || echo 0)"
  echo "distinct colours: $COLORS" | tee -a "$OUT/result.txt"
  if [ "$COLORS" -lt 40 ] 2>/dev/null; then
    echo "RESULT: app is alive but the window is blank ($COLORS colours)" | tee -a "$OUT/result.txt"
    exit 1
  fi
fi

if [ -z "$PID" ]; then
  echo "RESULT: app is not running after 30s (crashed at startup)" | tee -a "$OUT/result.txt"
  exit 1
fi

echo "RESULT: app is running (pid $PID)" | tee -a "$OUT/result.txt"
