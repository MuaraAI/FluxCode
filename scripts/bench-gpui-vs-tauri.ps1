# Benchmark: FluxCode Tauri (WebView2) vs FluxCode GPUI on the same machine.
#
#   powershell -File scripts/bench-gpui-vs-tauri.ps1 [-Runs 5] [-Out bench.json]
#
# Measures, for each app:
#   * start -> first painted UI (window visible AND its pixels show real UI, not
#     just the background colour), median of -Runs warm starts
#   * idle memory 10 s after start (sum over the process tree; for Tauri that
#     includes every msedgewebview2.exe child): private bytes and working set
#   * the same with a large file (60k lines of JS) open
#   * CPU time while typing (SendKeys) and while scrolling (mouse wheel)
#   * executable size
#
# The Tauri runs use a throw-away WebView2 profile (WEBVIEW2_USER_DATA_FOLDER)
# so your real FluxCode session and history are not touched.
param(
  [int]$Runs = 5,
  [string]$Tauri = "$env:USERPROFILE\.fluxcode-target\release\fluxcode.exe",
  [string]$Gpui = "$env:USERPROFILE\.fluxcode-target-gpui\release\fluxcode-gpui.exe",
  [string]$Folder = (Resolve-Path "$PSScriptRoot\..").Path,
  [string]$Out = ""
)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
public static class B {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, int d, UIntPtr e);
  public struct RECT { public int L, T, R, B; }
  public static IntPtr VisibleWindow(uint pid) {
    IntPtr found = IntPtr.Zero;
    EnumWindows((h, l) => { uint p; GetWindowThreadProcessId(h, out p);
      RECT r; if (p == pid && IsWindowVisible(h) && GetWindowRect(h, out r) && (r.R - r.L) > 300 && (r.B - r.T) > 200) { found = h; return false; }
      return true; }, IntPtr.Zero);
    return found;
  }
}
"@
[B]::SetProcessDPIAware() | Out-Null

$profileDir = Join-Path $env:TEMP "fluxcode-bench-webview2"
$env:WEBVIEW2_USER_DATA_FOLDER = $profileDir

function Get-Tree([int]$rootPid) {
  $all = Get-CimInstance Win32_Process
  $ids = New-Object System.Collections.Generic.List[int]; $ids.Add($rootPid)
  $i = 0
  while ($i -lt $ids.Count) { foreach ($p in $all) { if ($p.ParentProcessId -eq $ids[$i] -and -not $ids.Contains([int]$p.ProcessId)) { $ids.Add([int]$p.ProcessId) } }; $i++ }
  # WebView2 children are re-parented to the WebView2 browser process; catch them by name.
  foreach ($p in $all) { if ($p.Name -eq "msedgewebview2.exe" -and $p.CommandLine -match [regex]::Escape("webview-exe-name=$((Get-Process -Id $rootPid).ProcessName).exe") -and -not $ids.Contains([int]$p.ProcessId)) { $ids.Add([int]$p.ProcessId) } }
  return $ids
}

function Get-Mem([int]$rootPid) {
  $priv = 0; $ws = 0; $n = 0
  foreach ($id in (Get-Tree $rootPid)) { $g = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($g) { $priv += $g.PrivateMemorySize64; $ws += $g.WorkingSet64; $n++ } }
  [pscustomobject]@{ Procs = $n; PrivateMB = [math]::Round($priv / 1MB, 1); WorkingSetMB = [math]::Round($ws / 1MB, 1) }
}

function Get-Cpu([int]$rootPid) {
  $t = 0.0; foreach ($id in (Get-Tree $rootPid)) { $g = Get-Process -Id $id -ErrorAction SilentlyContinue; if ($g) { $t += $g.TotalProcessorTime.TotalSeconds } }; $t
}

function Stop-App($exe) {
  $name = [IO.Path]::GetFileNameWithoutExtension($exe)
  Get-Process $name -ErrorAction SilentlyContinue | Stop-Process -Force
  for ($i = 0; $i -lt 50; $i++) {
    $left = @(Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" | Where-Object { $_.CommandLine -match "webview-exe-name=$name.exe" })
    if ($left.Count -eq 0 -and -not (Get-Process $name -ErrorAction SilentlyContinue)) { break }
    $left | ForEach-Object { Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue }
    Start-Sleep -Milliseconds 200
  }
  Start-Sleep -Milliseconds 500
}

# Distinct colours on a coarse grid of the window: a blank window has 1-3.
function Test-Painted([IntPtr]$h) {
  $r = New-Object B+RECT; [B]::GetWindowRect($h, [ref]$r) | Out-Null
  $w = $r.R - $r.L; $hh = $r.B - $r.T
  if ($w -le 0 -or $hh -le 0) { return $false }
  $bmp = New-Object System.Drawing.Bitmap $w, $hh
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
  $set = @{}
  for ($y = 8; $y -lt $hh; $y += [math]::Max(4, [int]($hh / 60))) { for ($x = 8; $x -lt $w; $x += [math]::Max(4, [int]($w / 80))) { $set[$bmp.GetPixel($x, $y).ToArgb()] = 1 } }
  $g.Dispose(); $bmp.Dispose()
  return $set.Count -ge 12
}

function Start-App($exe, [string[]]$appArgs) {
  $sw = [Diagnostics.Stopwatch]::StartNew()
  $p = Start-Process $exe -ArgumentList ($appArgs | ForEach-Object { "`"$_`"" }) -PassThru
  $h = [IntPtr]::Zero; $shown = $null; $painted = $null
  while ($sw.ElapsedMilliseconds -lt 20000) {
    if ($h -eq [IntPtr]::Zero) { $h = [B]::VisibleWindow([uint32]$p.Id); if ($h -ne [IntPtr]::Zero) { $shown = $sw.ElapsedMilliseconds; [B]::SetForegroundWindow($h) | Out-Null } }
    elseif (Test-Painted $h) { $painted = $sw.ElapsedMilliseconds; break }
    Start-Sleep -Milliseconds 15
  }
  [pscustomobject]@{ Process = $p; Window = $h; ShownMs = $shown; PaintedMs = $painted }
}

function Median($xs) { $s = @($xs | Where-Object { $_ -ne $null } | Sort-Object); if ($s.Count -eq 0) { return $null }; $s[[int][math]::Floor(($s.Count - 1) / 2)] }

# A large JS file (60k lines, ~3 MB) shared by both apps.
$big = Join-Path $env:TEMP "fluxcode-bench-big.js"
if (-not (Test-Path $big)) {
  $sb = New-Object System.Text.StringBuilder
  for ($i = 0; $i -lt 15000; $i++) {
    [void]$sb.AppendLine("// block $i")
    [void]$sb.AppendLine("export function handler$i(req, res) { const value = req.query?.id ?? $i; return res.json({ ok: true, value, name: `"item-$i`" }); }")
    [void]$sb.AppendLine("const list$i = [1, 2, 3, 4, 5].map((n) => n * $i).filter((n) => n % 2 === 0);")
    [void]$sb.AppendLine("")
  }
  [IO.File]::WriteAllText($big, $sb.ToString())
}

