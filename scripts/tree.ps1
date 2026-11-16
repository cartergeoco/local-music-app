# Per-process memory for the running app's process tree.
param([string]$ProcessName = 'local-music-app')

$root = (Get-Process $ProcessName -ErrorAction Stop | Select-Object -First 1).Id
$all = Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, CommandLine
$ids = [System.Collections.Generic.List[int]]::new()
$ids.Add($root)
for ($i = 0; $i -lt $ids.Count; $i++) {
  foreach ($p in $all) { if ($p.ParentProcessId -eq $ids[$i]) { $ids.Add([int]$p.ProcessId) } }
}
$rows = foreach ($id in $ids) {
  $p = Get-Process -Id $id -ErrorAction SilentlyContinue
  if (-not $p) { continue }
  $cmd = ($all | Where-Object ProcessId -eq $id).CommandLine
  $type = if ($cmd -match '--type=([\w-]+)') { $Matches[1] } elseif ($id -eq $root) { 'app' } else { 'browser' }
  if ($cmd -match '--utility-sub-type=([\w.]+)') { $type += " ($($Matches[1]))" }
  [pscustomobject]@{
    Pid       = $id
    Type      = $type
    PrivateMB = [math]::Round($p.PrivateMemorySize64 / 1MB, 1)
    WorkingMB = [math]::Round($p.WorkingSet64 / 1MB, 1)
  }
}
$rows | Format-Table -AutoSize
"total private: {0:N1} MB" -f (($rows | Measure-Object PrivateMB -Sum).Sum)
