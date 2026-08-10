<#
  AgentFrame native Office renderer.

  Converts PPTX/DOCX through the installed Microsoft Office desktop application
  over COM, so a render carries the deck's real fonts, shapes, and text flow.
  This is the only sanctioned PPTX/DOCX rasterization path in AgentFrame; there
  is deliberately no LibreOffice fallback (see office_com.ps1 for why).

  Commands
    probe                                   Report which Office apps can automate.
    pdf   -Source <file> -Output <file>     PPTX/PPT via PowerPoint, DOCX/DOC via Word.
    png   -Source <pptx> -OutputDir <dir>   One PNG per slide (PowerPoint only).

  Slide/page numbering is 1:1 with the deck: hidden slides are included so that
  PDF page N and png prefix N always mean slide N.

  Examples
    powershell -File system/tools/office_render.ps1 probe
    powershell -File system/tools/office_render.ps1 pdf -Source deck.pptx -Output deck.pdf
    powershell -File system/tools/office_render.ps1 png -Source deck.pptx -OutputDir renders
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true, Position = 0)]
    [ValidateSet('probe', 'pdf', 'png')]
    [string]$Command,

    # Not named -Input: $Input is a PowerShell automatic variable.
    [string]$Source,
    [string]$Output,
    [string]$OutputDir,
    [string]$Prefix = 'slide',
    [int]$Width = 2560,
    [int]$Height = 1440,
    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

. (Join-Path $PSScriptRoot 'office_com.ps1')

# PowerPoint: ppFixedFormatTypePDF / ppFixedFormatIntentPrint / msoFalse /
# ppPrintHandoutVerticalFirst / ppPrintOutputSlides / msoTrue(hidden slides)
$PP_PDF = 2
$PP_INTENT_PRINT = 1
$PP_HANDOUT_VERTICAL = 1
$PP_OUTPUT_SLIDES = 1
$MSO_FALSE = 0
$MSO_TRUE = -1
$PP_SAVE_AS_PDF = 32

# Word: wdExportFormatPDF / wdExportOptimizeForPrint / wdAlertsNone
$WD_PDF = 17
$WD_OPTIMIZE_PRINT = 0
$WD_ALERTS_NONE = 0

$POWERPOINT_EXT = @('.pptx', '.ppt', '.pptm', '.ppsx')
$WORD_EXT = @('.docx', '.doc', '.docm', '.rtf')

function Resolve-InputFile([string]$PathValue) {
    if ([string]::IsNullOrWhiteSpace($PathValue)) { throw '-Source is required.' }
    $resolved = Resolve-Path -LiteralPath $PathValue -ErrorAction Stop
    if (-not (Test-Path -LiteralPath $resolved.Path -PathType Leaf)) {
        throw "-Source is not a file: $($resolved.Path)"
    }
    return $resolved.Path
}

function Resolve-OutputFile([string]$PathValue, [string]$Label) {
    if ([string]::IsNullOrWhiteSpace($PathValue)) { throw "$Label is required." }
    $full = [System.IO.Path]::GetFullPath($PathValue)
    if ((Test-Path -LiteralPath $full) -and -not $Force) {
        throw "$Label already exists: $full (pass -Force to replace it)"
    }
    $parent = Split-Path -Parent $full
    if ($parent) { [System.IO.Directory]::CreateDirectory($parent) | Out-Null }
    if (Test-Path -LiteralPath $full) { Remove-Item -LiteralPath $full -Force }
    return $full
}

function Invoke-Probe {
    $report = [ordered]@{}
    foreach ($progId in @('PowerPoint.Application', 'Word.Application')) {
        $key = $progId.Split('.')[0].ToLowerInvariant()
        $handle = $null
        try {
            $handle = Get-OfficeApp -ProgId $progId
            $report[$key] = [ordered]@{
                available = $true
                version   = (Get-OfficeVersion $handle.App)
            }
        }
        catch {
            $report[$key] = [ordered]@{ available = $false; error = $_.Exception.Message }
        }
        finally {
            if ($null -ne $handle) { Close-OfficeApp $handle }
        }
    }
    $report | ConvertTo-Json -Depth 4
}

