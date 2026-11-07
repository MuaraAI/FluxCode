# Dev helper: screenshot the FluxCode window (used to check the UI without a browser).
# Usage: powershell -File scripts/shot.ps1 -Out shot.png [-Keys "{F5}"] [-Wait 1500]
param([string]$Out = "shot.png", [string]$Keys = "", [int]$Wait = 800, [string]$Click = "")

Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class W {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int x, int y, uint d, UIntPtr e);
  public struct RECT { public int L, T, R, B; }
}
"@

[W]::SetProcessDPIAware() | Out-Null  # real pixel sizes on scaled displays
$p = Get-Process fluxcode -ErrorAction Stop | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
$h = $p.MainWindowHandle
[W]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 300
if ($Click) {
  # "x,y" in window pixels (same coordinates as the saved screenshot).
  if ([W]::GetForegroundWindow() -ne $h) { throw "FluxCode is not the foreground window; not clicking." }
  $r0 = New-Object W+RECT; [W]::GetWindowRect($h, [ref]$r0) | Out-Null
  $x, $y = $Click.Split(",") | ForEach-Object { [int]$_ }
  [W]::SetCursorPos($r0.L + $x, $r0.T + $y) | Out-Null
  [W]::mouse_event(0x2, 0, 0, 0, [UIntPtr]::Zero); [W]::mouse_event(0x4, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 200
}
if ($Keys) {
  # Only type when FluxCode really is the foreground window.
  if ([W]::GetForegroundWindow() -ne $h) { throw "FluxCode is not the foreground window; not sending keys." }
  [System.Windows.Forms.SendKeys]::SendWait($Keys)
}
Start-Sleep -Milliseconds $Wait
$r = New-Object W+RECT
[W]::GetWindowRect($h, [ref]$r) | Out-Null
$bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$dc = $g.GetHdc()
[W]::PrintWindow($h, $dc, 2) | Out-Null   # PW_RENDERFULLCONTENT: works for GPU/WebView content
$g.ReleaseHdc($dc)
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
"saved $Out"
