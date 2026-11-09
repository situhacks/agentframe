#Requires -RunAsAdministrator
<#
.SYNOPSIS
    One-time setup of a Windows laptop as AgentFrame's always-on machine. Run it yourself, as administrator.

.DESCRIPTION
    Makes three kinds of change, each printed before it is made, then prints a summary with the
    undo command for each change it made:

      1. Power, on wall power only: never sleep, never hibernate, closing the lid does nothing.
         The display timeout is left alone unless -DisplayOffMinutes is given.
      2. The sync hub: creates the hub folder, limits it to one named user (plus SYSTEM and
         Administrators), and shares it under -ShareName with full access for that user only.
         Other accounts on the laptop get no access; no guest or Everyone access. The folder's
         permissions are read back afterwards; if any other account still has an entry on the
         folder or anything in it, the script stops, names the entries, and prints the reset
         command. It never resets an existing folder's permissions on its own.
      3. The firewall: inbound file sharing (SMB) and local name lookup, on the Private profile
         only, from the local subnet only, as rules in the firewall group 'AgentFrame hub'.

    Before any change it checks the network adapter: the one -InterfaceAlias names, or else each
    connected adapter with a default gateway (so virtual adapters from WSL or Hyper-V are left
    out). If that network is Public, it says so and stops with nothing changed, unless
    -SetNetworkPrivate asks it to make it Private. It sets at most one adapter Private unless
    -InterfaceAlias picks it. Use -SetNetworkPrivate only on your own home network.

    It stores no password and turns on no automatic sign-in; the README describes sign-in at startup.
    Rerunning it is safe: each step checks the current state first. If a step fails, the summary
    names it, and the steps after it have not run.

.PARAMETER HubPath
    The hub folder. Default C:\afhub.

.PARAMETER ShareName
    The network share name for the hub. Default afhub. The desktop reaches it as \\<laptop-name>\<ShareName>.

.PARAMETER ShareUser
    The one account given full access to the share and the folder. Default: the account running
    this script, as DOMAIN\user (on a home laptop, DOMAIN is the laptop's own name).

.PARAMETER DisplayOffMinutes
    Optional. Turns the display off after this many minutes on wall power; 0 means never.
    Leave it out to keep the current display timeout.

.PARAMETER InterfaceAlias
    Optional. The network adapter to check and, with -SetNetworkPrivate, set Private, as
    Get-NetConnectionProfile names it (for example Wi-Fi or Ethernet).

.PARAMETER SetNetworkPrivate
    Sets the home network to Private instead of stopping when it is Public.

.EXAMPLE
    .\setup-laptop.ps1 -WhatIf
    Prints every change it would make and makes none.

.EXAMPLE
    .\setup-laptop.ps1 -InterfaceAlias Wi-Fi -SetNetworkPrivate

.NOTES
    UNDO, in an administrator PowerShell. The closing summary prints these with the exact values
    that were replaced; the values below are common Windows defaults.

      Sleep on wall power (seconds; 1800 is 30 minutes):
        powercfg /setacvalueindex SCHEME_CURRENT SUB_SLEEP STANDBYIDLE 1800; powercfg /setactive SCHEME_CURRENT
      Hibernate on wall power (seconds; 0 is never):
        powercfg /setacvalueindex SCHEME_CURRENT SUB_SLEEP HIBERNATEIDLE 10800; powercfg /setactive SCHEME_CURRENT
      Lid close on wall power (0 nothing, 1 sleep, 2 hibernate, 3 shut down):
        powercfg /setacvalueindex SCHEME_CURRENT SUB_BUTTONS LIDACTION 1; powercfg /setactive SCHEME_CURRENT
      Display timeout on wall power (seconds), only if -DisplayOffMinutes was given:
        powercfg /setacvalueindex SCHEME_CURRENT SUB_VIDEO VIDEOIDLE 600; powercfg /setactive SCHEME_CURRENT
      The share:
        Remove-SmbShare -Name 'afhub' -Force
      The folder permission (back to what the drive passes down):
        icacls 'C:\afhub' /reset /T /C
      The hub folder, once nothing in it is needed:
        Remove-Item -Recurse -Force 'C:\afhub'
      The firewall rules:
        Remove-NetFirewallRule -Group 'AgentFrame hub'
      A network this script set to Private:
        Set-NetConnectionProfile -InterfaceAlias '<alias>' -NetworkCategory Public
#>
[CmdletBinding(SupportsShouldProcess = $true)]
param(
    [ValidateNotNullOrEmpty()][string]$HubPath = 'C:\afhub',
    [ValidateNotNullOrEmpty()][string]$ShareName = 'afhub',
    [ValidateNotNullOrEmpty()][string]$ShareUser = "$env:USERDOMAIN\$env:USERNAME",
    [ValidateRange(0, 1440)][int]$DisplayOffMinutes = 0,
    [string]$InterfaceAlias,
    [switch]$SetNetworkPrivate
)

$ErrorActionPreference = 'Stop'
$script:Cmdlet = $PSCmdlet
$script:Changes = New-Object System.Collections.Generic.List[object]
$script:CurrentStep = 'preflight'
$script:Failure = $null
$FirewallGroup = 'AgentFrame hub'
$SystemSid = 'S-1-5-18'
$AdminsSid = 'S-1-5-32-544'

function Add-Record([string]$Status, [string]$What, [string]$Undo) {
    $script:Changes.Add([pscustomobject]@{ Status = $Status; What = $What; Undo = $Undo })
}

function Invoke-Change {
    # Prints the change, makes it unless -WhatIf or a No under -Confirm, and records it with its
    # undo command. The action runs here, at once, so it reads the caller's variables; keep these
    # parameter names out of it.
    param([string]$What, [string]$Undo, [scriptblock]$Action)
    Write-Host "CHANGE: $What"
    $script:CurrentStep = $What
    if (-not $script:Cmdlet.ShouldProcess($What, 'Change')) {
        if ($WhatIfPreference) { Add-Record 'would change (WhatIf)' $What '' } else { Add-Record 'declined' $What '' }
        return
    }
    try {
        & $Action
    } catch {
        Add-Record 'FAILED' $What ''
        throw
    }
    Add-Record 'done' $What $Undo
}

function Note-Unchanged([string]$What) {
    Write-Host "OK:     $What"
    Add-Record 'already so' $What ''
}

function Invoke-Native {
    # Runs a native command and throws on a non-zero exit, so a failed step stops the script.
    param([string]$Exe, [string[]]$Arguments)
    & $Exe @Arguments | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "$Exe $($Arguments -join ' ') failed with exit code $LASTEXITCODE" }
}

