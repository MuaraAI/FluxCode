<#
FluxCode benchmark: FluxCode, Zed and VS Code on the same Windows PC, same method.

Each app starts with a fresh, empty, throwaway profile (telemetry and update checks
off, no extensions, no folder open), and per run this records:

  window_ms   launch to the first visible top-level window
  memory_mb   Task Manager's "Memory" (private working set), summed over all of the
              app's processes, 9 s after launch
  commit_mb   memory reserved (private bytes), summed, 9 s after launch
  cpu_pct     CPU used while idle from 9 s to 39 s after launch, in % of one core
  processes   how many processes the app runs

plus each app's size on disk. Results go to results.json (next to this script) with
the PC's CPU, memory and Windows build, and each app's version. chart.py draws the
README charts from that file.

  powershell -ExecutionPolicy Bypass -File tools\bench\bench.ps1 -FluxExe target\release\fluxcode.exe
#>
param(
    [Parameter(Mandatory)] [string]$FluxExe,
    [int]$Runs = 5,
    [string]$Out = (Join-Path $PSScriptRoot "results.json")
)
$ErrorActionPreference = "Stop"
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Collections.Generic;
public class Bench {
  public delegate bool P(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] static extern bool EnumWindows(P f, IntPtr l);
  [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] static extern int GetWindowTextLength(IntPtr h);
  public static bool AnyVisible(HashSet<uint> pids) {
    bool found = false;
    EnumWindows((h, l) => { uint pid; GetWindowThreadProcessId(h, out pid);
      if (pids.Contains(pid) && IsWindowVisible(h) && GetWindowTextLength(h) > 0) { found = true; return false; } return true; }, IntPtr.Zero);
    return found;
  }
}
"@
$tmp = Join-Path ([IO.Path]::GetTempPath()) "fluxcode-bench"
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $tmp | Out-Null

$fluxExe = (Resolve-Path $FluxExe).Path
$code = "$env:LOCALAPPDATA\Programs\Microsoft VS Code\Code.exe"
$zed = "$env:LOCALAPPDATA\Programs\Zed\zed.exe"
$fluxProc = [IO.Path]::GetFileNameWithoutExtension($fluxExe)

$apps = @(
    @{ name = "FluxCode"; exe = $fluxExe; proc = $fluxProc; dir = $null; prep = {
        param($d)
        $p = New-Object Diagnostics.ProcessStartInfo $fluxExe
        $p.UseShellExecute = $false; $p.EnvironmentVariables["APPDATA"] = $d; $p.EnvironmentVariables["USERPROFILE"] = $d
        $p } },
    @{ name = "Zed"; exe = $zed; proc = "zed"; dir = (Split-Path $zed); prep = {
        param($d)
        New-Item -ItemType Directory -Force "$d\Zed" | Out-Null
        Set-Content -Encoding utf8 "$d\Zed\settings.json" '{"telemetry":{"diagnostics":false,"metrics":false},"auto_update":false}'
        $p = New-Object Diagnostics.ProcessStartInfo $zed
        $p.UseShellExecute = $false; $p.EnvironmentVariables["APPDATA"] = $d
        $p.Arguments = "--user-data-dir `"$d\data`""
        $p } },
    @{ name = "VS Code"; exe = $code; proc = "Code"; dir = (Split-Path $code); prep = {
        param($d)
        New-Item -ItemType Directory -Force "$d\User" | Out-Null
        Set-Content -Encoding utf8 "$d\User\settings.json" '{"telemetry.telemetryLevel":"off","update.mode":"none","workbench.startupEditor":"none","extensions.autoCheckUpdates":false,"extensions.autoUpdate":false,"workbench.enableExperiments":false}'
        $p = New-Object Diagnostics.ProcessStartInfo $code
        $p.UseShellExecute = $false; $p.RedirectStandardOutput = $true; $p.RedirectStandardError = $true
        $p.Arguments = "--user-data-dir `"$d`" --extensions-dir `"$d\ext`" --disable-extensions --skip-welcome --skip-release-notes --disable-workspace-trust --new-window"
        $p } }
)

