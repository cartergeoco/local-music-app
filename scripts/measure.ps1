# Launches the release build and samples memory/CPU of its whole process tree.
param(
  [int]$Seconds = 180,
  [int]$Interval = 15,
  [int]$SoakSeconds = 0,
  [string]$Exe = "$PSScriptRoot\..\src-tauri\target\release\local-music-app.exe"
)

function Get-Tree([int]$root) {
  $all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId
  $ids = [System.Collections.Generic.List[int]]::new()
  $ids.Add($root)
  for ($i = 0; $i -lt $ids.Count; $i++) {
    foreach ($p in $all) { if ($p.ParentProcessId -eq $ids[$i]) { $ids.Add([int]$p.ProcessId) } }
  }
  $ids
}

$env:MUSIC_SOAK = "$SoakSeconds"
$app = Start-Process -FilePath $Exe -PassThru
$start = Get-Date
$cores = [Environment]::ProcessorCount
$prevCpu = $null
"{0,6} {1,11} {2,12} {3,8} {4,6}" -f 'sec', 'private MB', 'working MB', 'cpu %', 'procs'
while (((Get-Date) - $start).TotalSeconds -lt $Seconds -and -not $app.HasExited) {
  Start-Sleep -Seconds $Interval
  $procs = Get-Tree $app.Id | ForEach-Object { Get-Process -Id $_ -ErrorAction SilentlyContinue }
  $private = ($procs | Measure-Object PrivateMemorySize64 -Sum).Sum / 1MB
  $working = ($procs | Measure-Object WorkingSet64 -Sum).Sum / 1MB
  $cpu = 0.0
  foreach ($p in $procs) { if ($p.TotalProcessorTime) { $cpu += $p.TotalProcessorTime.TotalSeconds } }
  $cpuPct = if ($null -ne $prevCpu) { ($cpu - $prevCpu) / $Interval / $cores * 100 } else { 0 }
  $prevCpu = $cpu
  "{0,6:N0} {1,11:N1} {2,12:N1} {3,8:N2} {4,6}" -f ((Get-Date) - $start).TotalSeconds, $private, $working, $cpuPct, $procs.Count
}
if ($app.HasExited) { "app exited early" } else { "app pid $($app.Id) left running" }
