# The weekly consolidation: runs the consolidator role headless over workspace/life/,
# saves its return to system/logs/consolidation-<date>.md, and raises a review loop.
# Task Scheduler runs it on Sundays (register-weekly-consolidation.ps1); run it by hand the same way.

param(
    [int]$MaxTurns = 100
)

$ErrorActionPreference = 'Stop'

$vault = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
Set-Location $vault
$env:AF_ROOT = $vault

try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch { }

$today = Get-Date
$date = $today.ToString('yyyy-MM-dd')
$daysToMonday = (8 - [int]$today.DayOfWeek) % 7
if ($daysToMonday -eq 0) { $daysToMonday = 7 }
$nextMonday = $today.AddDays($daysToMonday).ToString('yyyy-MM-dd')

$logDir = Join-Path $vault 'system\logs'
if (-not (Test-Path $logDir)) { New-Item -ItemType Directory -Path $logDir | Out-Null }
$logRel = "system/logs/consolidation-$date.md"
$logPath = Join-Path $vault $logRel

$brief = 'Layer: workspace/life/. Pages: all. Pages follow library/schema/knowledge-page.md. ' +
    'Rebuild each Current from its Timeline, archive timeline entries older than six months, ' +
    'propose closures for open.md with the evidence, report life.md against its cap. ' +
    'Do not close or re-date loops, and do not commit. Return your report per your role.'

$claudeArgs = @(
    '-p', $brief,
    '--agent', 'consolidator',
    '--model', 'claude-sonnet-5-5',
    '--max-turns', "$MaxTurns",
    '--permission-mode', 'acceptEdits',
    '--allowedTools', 'Bash(git -C workspace diff *)', 'Bash(git -C workspace status *)', 'Bash(af open due)', 'Bash(af open due *)'
)

$ErrorActionPreference = 'Continue'
$output = (& claude @claudeArgs 2>&1 | ForEach-Object { "$_" }) -join "`n"
$exit = $LASTEXITCODE
if ($null -eq $exit) { $exit = 1 }
$ErrorActionPreference = 'Stop'

$header = "# Consolidation $date`n`nclaude exit code: $exit`n`n"
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($logPath, $header + $output, $utf8)

& af open add "Review the Sunday consolidation: $logRel and git -C workspace diff --stat" --nudge $nextMonday --from consolidation

exit $exit
