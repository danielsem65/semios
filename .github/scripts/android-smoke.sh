#!/usr/bin/env bash
# Boots Semios on an emulator, then records whether it survived startup.
# Never exits non-zero before the diagnostics are collected: a crashed app is
# the interesting result, and the logcat around the crash is the whole point.
set -u

PKG="app.semios.browser"
OUT="smoke-logs"
mkdir -p "$OUT"

APK="$(find src-tauri/gen/android/app/build/outputs/apk -name '*.apk' | head -1)"
echo "APK: $APK"
if [ -z "$APK" ]; then
  echo "no APK was produced" | tee "$OUT/result.txt"
  exit 1
fi

adb install -r -t "$APK" > "$OUT/install.txt" 2>&1
echo "install exit: $?" >> "$OUT/install.txt"

adb logcat -c
adb shell am force-stop "$PKG" || true

echo "launching..."
adb shell monkey -p "$PKG" -c android.intent.category.LAUNCHER 1 > "$OUT/launch.txt" 2>&1
sleep 30

PID="$(adb shell pidof "$PKG" 2>/dev/null | tr -d '\r')"
echo "pid: '$PID'" | tee "$OUT/result.txt"

adb shell dumpsys activity activities > "$OUT/activity.txt" 2>&1
adb shell dumpsys window > "$OUT/window.txt" 2>&1
adb logcat -d -v threadtime > "$OUT/logcat-full.txt" 2>&1

{
  echo "=== fatal / crash / panic ==="
  grep -iE "FATAL EXCEPTION|beginning of crash|panicked at|force finishing|has died|ANR in|RustStdout|semios:" "$OUT/logcat-full.txt" | tail -120
  echo
  echo "=== tauri / webview / rust ==="
  grep -iE "tauri|chromium|webview|RustStderr|JNI" "$OUT/logcat-full.txt" | tail -120
} | tee "$OUT/summary.txt"

adb exec-out screencap -p > "$OUT/screen.png" 2>/dev/null || true

if [ -z "$PID" ]; then
  echo "RESULT: app is not running after 30s (crashed at startup)" | tee -a "$OUT/result.txt"
  exit 1
fi

echo "RESULT: app is running (pid $PID)" | tee -a "$OUT/result.txt"
