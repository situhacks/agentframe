[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('render', 'replace')]
    [string]$Command,

    [string]$Deck,
    [string]$OutputDir,

    [string]$InputDeck,
    [string]$OutputDeck,
    [string]$ReplacementDeck,
    [int]$TargetSlide,
    [int]$ReplacementSlide = 1,
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'office_com.ps1')

function Resolve-ExistingFile([string]$PathValue, [string]$Label) {
    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        throw "$Label is required."
    }
    $resolved = Resolve-Path -LiteralPath $PathValue -ErrorAction Stop
    if (-not (Test-Path -LiteralPath $resolved.Path -PathType Leaf)) {
        throw "$Label is not a file: $($resolved.Path)"
    }
    return $resolved.Path
}

function Release-ComObject($Value) {
    if ($null -ne $Value -and [System.Runtime.InteropServices.Marshal]::IsComObject($Value)) {
        [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($Value)
    }
}

$handle = $null
$powerPoint = $null
$presentation = $null

try {
    # Activation goes through Get-OfficeApp: `New-Object -ComObject` returns an
    # object whose every property reads back null on Click-to-Run installs, which
    # surfaced here as "You cannot call a method on a null-valued expression."
    $handle = Get-OfficeApp -ProgId 'PowerPoint.Application'
    $powerPoint = $handle.App

    if ($Command -eq 'render') {
        $deckPath = Resolve-ExistingFile $Deck 'Deck'
        if ([string]::IsNullOrWhiteSpace($OutputDir)) {
            throw 'OutputDir is required for render.'
        }
        $renderDir = [System.IO.Path]::GetFullPath($OutputDir)
        [System.IO.Directory]::CreateDirectory($renderDir) | Out-Null
        $presentation = $powerPoint.Presentations.Open($deckPath, $true, $false, $false)
        for ($index = 1; $index -le $presentation.Slides.Count; $index++) {
            $target = Join-Path $renderDir ('origin_{0:D2}.png' -f $index)
            $presentation.Slides.Item($index).Export($target, 'PNG', 2560, 1440)
        }
        Write-Output $renderDir
        exit 0
    }

    $inputPath = Resolve-ExistingFile $InputDeck 'InputDeck'
    $replacementPath = Resolve-ExistingFile $ReplacementDeck 'ReplacementDeck'
    if ([string]::IsNullOrWhiteSpace($OutputDeck)) {
        throw 'OutputDeck is required for replace.'
    }
    $outputPath = [System.IO.Path]::GetFullPath($OutputDeck)
    if ($outputPath -eq $inputPath) {
        throw 'OutputDeck must differ from InputDeck; source decks are never edited in place.'
    }
    if ($outputPath -eq $replacementPath) {
        throw 'OutputDeck must differ from ReplacementDeck.'
    }
    if (Test-Path -LiteralPath $outputPath) {
        if (-not $Force) {
            throw "OutputDeck already exists: $outputPath (pass -Force to replace it)"
        }
        Remove-Item -LiteralPath $outputPath -Force
    }
    $outputParent = Split-Path -Parent $outputPath
    [System.IO.Directory]::CreateDirectory($outputParent) | Out-Null
    Copy-Item -LiteralPath $inputPath -Destination $outputPath

    $presentation = $powerPoint.Presentations.Open($outputPath, $false, $false, $false)
    if ($TargetSlide -lt 1 -or $TargetSlide -gt $presentation.Slides.Count) {
        throw "TargetSlide $TargetSlide is outside 1..$($presentation.Slides.Count)."
    }

    $replacementProbe = $powerPoint.Presentations.Open(
        $replacementPath, $true, $false, $false
    )
    try {
        if ($ReplacementSlide -lt 1 -or $ReplacementSlide -gt $replacementProbe.Slides.Count) {
            throw "ReplacementSlide $ReplacementSlide is outside 1..$($replacementProbe.Slides.Count)."
        }
    }
    finally {
        $replacementProbe.Close()
        Release-ComObject $replacementProbe
    }

    $beforeCount = $presentation.Slides.Count
    [void]$presentation.Slides.InsertFromFile(
        $replacementPath, $TargetSlide, $ReplacementSlide, $ReplacementSlide
    )
    if ($presentation.Slides.Count -ne ($beforeCount + 1)) {
        throw 'PowerPoint did not insert exactly one replacement slide.'
    }
    $presentation.Slides.Item($TargetSlide).Delete()
    if ($presentation.Slides.Count -ne $beforeCount) {
        throw 'Slide replacement changed the deck slide count.'
    }
    $presentation.Save()
    Write-Output $outputPath
}
finally {
    if ($null -ne $presentation) {
        try { $presentation.Close() } catch { }
        Release-ComObject $presentation
    }
    # Close-OfficeApp quits only an instance this script created, so an operator's
    # already-open PowerPoint is never shut down underneath them.
    Close-OfficeApp $handle
}
