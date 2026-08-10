# Shared Microsoft Office COM activation for AgentFrame native rendering.
#
# Dot-source this file; it defines functions only and runs nothing on import.
#
# Why this exists: `New-Object -ComObject PowerPoint.Application` returns an
# object that looks alive but whose every property reads back null/empty on
# Office Click-to-Run installs (Word and Excel behave the same way). Late
# binding through the ProgID type works correctly. Every AgentFrame Office
# automation path must activate through Get-OfficeApp so the whole system
# shares one working activation and one actionable failure message.
#
# AgentFrame has no LibreOffice fallback by design. LibreOffice substitutes
# fonts it cannot resolve — on a real client deck Calibri became Cooper Black,
# which reflowed every headline, overflowed its text box, and clipped text
# mid-word. A render that lies is worse than no render.

Set-StrictMode -Version Latest

$script:OfficeProductNames = @{
    'PowerPoint.Application' = 'Microsoft PowerPoint'
    'Word.Application'       = 'Microsoft Word'
}

function Get-OfficeProductName([string]$ProgId) {
    if ($script:OfficeProductNames.ContainsKey($ProgId)) {
        return $script:OfficeProductNames[$ProgId]
    }
    return $ProgId
}

function Get-OfficeUnavailableMessage([string]$ProgId, [string]$Detail) {
    $product = Get-OfficeProductName $ProgId
    $lines = @(
        "$product is required but its COM automation server did not respond (ProgID '$ProgId')."
        ''
        'AgentFrame renders Office files through the installed desktop application and has'
        'no LibreOffice fallback: LibreOffice substitutes fonts it cannot resolve and'
        'silently corrupts the render. Fix one of these, then retry:'
        "  1. Install $product (desktop) on this machine."
        "  2. Launch $product once interactively to clear first-run/activation prompts."
        '  3. Produce the deliverable as an HTML deck instead of PPTX.'
    )
    if ($Detail) { $lines += @('', "Underlying error: $Detail") }
    return ($lines -join [Environment]::NewLine)
}

function Get-OfficeVersion($App) {
    if ($null -eq $App) { return $null }
    try {
        $version = $App.Version
        if (-not [string]::IsNullOrWhiteSpace([string]$version)) { return [string]$version }
    }
    catch { }
    try {
        $version = $App.GetType().InvokeMember('Version', 'GetProperty', $null, $App, $null)
        if (-not [string]::IsNullOrWhiteSpace([string]$version)) { return [string]$version }
    }
    catch { }
    return $null
}

function Test-OfficeAppLive($App) {
    # A dead PIA wrapper answers every property with null/empty instead of throwing,
    # so "did Version come back non-empty" is the only reliable liveness probe.
    return $null -ne (Get-OfficeVersion $App)
}

function Invoke-OfficeMethod {
    <#
      Calls a COM method through reflection. PowerShell's COM adapter misreads
      some Office methods as property setters (ExportAsFixedFormat raises
      "Cannot convert the ... value of type int to type Object"), so every
      method that takes enum arguments goes through InvokeMember instead.
    #>
    param(
        [Parameter(Mandatory = $true)] $Target,
        [Parameter(Mandatory = $true)] [string]$Name,
        [object[]]$Arguments = @()
    )
    return $Target.GetType().InvokeMember(
        $Name, 'InvokeMethod', $null, $Target, $Arguments
    )
}

function Get-OfficeApp {
    <#
      Returns @{ App; Type; Created; ProgId }. Attaches to a running instance
      when one exists so an operator's open PowerPoint is never hijacked or
      quit out from under them; only an instance this process created is quit
      by Close-OfficeApp.
    #>
    param([Parameter(Mandatory = $true)] [string]$ProgId)

    $type = [Type]::GetTypeFromProgID($ProgId)
    if ($null -eq $type) {
        throw (Get-OfficeUnavailableMessage $ProgId 'ProgID is not registered on this machine.')
    }

    # Prefer an already-running instance (attach, never quit).
    $existing = $null
    try { $existing = [Runtime.InteropServices.Marshal]::GetActiveObject($ProgId) } catch { }
    if (Test-OfficeAppLive $existing) {
        return @{ App = $existing; Type = $type; Created = $false; ProgId = $ProgId }
    }

    $detail = $null
    $app = $null
    try { $app = [Activator]::CreateInstance($type) } catch { $detail = $_.Exception.Message }

    if (-not (Test-OfficeAppLive $app)) {
        if (-not $detail) {
            $detail = 'Activation returned an object with no readable Version property.'
        }
        throw (Get-OfficeUnavailableMessage $ProgId $detail)
    }
    return @{ App = $app; Type = $type; Created = $true; ProgId = $ProgId }
}

function Close-OfficeApp($Handle) {
    if ($null -eq $Handle) { return }
    if ($Handle.Created) {
        try { Invoke-OfficeMethod -Target $Handle.App -Name 'Quit' | Out-Null } catch { }
    }
    if ($null -ne $Handle.App -and
        [System.Runtime.InteropServices.Marshal]::IsComObject($Handle.App)) {
        try {
            [void][System.Runtime.InteropServices.Marshal]::FinalReleaseComObject($Handle.App)
        }
        catch { }
    }
    [GC]::Collect()
    [GC]::WaitForPendingFinalizers()
}
