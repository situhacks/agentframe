"""The helper windows the live desktop tests open (test_desktop_tool.py), as PowerShell and C# source text.

This module only holds text, so test_desktop_static.py can parse it without a desktop. A helper window never
outlives its test run by long: each form closes itself 30 s after the test process stops touching its heartbeat
file, the thief window closes itself 15 s after it opens and never stays topmost, and the named executable
closes itself after 30 s.
"""

# The heartbeat file is touched every few seconds by the test run. If the run is killed, the forms close
# within HEARTBEAT_LIMIT_SECONDS; a form with no heartbeat file at all closes after the same time.
HEARTBEAT_LIMIT_SECONDS = 30
THIEF_LIFETIME_SECONDS = 15
NAMED_EXE_LIFETIME_SECONDS = 30

FIXTURE = r"""
param([string]$Title, [string]$StateFile, [string]$CommandFile, [string]$HeartbeatFile, [int]$Left = 200)
Add-Type -AssemblyName System.Windows.Forms, System.Drawing

$script:Pressed = 0
$script:LabelClicks = 0
$script:CommandsDone = 0
$script:OnType = ''
$script:Thief = $null
$script:ThiefHolder = $null
$script:ThiefEnd = $null
$script:Started = Get-Date

function Write-State {
    $json = @{
        title = $form.Text; text = $editor.Text; secret_length = $secret.TextLength
        pressed = $script:Pressed; label_clicks = $script:LabelClicks; commands_done = $script:CommandsDone
    } | ConvertTo-Json -Compress
    $temp = "$StateFile.tmp"
    [IO.File]::WriteAllText($temp, $json)
    for ($try = 0; $try -lt 10; $try++) {
        try { Move-Item -LiteralPath $temp -Destination $StateFile -Force; return } catch { Start-Sleep -Milliseconds 20 }
    }
}

function Stop-Thief {
    if ($script:ThiefHolder) { $script:ThiefHolder.Stop(); $script:ThiefHolder.Dispose(); $script:ThiefHolder = $null }
    if ($script:ThiefEnd) { $script:ThiefEnd.Stop(); $script:ThiefEnd.Dispose(); $script:ThiefEnd = $null }
    if ($script:Thief) {
        $script:Thief.TopMost = $false
        $script:Thief.Close()
        $script:Thief = $null
    }
}

function Start-Thief([string]$ThiefTitle) {
    Stop-Thief
    $script:Thief = New-Object Windows.Forms.Form
    $script:Thief.Text = $ThiefTitle
    $script:Thief.TopMost = $true
    $script:Thief.StartPosition = 'Manual'
    $script:Thief.Location = New-Object Drawing.Point(($Left + 60), 260)
    $script:Thief.Show()
    $script:Thief.Activate()
    $script:ThiefHolder = New-Object Windows.Forms.Timer
    $script:ThiefHolder.Interval = 40
    $script:ThiefHolder.Add_Tick({ if ($script:Thief) { $script:Thief.Activate() } })
    $script:ThiefHolder.Start()
    # The thief gives the foreground back on its own, whatever happens to the test run.
    $script:ThiefEnd = New-Object Windows.Forms.Timer
    $script:ThiefEnd.Interval = 15000
    $script:ThiefEnd.Add_Tick({ Stop-Thief })
    $script:ThiefEnd.Start()
}

$form = New-Object Windows.Forms.Form
$form.Text = $Title
$form.StartPosition = 'Manual'
$form.Location = New-Object Drawing.Point($Left, 200)
$form.ClientSize = New-Object Drawing.Size(480, 320)
$form.BackColor = [Drawing.Color]::Magenta
$form.TopMost = $true

$editor = New-Object Windows.Forms.TextBox
$editor.Name = 'editor'; $editor.AccessibleName = 'Editor'; $editor.Multiline = $true
$editor.Location = New-Object Drawing.Point(10, 10); $editor.Size = New-Object Drawing.Size(300, 60)

$secret = New-Object Windows.Forms.TextBox
$secret.Name = 'secret'; $secret.AccessibleName = 'Secret'; $secret.UseSystemPasswordChar = $true
$secret.Location = New-Object Drawing.Point(10, 80); $secret.Size = New-Object Drawing.Size(300, 24)

$button = New-Object Windows.Forms.Button
$button.Name = 'pressMe'; $button.Text = 'Press Me'
$button.Location = New-Object Drawing.Point(10, 115); $button.Size = New-Object Drawing.Size(120, 30)

$disabled = New-Object Windows.Forms.Button
$disabled.Name = 'neverEnabled'; $disabled.Text = 'Never Enabled'; $disabled.Enabled = $false
$disabled.Location = New-Object Drawing.Point(140, 115); $disabled.Size = New-Object Drawing.Size(120, 30)

$label = New-Object Windows.Forms.Label
$label.Name = 'fallbackLabel'; $label.Text = 'Fallback Target'; $label.BackColor = [Drawing.Color]::White
$label.Location = New-Object Drawing.Point(10, 155); $label.Size = New-Object Drawing.Size(160, 30)

$form.Controls.AddRange(@($editor, $secret, $button, $disabled, $label))

$button.Add_Click({ $script:Pressed++; Write-State })
$label.Add_Click({ $script:LabelClicks++; Write-State })
$secret.Add_TextChanged({ Write-State })
$editor.Add_TextChanged({
    if ($script:OnType) {
        $action = $script:OnType
        $script:OnType = ''
        if ($action.StartsWith('thief=')) { Start-Thief $action.Substring(6) }
        elseif ($action.StartsWith('title=')) { $form.Text = $action.Substring(6) }
        elseif ($action -eq 'focus=secret') { [void]$secret.Focus() }
    }
    Write-State
})

$commands = New-Object Windows.Forms.Timer
$commands.Interval = 40
$commands.Add_Tick({
    if (-not (Test-Path -LiteralPath $CommandFile)) { return }
    $line = [IO.File]::ReadAllText($CommandFile, [Text.Encoding]::UTF8)
    Remove-Item -LiteralPath $CommandFile -Force
    if ($line.StartsWith('title=')) { $form.Text = $line.Substring(6) }
    elseif ($line -eq 'focus=secret') { [void]$secret.Focus() }
    elseif ($line -eq 'focus=editor') { [void]$editor.Focus() }
    elseif ($line.StartsWith('thief=')) { Start-Thief $line.Substring(6) }
    elseif ($line.StartsWith('ontype=')) { $script:OnType = $line.Substring(7) }
    $script:CommandsDone++
    Write-State
})
$commands.Start()

# Closes the form when the test run stops touching the heartbeat file (it was killed or has finished), or
# after 15 minutes whatever happens.
$failsafe = New-Object Windows.Forms.Timer
$failsafe.Interval = 2000
$failsafe.Add_Tick({
    $beat = $script:Started
    if (Test-Path -LiteralPath $HeartbeatFile) { $beat = (Get-Item -LiteralPath $HeartbeatFile).LastWriteTime }
    if (((Get-Date) - $beat).TotalSeconds -gt 30 -or ((Get-Date) - $script:Started).TotalMinutes -gt 15) {
        Stop-Thief
        $form.Close()
    }
})
$failsafe.Start()

$form.Add_Shown({ [void]$editor.Focus(); Write-State })
[Windows.Forms.Application]::Run($form)
"""

