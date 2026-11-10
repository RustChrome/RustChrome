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

$rss1 = $proc.WorkingSet64
$cpu1 = $proc.TotalProcessorTime.TotalMilliseconds

Start-Sleep -Seconds 5
$proc.Refresh()
$rss2 = $proc.WorkingSet64
$cpu2 = $proc.TotalProcessorTime.TotalMilliseconds

$delta = [math]::Max(0, $cpu2 - $cpu1)

$lines = @(
    "# RustChrome measurement",
    "date: $((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
    "os: $([System.Environment]::OSVersion.VersionString)",
    "cpus: $env:NUMBER_OF_PROCESSORS",
    "exe_bytes: $((Get-Item $Exe).Length)",
    "pid: $($p.Id)",
    "rss_kb_after_5s: $([math]::Round($rss1 / 1kb))",
    "rss_kb_after_10s: $([math]::Round($rss2 / 1kb))",
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
Remove-Item $startupLog -ErrorAction SilentlyContinue