function Sum-PrivateWs($proc) {
    $c = Get-Counter "\Process($proc*)\Working Set - Private" -ErrorAction SilentlyContinue
    ($c.CounterSamples | Where-Object { $_.InstanceName -like "$($proc.ToLower())*" } | Measure-Object CookedValue -Sum).Sum / 1MB
}
function Procs($proc) { @(Get-Process -Name $proc -ErrorAction SilentlyContinue) }

$cores = [Environment]::ProcessorCount
$results = @()
foreach ($a in $apps) {
    if (-not (Test-Path $a.exe)) { Write-Warning "$($a.name) not found at $($a.exe); skipped"; continue }
    if ((Procs $a.proc).Count -gt 0) { throw "$($a.name) is running; close it first so the numbers are its own." }
    $samples = @()
    foreach ($i in 1..$Runs) {
        $d = Join-Path $tmp "$($a.name -replace ' ', '')-$i"
        New-Item -ItemType Directory -Force $d | Out-Null
        $psi = & $a.prep $d
        $sw = [Diagnostics.Stopwatch]::StartNew()
        $root = [Diagnostics.Process]::Start($psi)
        $ms = $null
        while ($sw.ElapsedMilliseconds -lt 20000) {
            $pids = New-Object 'System.Collections.Generic.HashSet[uint32]'
            $null = $pids.Add([uint32]$root.Id)
            Procs $a.proc | ForEach-Object { $null = $pids.Add([uint32]$_.Id) }
            if ([Bench]::AnyVisible($pids)) { $ms = $sw.Elapsed.TotalMilliseconds; break }
            Start-Sleep -Milliseconds 2
        }
        Start-Sleep -Milliseconds ([math]::Max(0, 9000 - $sw.ElapsedMilliseconds))
        $mem = Sum-PrivateWs $a.proc
        $ps = Procs $a.proc
        $commit = ($ps | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB
        $cpu0 = ($ps | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
        Start-Sleep -Seconds 30
        $ps2 = Procs $a.proc
        $cpu1 = ($ps2 | ForEach-Object { $_.TotalProcessorTime.TotalMilliseconds } | Measure-Object -Sum).Sum
        $samples += [ordered]@{
            window_ms = if ($ms) { [math]::Round($ms) } else { $null }
            memory_mb = [math]::Round($mem, 1)
            commit_mb = [math]::Round($commit, 1)
            cpu_pct = [math]::Round(($cpu1 - $cpu0) / 30000 * 100, 2)
            processes = $ps2.Count
        }
        $ps2 | Stop-Process -Force -ErrorAction SilentlyContinue
        Start-Sleep -Seconds 3
        Write-Host ("{0} run {1}: {2}" -f $a.name, $i, ($samples[-1] | ConvertTo-Json -Compress))
    }
    $size = if ($a.dir) { (Get-ChildItem $a.dir -Recurse -File -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum } else { (Get-Item $a.exe).Length }
    $ver = (Get-Item $a.exe).VersionInfo
    $results += [ordered]@{
        name = $a.name
        version = if ($a.name -eq "FluxCode") { ([string]$ver.FileVersion).Trim() } else { ([string]$ver.ProductVersion).Trim() }
        disk_mb = [math]::Round($size / 1MB, 1)
        runs = $samples
    }
}
$cpuName = (Get-CimInstance Win32_Processor | Select-Object -First 1).Name.Trim()
$ram = [math]::Round((Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory / 1GB)
$os = (Get-CimInstance Win32_OperatingSystem)
$doc = [ordered]@{
    date = (Get-Date -Format "yyyy-MM-dd")
    machine = [ordered]@{ cpu = $cpuName; cores = $cores; ram_gb = $ram; os = "$($os.Caption) $($os.BuildNumber)" }
    method = "Fresh empty profile per run, telemetry and update checks off, no extensions, no folder open. window_ms: launch to first visible window. memory_mb / commit_mb: 9 s after launch, summed over all processes. cpu_pct: idle CPU from 9 s to 39 s, % of one core. disk_mb: install folder (FluxCode: its one program file)."
    apps = $results
}
$doc | ConvertTo-Json -Depth 6 | Set-Content -Encoding utf8 $Out
Write-Host "Wrote $Out"
Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
