# Captures a window by process name to a PNG without bringing it to the front.
param(
  [string]$ProcessName = 'local-music-app',
  [string]$Out = "$env:TEMP\local-music-app.png"
)

Add-Type -AssemblyName System.Drawing
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class Win32Capture {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@

[Win32Capture]::SetProcessDPIAware() | Out-Null
$proc = Get-Process -Name $ProcessName -ErrorAction Stop | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $proc) { throw "No visible window for $ProcessName" }
$rect = New-Object Win32Capture+RECT
[Win32Capture]::GetWindowRect($proc.MainWindowHandle, [ref]$rect) | Out-Null
$w = $rect.Right - $rect.Left
$h = $rect.Bottom - $rect.Top
$bmp = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($bmp)
$hdc = $g.GetHdc()
# PW_RENDERFULLCONTENT captures GPU-composited content such as WebView2.
[Win32Capture]::PrintWindow($proc.MainWindowHandle, $hdc, 2) | Out-Null
$g.ReleaseHdc($hdc)
$g.Dispose()
$bmp.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
"$Out ${w}x${h}"
