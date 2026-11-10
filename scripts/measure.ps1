param(
  [Parameter(Mandatory = $true)][string]$Exe,
  [Parameter(Mandatory = $true)][string]$Out
)

# Launch RustChrome, let it settle, then sample memory and idle CPU over a
# 5 second window. Writes the numbers it measured, nothing else.

$startupLog = [System.IO.Path]::GetTempFileName()
$env:RUSTCHROME_STARTUP_LOG = $startupLog
$env:RUSTCHROME_COMPAT = $null

$p = Start-Process -FilePath $Exe -PassThru
Start-Sleep -Seconds 5

try {
    $proc = Get-Process -Id $p.Id -ErrorAction Stop
} catch {
    "target process found: no" | Set-Content -Path $Out
    exit 1
}

# Sum over the whole tree: WebView2 runs the renderer, GPU and utility
# processes as children, so the browser process alone is not the footprint.
function Get-Tree([int]$Root) {
    $all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId
    $ids = @($Root)
    $frontier = @($Root)
    while ($frontier.Count -gt 0) {
        $next = @()
        foreach ($p in $frontier) {
            $kids = $all | Where-Object { $_.ParentProcessId -eq $p } | Select-Object -ExpandProperty ProcessId
            foreach ($k in $kids) { if ($ids -notcontains $k) { $ids += $k; $next += $k } }
        }
        $frontier = $next
    }
    return $ids
}

function Get-TreeStats([int]$Root) {
    $ids = Get-Tree $Root
    $rss = 0
    $cpu = 0.0
    foreach ($id in $ids) {
        $pr = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($pr) {
            $rss += $pr.WorkingSet64
            $cpu += $pr.TotalProcessorTime.TotalMilliseconds
        }
    }
    return [pscustomobject]@{ Rss = $rss; Cpu = $cpu; Count = $ids.Count }
}

$a = Get-TreeStats $p.Id
Start-Sleep -Seconds 5
$b = Get-TreeStats $p.Id

$delta = [math]::Max(0, $b.Cpu - $a.Cpu)

$lines = @(
    "# RustChrome measurement",
    "date: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
    "os: $([System.Environment]::OSVersion.VersionString)",
    "cpus: $env:NUMBER_OF_PROCESSORS",
    "exe_bytes: $((Get-Item $Exe).Length)",
    "pid: $($p.Id)",
    "processes: $($a.Count) -> $($b.Count)",
    "rss_kb_after_5s: $([math]::Round($a.Rss / 1kb))",
    "rss_kb_after_10s: $([math]::Round($b.Rss / 1kb))",
    "idle_window_s: 5",
    "idle_cpu_ms: $([math]::Round($delta))",
    "idle_cpu_percent_of_one_core: $([math]::Round($delta / 50))"
)

if (Test-Path $startupLog) {
    $hit = Get-Content $startupLog | Select-String 'startup_ms' | Select-Object -First 1
    if ($hit) { $lines += $hit.ToString() } else { $lines += "startup_ms: not reported" }
} else {
    $lines += "startup_ms: not reported"
}

$lines | Set-Content -Path $Out
Get-Content $Out

Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue
Get-Process msedgewebview2 -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Remove-Item $startupLog -ErrorAction SilentlyContinue