function Get-AcPowerValue([string]$Subgroup, [string]$Setting) {
    # The current wall-power value of one setting. The AC and DC values are the last two hex
    # numbers powercfg prints, in that order, whatever the display language.
    $ErrorActionPreference = 'Continue'
    $out = & powercfg /query SCHEME_CURRENT $Subgroup $Setting 2>$null
    if ($LASTEXITCODE -ne 0) { return $null }
    $hex = @($out | Select-String -Pattern '0x[0-9a-fA-F]+' -AllMatches | ForEach-Object { $_.Matches } | ForEach-Object { $_.Value })
    if ($hex.Count -lt 2) { return $null }
    return [Convert]::ToInt64($hex[$hex.Count - 2], 16)
}

function Set-AcPowerValue([string]$Subgroup, [string]$Setting, [long]$Value, [string]$Label, [string]$Shown) {
    $script:CurrentStep = $Label
    $current = Get-AcPowerValue $Subgroup $Setting
    if ($null -ne $current -and $current -eq $Value) { Note-Unchanged "$Label is already $Shown"; return }
    $was = 'unknown'
    $undoValue = '<previous value>'
    if ($null -ne $current) { $was = "$current"; $undoValue = "$current" }
    Invoke-Change -What "$Label -> $Shown (was $was)" `
        -Undo "powercfg /setacvalueindex SCHEME_CURRENT $Subgroup $Setting $undoValue; powercfg /setactive SCHEME_CURRENT" `
        -Action {
            Invoke-Native 'powercfg' @('/setacvalueindex', 'SCHEME_CURRENT', $Subgroup, $Setting, "$Value")
            Invoke-Native 'powercfg' @('/setactive', 'SCHEME_CURRENT')
        }
}

function Get-Sid([string]$Account) {
    try {
        $nt = New-Object System.Security.Principal.NTAccount($Account)
        return $nt.Translate([System.Security.Principal.SecurityIdentifier]).Value
    } catch { return $null }
}

function Get-AccountName([string]$Sid) {
    try {
        $s = New-Object System.Security.Principal.SecurityIdentifier($Sid)
        return $s.Translate([System.Security.Principal.NTAccount]).Value
    } catch { return $Sid }
}

function Get-AclRules([string]$Path) {
    $acl = Get-Acl -LiteralPath $Path
    return @($acl.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier]))
}

