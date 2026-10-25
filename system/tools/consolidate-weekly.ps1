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

# Register the run as a worker before it starts, so its first session start prints this
# brief instead of the off-system question an unregistered chat gets.
$sessionName = 'worker/consolidation'
$sessionId = [guid]::NewGuid().ToString()
$sessionDir = Join-Path $vault 'system\sessions\worker\consolidation'
if (-not (Test-Path $sessionDir)) { New-Item -ItemType Directory -Path $sessionDir | Out-Null }
$briefPath = Join-Path $sessionDir 'brief.md'
$briefText = "# Brief: worker/consolidation`n`nGoal: the weekly consolidation of workspace/life/.`nDone when: every knowledge page's Current is rebuilt from its Timeline, old entries are archived, closures are proposed, and life.md is reported against its cap.`nWrite surface: workspace/life/knowledge/, workspace/life/knowledge/_archive/, system/sessions/worker/consolidation/.`nContext: library/schema/knowledge-page.md, library/schema/open-loops.md.`n`n$brief`n"
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($briefPath, $briefText, $utf8)
& af session register $sessionName --session $sessionId --model claude-sonnet-5-5 --brief $briefPath | Out-Null

$claudeArgs = @(
    '-p', $brief,
    '--session-id', $sessionId,
    '--agent', 'consolidator',
    '--model', 'claude-sonnet-5-5',
    '--max-turns', "$MaxTurns",
    '--permission-mode', 'acceptEdits',
    '--allowedTools', 'Bash(git -C workspace diff *)', 'Bash(git -C workspace status *)', 'Bash(af open due)', 'Bash(af open due *)', 'Bash(af search *)'
)

$ErrorActionPreference = 'Continue'
$output = (& claude @claudeArgs 2>&1 | ForEach-Object { "$_" }) -join "`n"
$exit = $LASTEXITCODE
if ($null -eq $exit) { $exit = 1 }
& af session end $sessionName --reason "weekly run finished, claude exit $exit" | Out-Null
$ErrorActionPreference = 'Stop'

$header = "# Consolidation $date`n`nclaude exit code: $exit`n`n"
$utf8 = New-Object System.Text.UTF8Encoding($false)
[System.IO.File]::WriteAllText($logPath, $header + $output, $utf8)

& af open add "Review the Sunday consolidation: $logRel and git -C workspace diff --stat" --nudge $nextMonday --from consolidation

exit $exit
