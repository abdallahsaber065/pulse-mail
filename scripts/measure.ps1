# Pulse Mail memory sampler (docs/PLAN.md, performance table).
#
# Sums the working set and private memory of pulse-mail.exe plus every
# msedgewebview2.exe descendant (the WebView2 processes it hosts).
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\measure.ps1                # one snapshot
#   powershell -ExecutionPolicy Bypass -File scripts\measure.ps1 -Seconds 600   # sample for 10 min
#   powershell -ExecutionPolicy Bypass -File scripts\measure.ps1 -Seconds 60 -IntervalSec 10
#
# Match the numbers against the PLAN.md targets:
#   idle, tray only (window closed)  -> app private bytes <= 25 MB
#   idle, window hidden              -> app + webview2 total <= 120 MB
#   window open, reading mail        -> total <= 200 MB
param(
    [int]$Seconds = 0,
    [int]$IntervalSec = 5
)

$ErrorActionPreference = 'Stop'

$all = Get-CimInstance Win32_Process
$root = $all | Where-Object { $_.Name -eq 'pulse-mail.exe' } | Select-Object -First 1
if (-not $root) {
    Write-Error 'pulse-mail.exe is not running. Start the app first (installed build, or pnpm tauri dev).'
}

# pid -> children map, then collect all descendants of the app (WebView2 trees).
$children = @{}
foreach ($p in $all) {
    if (-not $children.ContainsKey([uint32]$p.ParentProcessId)) {
        $children[[uint32]$p.ParentProcessId] = New-Object System.Collections.Generic.List[uint32]
    }
    $children[[uint32]$p.ParentProcessId].Add([uint32]$p.ProcessId)
}
$descendants = New-Object System.Collections.Generic.List[uint32]
$stack = New-Object System.Collections.Generic.Stack[uint32]
foreach ($c in $children[[uint32]$root.ProcessId]) { $stack.Push($c) }
while ($stack.Count -gt 0) {
    $cur = $stack.Pop()
    $descendants.Add($cur)
    if ($children.ContainsKey($cur)) {
        foreach ($c in $children[$cur]) { $stack.Push($c) }
    }
}

function Mb([long]$bytes) { return [math]::Round($bytes / 1MB, 1) }

$deadline = (Get-Date).AddSeconds($Seconds)
do {
    $ids = @([uint32]$root.ProcessId) + @($descendants)
    $procs = Get-Process -Id $ids -ErrorAction SilentlyContinue
    $app = @($procs | Where-Object { $_.Name -eq 'pulse-mail' })
    $web = @($procs | Where-Object { $_.Name -eq 'msedgewebview2' })

    $appWs = ($app | Measure-Object WorkingSet64 -Sum).Sum
    $appPm = ($app | Measure-Object PrivateMemorySize64 -Sum).Sum
    $webWs = ($web | Measure-Object WorkingSet64 -Sum).Sum
    $webPm = ($web | Measure-Object PrivateMemorySize64 -Sum).Sum
    if (-not $appWs) { $appWs = 0 }
    if (-not $appPm) { $appPm = 0 }
    if (-not $webWs) { $webWs = 0 }
    if (-not $webPm) { $webPm = 0 }

    $stamp = Get-Date -Format 'HH:mm:ss'
    Write-Host ("{0}  app WS {1} MB / private {2} MB   webview2 x{3} WS {4} MB / private {5} MB   TOTAL WS {6} MB / private {7} MB" -f `
        $stamp, (Mb $appWs), (Mb $appPm), $web.Count, (Mb $webWs), (Mb $webPm), (Mb ($appWs + $webWs)), (Mb ($appPm + $webPm)))

    if ((Get-Date) -lt $deadline) { Start-Sleep -Seconds $IntervalSec }
} while ((Get-Date) -lt $deadline)