$results = [ordered]@{}
foreach ($app in @(@{ Name = "Tauri"; Exe = $Tauri }, @{ Name = "GPUI"; Exe = $Gpui })) {
  $exe = $app.Exe
  Write-Host "== $($app.Name): $exe"
  Stop-App $exe
  # Warm-up run (first WebView2 profile creation / font cache), not counted.
  $s = Start-App $exe @($Folder); Start-Sleep 2; Stop-App $exe

  $shown = @(); $painted = @(); $idle = @()
  for ($n = 0; $n -lt $Runs; $n++) {
    $s = Start-App $exe @($Folder)
    $shown += $s.ShownMs; $painted += $s.PaintedMs
    Start-Sleep 10
    $idle += Get-Mem $s.Process.Id
    Stop-App $exe
  }

  # Large file open, then typing and scrolling.
  $s = Start-App $exe @($Folder, $big)
  Start-Sleep 10
  $bigMem = Get-Mem $s.Process.Id
  $h = $s.Window
  $r = New-Object B+RECT; [B]::GetWindowRect($h, [ref]$r) | Out-Null
  $cx = [int](($r.L + $r.R) / 2) + 150; $cy = [int](($r.T + $r.B) / 2)
  [B]::SetForegroundWindow($h) | Out-Null; [B]::SetCursorPos($cx, $cy); Start-Sleep -Milliseconds 300
  [B]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero); [B]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero)  # click into the editor
  Start-Sleep -Milliseconds 500

  $c0 = Get-Cpu $s.Process.Id; $t0 = [Diagnostics.Stopwatch]::StartNew()
  for ($k = 0; $k -lt 100; $k++) { [System.Windows.Forms.SendKeys]::SendWait("ab"); Start-Sleep -Milliseconds 40 }
  $typeCpu = (Get-Cpu $s.Process.Id) - $c0; $typeSec = $t0.Elapsed.TotalSeconds

  $c0 = Get-Cpu $s.Process.Id; $t0 = [Diagnostics.Stopwatch]::StartNew()
  for ($k = 0; $k -lt 100; $k++) { [B]::mouse_event(0x0800, 0, 0, -120, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40 }
  for ($k = 0; $k -lt 100; $k++) { [B]::mouse_event(0x0800, 0, 0, 120, [UIntPtr]::Zero); Start-Sleep -Milliseconds 40 }
  $scrollCpu = (Get-Cpu $s.Process.Id) - $c0; $scrollSec = $t0.Elapsed.TotalSeconds

  Start-Sleep 3
  $c0 = Get-Cpu $s.Process.Id; Start-Sleep 5; $idleCpu = (Get-Cpu $s.Process.Id) - $c0
  Stop-App $exe

  $results[$app.Name] = [ordered]@{
    ExeMB          = [math]::Round((Get-Item $exe).Length / 1MB, 2)
    ShownMs        = Median $shown
    PaintedMs      = Median $painted
    IdleProcs      = ($idle | Select-Object -Last 1).Procs
    IdlePrivateMB  = Median ($idle | ForEach-Object { $_.PrivateMB })
    IdleWorkingSetMB = Median ($idle | ForEach-Object { $_.WorkingSetMB })
    BigPrivateMB   = $bigMem.PrivateMB
    BigWorkingSetMB = $bigMem.WorkingSetMB
    TypingCpuPct   = [math]::Round(100 * $typeCpu / $typeSec, 1)
    ScrollCpuPct   = [math]::Round(100 * $scrollCpu / $scrollSec, 1)
    IdleCpuPct     = [math]::Round(100 * $idleCpu / 5, 2)
    RawPaintedMs   = ($painted -join ",")
  }
  $results[$app.Name] | Format-List | Out-String | Write-Host
}
Remove-Item Env:WEBVIEW2_USER_DATA_FOLDER -ErrorAction SilentlyContinue
if ($Out) { $results | ConvertTo-Json | Set-Content -Encoding utf8 $Out }
$results | ConvertTo-Json
