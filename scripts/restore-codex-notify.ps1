$ErrorActionPreference = 'Stop'

$codexRoot = Join-Path $env:USERPROFILE '.codex'
$configPath = Join-Path $codexRoot 'config.toml'
if (-not (Test-Path -LiteralPath $configPath)) {
    throw "Codex config file not found: $configPath"
}

# Select a backup that contains only the original Codex Desktop callback.
$cleanBackup = Get-ChildItem -LiteralPath $codexRoot -Filter 'config.toml.*.bak' -File |
    Sort-Object LastWriteTime -Descending |
    Where-Object {
        $line = (Get-Content -LiteralPath $_.FullName | Where-Object { $_ -match '^notify\s*=' } | Select-Object -First 1)
        $line -and $line -match 'codex-computer-use\.exe' -and $line -notmatch 'agent-mail-notifier\.exe' -and $line -notmatch '--previous-notify'
    } |
    Select-Object -First 1

if (-not $cleanBackup) {
    throw 'No clean Codex Desktop notify backup was found. No files were changed.'
}

$sourceNotify = (Get-Content -LiteralPath $cleanBackup.FullName |
    Where-Object { $_ -match '^notify\s*=' } |
    Select-Object -First 1)
$currentLines = Get-Content -LiteralPath $configPath
$notifyIndex = -1
for ($i = 0; $i -lt $currentLines.Count; $i++) {
    if ($currentLines[$i] -match '^notify\s*=') {
        $notifyIndex = $i
        break
    }
}
if ($notifyIndex -lt 0 -or [string]::IsNullOrWhiteSpace($sourceNotify)) {
    throw 'No replaceable notify line was found. No files were changed.'
}

$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$restoreBackup = Join-Path $codexRoot "config.toml.archive-restore-$stamp.bak"
Copy-Item -LiteralPath $configPath -Destination $restoreBackup -Force
$currentLines[$notifyIndex] = $sourceNotify
$tempPath = "$configPath.archive-restore.tmp"
[System.IO.File]::WriteAllText(
    $tempPath,
    ([string]::Join([Environment]::NewLine, $currentLines) + [Environment]::NewLine),
    (New-Object System.Text.UTF8Encoding($false))
)
Move-Item -LiteralPath $tempPath -Destination $configPath -Force

Write-Output 'Codex notify restored to the original Desktop callback.'
Write-Output "Backup of the previous config: $restoreBackup"
Write-Output 'No other config.toml settings were changed.'