function Export-PowerPointPdf([string]$Source, [string]$Target) {
    $handle = Get-OfficeApp -ProgId 'PowerPoint.Application'
    $pres = $null
    try {
        $pres = $handle.App.Presentations.Open($Source, $true, $false, $false)
        try {
            # Preferred: a true export, leaving the presentation identity alone.
            Invoke-OfficeMethod -Target $pres -Name 'ExportAsFixedFormat' -Arguments @(
                [object]$Target,
                [object]$PP_PDF,
                [object]$PP_INTENT_PRINT,
                [object]$MSO_FALSE,
                [object]$PP_HANDOUT_VERTICAL,
                [object]$PP_OUTPUT_SLIDES,
                [object]$MSO_TRUE
            ) | Out-Null
        }
        catch {
            # Some builds reject the full ExportAsFixedFormat arity; SaveAs to a
            # brand-new path is equivalent here because the source is read-only.
            Invoke-OfficeMethod -Target $pres -Name 'SaveAs' -Arguments @(
                [object]$Target, [object]$PP_SAVE_AS_PDF
            ) | Out-Null
        }
    }
    finally {
        if ($null -ne $pres) { try { Invoke-OfficeMethod -Target $pres -Name 'Close' | Out-Null } catch { } }
        Close-OfficeApp $handle
    }
}

function Export-WordPdf([string]$Source, [string]$Target) {
    $handle = Get-OfficeApp -ProgId 'Word.Application'
    $doc = $null
    try {
        try { $handle.App.DisplayAlerts = $WD_ALERTS_NONE } catch { }
        # Open(FileName, ConfirmConversions, ReadOnly)
        $doc = $handle.App.Documents.Open($Source, $false, $true)
        try {
            Invoke-OfficeMethod -Target $doc -Name 'ExportAsFixedFormat' -Arguments @(
                [object]$Target,
                [object]$WD_PDF,
                [object]$false,
                [object]$WD_OPTIMIZE_PRINT
            ) | Out-Null
        }
        catch {
            Invoke-OfficeMethod -Target $doc -Name 'SaveAs2' -Arguments @(
                [object]$Target, [object]$WD_PDF
            ) | Out-Null
        }
    }
    finally {
        if ($null -ne $doc) {
            try { Invoke-OfficeMethod -Target $doc -Name 'Close' -Arguments @([object]$false) | Out-Null } catch { }
        }
        Close-OfficeApp $handle
    }
}

function Invoke-Pdf {
    $source = Resolve-InputFile $Source
    $target = Resolve-OutputFile $Output '-Output'
    $ext = [System.IO.Path]::GetExtension($source).ToLowerInvariant()

    if ($POWERPOINT_EXT -contains $ext) { Export-PowerPointPdf $source $target }
    elseif ($WORD_EXT -contains $ext) { Export-WordPdf $source $target }
    else { throw "Unsupported input extension '$ext'. Expected one of: $(($POWERPOINT_EXT + $WORD_EXT) -join ', ')" }

    if (-not (Test-Path -LiteralPath $target -PathType Leaf)) {
        throw "Office reported success but produced no PDF at $target"
    }
    Write-Output $target
}

function Invoke-Png {
    $source = Resolve-InputFile $Source
    $ext = [System.IO.Path]::GetExtension($source).ToLowerInvariant()
    if ($POWERPOINT_EXT -notcontains $ext) {
        throw "png renders presentations only; '$ext' is not a PowerPoint file."
    }
    if ([string]::IsNullOrWhiteSpace($OutputDir)) { throw '-OutputDir is required for png.' }
    $renderDir = [System.IO.Path]::GetFullPath($OutputDir)
    [System.IO.Directory]::CreateDirectory($renderDir) | Out-Null

    $handle = Get-OfficeApp -ProgId 'PowerPoint.Application'
    $pres = $null
    try {
        $pres = $handle.App.Presentations.Open($source, $true, $false, $false)
        $count = $pres.Slides.Count
        for ($i = 1; $i -le $count; $i++) {
            $target = Join-Path $renderDir ("{0}_{1:D2}.png" -f $Prefix, $i)
            if (Test-Path -LiteralPath $target) { Remove-Item -LiteralPath $target -Force }
            $pres.Slides.Item($i).Export($target, 'PNG', $Width, $Height)
            Write-Output $target
        }
    }
    finally {
        if ($null -ne $pres) { try { Invoke-OfficeMethod -Target $pres -Name 'Close' | Out-Null } catch { } }
        Close-OfficeApp $handle
    }
}

try {
    switch ($Command) {
        'probe' { Invoke-Probe }
        'pdf' { Invoke-Pdf }
        'png' { Invoke-Png }
    }
    exit 0
}
catch {
    [Console]::Error.WriteLine("office-render: $($_.Exception.Message)")
    exit 2
}
