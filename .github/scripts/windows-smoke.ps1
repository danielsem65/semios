param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [int]$SettleSeconds = 40,
  [string]$SmokeUrl = 'https://example.com/'
)

# Windows had a build check but no runtime check, which is why a blank window
# could ship unnoticed. This launches the real binary, proves it survives
# startup, and captures the app log plus a screenshot.

$ErrorActionPreference = 'Stop'

$work = Join-Path $env:RUNNER_TEMP 'windows-smoke'
New-Item -ItemType Directory -Force -Path $work | Out-Null
$logPath = Join-Path $env:LOCALAPPDATA 'Semios\semios.log'
if (Test-Path -LiteralPath $logPath) {
  Remove-Item -LiteralPath $logPath -Force
}

Write-Host "launching $Exe"
# The app reads this to walk a remote page load and a reload after the start
# page settles. Both go through code that a bare startup never reaches.
$env:SEMIOS_SMOKE_URL = $SmokeUrl
$process = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds $SettleSeconds
$process.Refresh()

if ($process.HasExited) {
  $crashed = $true
  "RESULT crashed exit=$($process.ExitCode) after ${SettleSeconds}s" |
    Set-Content (Join-Path $work 'result.txt')
} else {
  $crashed = $false
  "RESULT alive after ${SettleSeconds}s title=$($process.MainWindowTitle) handle=$($process.MainWindowHandle)" |
    Set-Content (Join-Path $work 'result.txt')
}

# Screenshots need an interactive desktop; never fail the run over one.
try {
  Add-Type -AssemblyName System.Drawing
  Add-Type @'
using System;
using System.Runtime.InteropServices;
public class SemiosSmokeNative {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int n);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
}
'@
  if (-not $process.HasExited -and $process.MainWindowHandle -ne 0) {
    [void][SemiosSmokeNative]::ShowWindow($process.MainWindowHandle, 9)
    [void][SemiosSmokeNative]::SetForegroundWindow($process.MainWindowHandle)
    Start-Sleep -Seconds 3
    $rect = New-Object SemiosSmokeNative+RECT
    [void][SemiosSmokeNative]::GetWindowRect($process.MainWindowHandle, [ref]$rect)
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    if ($width -gt 0 -and $height -gt 0) {
      $bitmap = New-Object System.Drawing.Bitmap($width, $height)
      $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
      $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, (New-Object System.Drawing.Size($width, $height)))
      $graphics.Dispose()
      $bitmap.Save((Join-Path $work 'window.png'), [System.Drawing.Imaging.ImageFormat]::Png)
      $bitmap.Dispose()
      Write-Host "captured ${width}x${height} screenshot"
    }
  }
} catch {
  "screenshot skipped: $($_.Exception.Message)" | Add-Content (Join-Path $work 'result.txt')
}

if (Test-Path -LiteralPath $logPath) {
  Copy-Item -LiteralPath $logPath -Destination (Join-Path $work 'semios.log') -Force
  Get-Content -LiteralPath $logPath | Set-Content (Join-Path $work 'semios.log.txt')
} else {
  "no log at $logPath" | Set-Content (Join-Path $work 'semios.log.txt')
}

if (-not $process.HasExited) {
  Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
}

Get-Content (Join-Path $work 'result.txt')

$logText = if (Test-Path (Join-Path $work 'semios.log.txt')) {
  Get-Content -Raw (Join-Path $work 'semios.log.txt')
} else {
  ''
}

Write-Host '--- app log ---'
Write-Host $logText

$problems = @()
if ($crashed) { $problems += "process exited during startup (exit=$($process.ExitCode))" }
if ($logText -match 'ERROR|PANIC') { $problems += 'log reported an error' }
if ($logText -notmatch 'start toolbar mounted') {
  $problems += 'start page never reported a successful mount'
}
if ($logText -notmatch 'overlay attached') {
  $problems += 'toolbar never attached to the remote page'
}
if ($logText -notmatch 'smoke complete: remote page loaded and reloaded') {
  $problems += 'remote page did not survive a load and a reload'
}
if ($problems.Count -gt 0) {
  Write-Host "FAILED: $($problems -join '; ')"
  exit 1
}

Write-Host 'PASSED: window alive, toolbar mounted, remote page loaded and reloaded'