function Get-ExtraAclEntries([string]$Path, [string[]]$Allowed) {
    # Every entry, on the folder or anything in it, for an account outside the intended set.
    $extra = New-Object System.Collections.Generic.List[string]
    $items = @($Path) + @(Get-ChildItem -LiteralPath $Path -Recurse -Force -ErrorAction SilentlyContinue | ForEach-Object { $_.FullName })
    foreach ($item in $items) {
        foreach ($rule in (Get-AclRules $item)) {
            $sid = $rule.IdentityReference.Value
            if ($Allowed -contains $sid) { continue }
            $extra.Add(("{0}: {1} {2} {3}" -f $item, (Get-AccountName $sid), $rule.AccessControlType, $rule.FileSystemRights))
        }
    }
    return , $extra
}

function Test-HubAclIsExact([string]$Path, [string[]]$Allowed) {
    # True when the folder inherits nothing and each intended account has full control.
    $acl = Get-Acl -LiteralPath $Path
    if (-not $acl.AreAccessRulesProtected) { return $false }
    $rules = Get-AclRules $Path
    foreach ($sid in $Allowed) {
        $full = @($rules | Where-Object { $_.IdentityReference.Value -eq $sid -and "$($_.AccessControlType)" -eq 'Allow' -and
            ($_.FileSystemRights -band [System.Security.AccessControl.FileSystemRights]::FullControl) -eq [System.Security.AccessControl.FileSystemRights]::FullControl })
        if ($full.Count -eq 0) { return $false }
    }
    return ((Get-ExtraAclEntries $Path $Allowed).Count -eq 0)
}

function Write-Summary {
    Write-Host ''
    Write-Host 'Summary'
    Write-Host '-------'
    if ($script:Changes.Count -eq 0) { Write-Host 'Nothing was changed.' }
    foreach ($c in $script:Changes) {
        Write-Host ("[{0}] {1}" -f $c.Status, $c.What)
        if ($c.Undo -and $c.Status -eq 'done') { Write-Host ("    undo: {0}" -f $c.Undo) }
    }
    if ($script:Failure) {
        Write-Host ''
        Write-Host "STOPPED at: $($script:CurrentStep)"
        Write-Host "Reason: $($script:Failure)"
        Write-Host 'The steps after it did not run. Fix the reason and rerun; finished steps show as already so.'
    }
}

# --- Preflight: nothing changes until these pass -------------------------------------------------

$userSid = Get-Sid $ShareUser
if (-not $userSid) { throw "The account '$ShareUser' was not found on this machine. Pass -ShareUser DOMAIN\user." }
if (-not [System.IO.Path]::IsPathRooted($HubPath)) { throw "-HubPath must be a full path, for example C:\afhub." }
$allowedSids = @($userSid, $SystemSid, $AdminsSid)