NAMED_EXE = r"""
using System.Windows.Forms;
public static class NamedExe {
    [System.STAThread]
    public static void Main(string[] args) {
        Form form = new Form();
        form.Text = args[0];
        Timer failsafe = new Timer();
        failsafe.Interval = 30000;
        failsafe.Tick += delegate { form.Close(); };
        failsafe.Start();
        Application.Run(form);
    }
}
"""

# A WPF window in its own PowerShell process (started with -STA): unlike a WinForms control, the managed UI
# Automation client sees its button as ControlType.Button with an Invoke pattern, and its text box as Edit. It has
# the same contract as FIXTURE where the tests need it: a state file (title, pressed count, text), the heartbeat
# rule, and a self-close after HEARTBEAT_LIMIT_SECONDS without a heartbeat.
WPF_FIXTURE = r"""
param([string]$Title, [string]$StateFile, [string]$HeartbeatFile)
Add-Type -AssemblyName PresentationFramework, PresentationCore, WindowsBase

$script:Pressed = 0
$script:Twins = 0
$script:Started = Get-Date

function Write-State {
    $json = @{ title = $w.Title; pressed = $script:Pressed; twins = $script:Twins; text = $t.Text } | ConvertTo-Json -Compress
    $temp = "$StateFile.tmp"
    [IO.File]::WriteAllText($temp, $json)
    for ($try = 0; $try -lt 10; $try++) {
        try { Move-Item -LiteralPath $temp -Destination $StateFile -Force; return } catch { Start-Sleep -Milliseconds 20 }
    }
}

$w = New-Object System.Windows.Window
$w.Title = $Title
$w.Width = 320
$w.Height = 240
$w.Left = 300
$w.Top = 300
$w.Topmost = $true
$p = New-Object System.Windows.Controls.StackPanel
$b = New-Object System.Windows.Controls.Button
$b.Content = 'Press Me'
$b.Height = 30
$t = New-Object System.Windows.Controls.TextBox
$t.Height = 30
[void]$p.Children.Add($b)
[void]$p.Children.Add($t)
# Two sibling buttons with one name: invoke must refuse to choose between them.
foreach ($n in 1..2) {
    $twin = New-Object System.Windows.Controls.Button
    $twin.Content = 'Twin'
    $twin.Height = 30
    $twin.Add_Click({ $script:Twins++; Write-State })
    [void]$p.Children.Add($twin)
}
$w.Content = $p

$b.Add_Click({ $script:Pressed++; Write-State })
$t.Add_TextChanged({ Write-State })
$w.Add_Loaded({ Write-State })

# Closes the window when the test run stops touching the heartbeat file, or after 15 minutes whatever happens.
$failsafe = New-Object System.Windows.Threading.DispatcherTimer
$failsafe.Interval = [TimeSpan]::FromSeconds(2)
$failsafe.Add_Tick({
    $beat = $script:Started
    if (Test-Path -LiteralPath $HeartbeatFile) { $beat = (Get-Item -LiteralPath $HeartbeatFile).LastWriteTime }
    if (((Get-Date) - $beat).TotalSeconds -gt 30 -or ((Get-Date) - $script:Started).TotalMinutes -gt 15) { $w.Close() }
})
$failsafe.Start()
[void]$w.ShowDialog()
"""