$existingShare = Get-SmbShare -Name $ShareName -ErrorAction SilentlyContinue
if ($existingShare -and ($existingShare.Path.TrimEnd('\') -ne $HubPath.TrimEnd('\'))) {
    throw "A share named '$ShareName' already exists for $($existingShare.Path). Pick another -ShareName or remove that share first."
}

# The home network: the adapter -InterfaceAlias names, or each connected adapter with a default
# gateway. Virtual adapters (WSL, Hyper-V) have none, so they are not checked or changed.
$connected = @(Get-NetConnectionProfile -ErrorAction SilentlyContinue)
if ($InterfaceAlias) {
    $networks = @($connected | Where-Object { $_.InterfaceAlias -eq $InterfaceAlias })
    if ($networks.Count -eq 0) {
        $names = ($connected | ForEach-Object { "'$($_.InterfaceAlias)'" }) -join ', '
        if (-not $names) { $names = 'none' }
        Write-Host "No connected network on adapter '$InterfaceAlias'. Connected adapters: $names."
        Write-Host 'Nothing was changed.'
        exit 1
    }
} else {
    $gateways = @(Get-NetRoute -DestinationPrefix '0.0.0.0/0', '::/0' -ErrorAction SilentlyContinue | ForEach-Object { $_.InterfaceIndex } | Sort-Object -Unique)
    $networks = @($connected | Where-Object { $gateways -contains $_.InterfaceIndex })
}
foreach ($n in $networks) { Write-Host "Network on '$($n.InterfaceAlias)': $($n.NetworkCategory)" }
if ($networks.Count -eq 0) {
    Write-Warning 'No connected network with a default gateway found. The firewall rules apply to Private networks only; make sure the home network is Private once connected.'
}
$public = @($networks | Where-Object { "$($_.NetworkCategory)" -eq 'Public' })
if ($public.Count -gt 0 -and -not $SetNetworkPrivate) {
    foreach ($p in $public) { Write-Host "The network on '$($p.InterfaceAlias)' is Public." }
    Write-Host 'File sharing is allowed on Private networks only, so the desktop could not reach the hub.'
    Write-Host 'Nothing was changed. On your home network, rerun with -SetNetworkPrivate (and -InterfaceAlias <adapter> to pick one), or set the network to Private in Settings, then rerun.'
    exit 1
}
if ($public.Count -gt 1 -and -not $InterfaceAlias) {
    Write-Host ('More than one adapter is on a Public network: ' + (($public | ForEach-Object { "'$($_.InterfaceAlias)'" }) -join ', ') + '.')
    Write-Host 'Nothing was changed. Rerun with -InterfaceAlias <adapter> to choose the home network.'
    exit 1
}

Write-Host "Laptop setup: hub $HubPath shared as '$ShareName' for $ShareUser"
if ($WhatIfPreference) { Write-Host 'WhatIf: printing the changes, making none.' }
Write-Host ''

try {
    # --- Network ---------------------------------------------------------------------------------
    foreach ($p in $public) {
        $alias = $p.InterfaceAlias
        $ifIndex = $p.InterfaceIndex
        Invoke-Change -What "Network on '$alias': Public -> Private" `
            -Undo "Set-NetConnectionProfile -InterfaceAlias '$alias' -NetworkCategory Public" `
            -Action { Set-NetConnectionProfile -InterfaceIndex $ifIndex -NetworkCategory Private }
    }

    # --- Power, on wall power --------------------------------------------------------------------
    Set-AcPowerValue 'SUB_SLEEP' 'STANDBYIDLE' 0 'Sleep on wall power' 'never'
    Set-AcPowerValue 'SUB_SLEEP' 'HIBERNATEIDLE' 0 'Hibernate on wall power' 'never'
    Set-AcPowerValue 'SUB_BUTTONS' 'LIDACTION' 0 'Lid close on wall power' 'do nothing'
    if ($PSBoundParameters.ContainsKey('DisplayOffMinutes')) {
        $shown = "$DisplayOffMinutes minutes"
        if ($DisplayOffMinutes -eq 0) { $shown = 'never' }
        Set-AcPowerValue 'SUB_VIDEO' 'VIDEOIDLE' ([long]$DisplayOffMinutes * 60) 'Display off on wall power' $shown
    }

    # --- Hub folder ------------------------------------------------------------------------------
    $script:CurrentStep = 'Hub folder'
    if (Test-Path -LiteralPath $HubPath -PathType Container) {
        Note-Unchanged "Hub folder $HubPath exists"
    } else {
        Invoke-Change -What "Create the hub folder $HubPath" `
            -Undo "Remove-Item -Recurse -Force '$HubPath'   # once nothing in it is needed" `
            -Action { New-Item -ItemType Directory -Path $HubPath | Out-Null }
    }

    # Folder permission: the one user, SYSTEM and Administrators only, so the other accounts on the
    # laptop cannot open the hub locally either. The user's own entry keeps the share working when
    # Windows filters an administrator's rights over the network. Read back afterwards: an entry an
    # existing folder or file already carried survives icacls, and is reported, never reset here.
    $script:CurrentStep = 'Folder permission'
    $permissionWhat = "Folder permission: $HubPath for $ShareUser, SYSTEM and Administrators only (other accounts lose access)"
    if ((Test-Path -LiteralPath $HubPath -PathType Container) -and (Test-HubAclIsExact $HubPath $allowedSids)) {
        Note-Unchanged "Folder permission: $HubPath is limited to $ShareUser, SYSTEM and Administrators"
    } else {
        Invoke-Change -What $permissionWhat `
            -Undo "icacls '$HubPath' /reset /T /C   # back to the permissions inherited from the drive" `
            -Action {
                if (-not (Test-Path -LiteralPath $HubPath -PathType Container)) { throw "The hub folder $HubPath does not exist." }
                Invoke-Native 'icacls' @($HubPath, '/inheritance:r', '/grant:r',
                    "*${userSid}:(OI)(CI)F", "*${SystemSid}:(OI)(CI)F", "*${AdminsSid}:(OI)(CI)F")
                $extra = Get-ExtraAclEntries $HubPath $allowedSids
                if ($extra.Count -gt 0) {
                    $shownList = (@($extra) | Select-Object -First 10) -join '; '
                    if ($extra.Count -gt 10) { $shownList += "; and $($extra.Count - 10) more" }
                    throw ("Other accounts still have entries on the hub or files in it, so it is not limited to {0}: {1}. " +
                        "To clear them, run: icacls '{2}' /reset /T /C   then rerun this script.") -f $ShareUser, $shownList, $HubPath
                }
            }
    }

    # --- Share -----------------------------------------------------------------------------------
    $script:CurrentStep = 'Share'
    if (-not $existingShare) {
        Invoke-Change -What "Share $HubPath as '$ShareName', full access for $ShareUser only" `
            -Undo "Remove-SmbShare -Name '$ShareName' -Force" `
            -Action { New-SmbShare -Name $ShareName -Path $HubPath -FullAccess $ShareUser -Description 'AgentFrame sync hub' | Out-Null }
    } else {
        Note-Unchanged "Share '$ShareName' exists for $HubPath"
    }

    # Whether new or old, the share ends with one allow entry: this user, full access.
    $script:CurrentStep = 'Share access'
    if (Get-SmbShare -Name $ShareName -ErrorAction SilentlyContinue) {
        $access = @(Get-SmbShareAccess -Name $ShareName)
        $mine = @($access | Where-Object { "$($_.AccessControlType)" -eq 'Allow' -and (Get-Sid $_.AccountName) -eq $userSid -and "$($_.AccessRight)" -eq 'Full' })
        $others = @($access | Where-Object { "$($_.AccessControlType)" -eq 'Allow' -and (Get-Sid $_.AccountName) -ne $userSid })
        if ($mine.Count -gt 0 -and $others.Count -eq 0) {
            Note-Unchanged "Share '$ShareName' allows $ShareUser only, full access"
        }
        if ($mine.Count -eq 0) {
            Invoke-Change -What "Share '$ShareName': grant full access to $ShareUser" `
                -Undo "Revoke-SmbShareAccess -Name '$ShareName' -AccountName '$ShareUser' -Force" `
                -Action { Grant-SmbShareAccess -Name $ShareName -AccountName $ShareUser -AccessRight Full -Force | Out-Null }
        }
        foreach ($entry in $others) {
            $other = $entry.AccountName
            $right = "$($entry.AccessRight)"
            Invoke-Change -What "Share '$ShareName': remove $right access for $other" `
                -Undo "Grant-SmbShareAccess -Name '$ShareName' -AccountName '$other' -AccessRight $right -Force" `
                -Action { Revoke-SmbShareAccess -Name $ShareName -AccountName $other -Force | Out-Null }
        }
    }

    # --- Firewall: Private profile, local subnet, inbound only -----------------------------------
    $rules = @(
        @{ Name = 'AgentFrame hub: file sharing (SMB, TCP 445)'; Protocol = 'TCP'; Port = '445' },
        @{ Name = 'AgentFrame hub: name lookup (NetBIOS, UDP 137)'; Protocol = 'UDP'; Port = '137' },
        @{ Name = 'AgentFrame hub: name lookup (LLMNR, UDP 5355)'; Protocol = 'UDP'; Port = '5355' },
        @{ Name = 'AgentFrame hub: name lookup (mDNS, UDP 5353)'; Protocol = 'UDP'; Port = '5353' }
    )
    foreach ($r in $rules) {
        $name = $r.Name
        $protocol = $r.Protocol
        $port = $r.Port
        $script:CurrentStep = "Firewall rule '$name'"
        $existing = @(Get-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue)
        if ($existing.Count -eq 1 -and "$($existing[0].Profile)" -eq 'Private' -and "$($existing[0].Enabled)" -eq 'True' -and "$($existing[0].Action)" -eq 'Allow') {
            Note-Unchanged "Firewall rule '$name' is in place (Private profile)"
            continue
        }
        $verb = 'Add'
        if ($existing.Count -gt 0) { $verb = 'Replace' }
        Invoke-Change -What "$verb firewall rule '$name' (inbound, Private profile, local subnet)" `
            -Undo "Remove-NetFirewallRule -Group '$FirewallGroup'" `
            -Action {
                Get-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue | Remove-NetFirewallRule
                New-NetFirewallRule -DisplayName $name -Group $FirewallGroup -Direction Inbound -Action Allow `
                    -Protocol $protocol -LocalPort $port -Profile Private -RemoteAddress LocalSubnet | Out-Null
            }
    }
} catch {
    $script:Failure = $_.Exception.Message
} finally {
    Write-Summary
}

if ($script:Failure) { exit 1 }

Write-Host ''
Write-Host "The desktop reaches the hub as \\$env:COMPUTERNAME\$ShareName, signing in as $ShareUser."
Write-Host 'Next: sign-in at startup and the rest of the hand steps in the README beside this script.'
