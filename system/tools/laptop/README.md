# The laptop kit

These scripts turn a spare Windows 11 laptop into AgentFrame's always-on machine. Once it is set up, the laptop stays awake on wall power, holds the sync hub the desktop syncs through, runs the scheduled jobs, and keeps one Claude Code session open that you reach from your phone, through Remote Control or a Discord direct message with your bot. The session runs under your own administrator account.

You run every script yourself. The setup script changes system and security settings, so it is yours to run, not an agent's.

## The scripts

| Script | Who runs it, and when | What it does |
|---|---|---|
| `setup-laptop.ps1` | You, once, in an administrator PowerShell | On wall power: never sleep, never hibernate, closing the lid does nothing. Creates the hub folder (default `C:\afhub`) and shares it for your account only. Opens file sharing in the firewall for Private networks only. Prints each change before making it, then a summary with the undo command for each, or the step it stopped at. `-WhatIf` shows the changes without making them. |
| `start-agent.ps1` | Task Scheduler, at each logon, as you (not elevated) | Starts Chrome, runs `af sync`, then opens the session in its own window named "AgentFrame session". The session keeps its own conversation id, so a restart reopens that conversation and never one left by a background job in the same folder. Restarts the session when it stops, and waits ten minutes after five restarts in ten minutes, sending you one message when that happens. Runs `af sync` every 15 minutes. Writes `system\logs\start-agent.log`. |
| `register-agent-logon.ps1` | You, once | Creates the Task Scheduler job `AgentFrame agent`, which runs `start-agent.ps1` at normal priority when you sign in. `-Unregister` removes it. |
| `timers.ps1` | You, once on each machine | `-Register` sets up the loop watch, loop sweep, bills watch, reminders push, weekly consolidation and weekly backlog pass on this machine; `-Unregister` removes them. With no switch it shows which of them are registered here. Only one machine should run them. |

The session's command line sits in one marked block at the top of `start-agent.ps1`. The Discord bridge (`--channels`) is a research preview whose flags may change; when they do, that block is the only place to edit. The permission mode defaults to `auto`, because a permission prompt stops an unattended session until someone answers it. Whether `auto` is accepted is not yet checked: if it is not, the session exits at once and the log shows it. The fix is `-PermissionMode` on `register-agent-logon.ps1`.

The session's conversation id lives in `system\logs\start-agent-session-id.txt`. Each start reopens that conversation when its transcript exists, and otherwise starts it under the same id; a failed start never throws the id away. Delete that file to have the next start open a new conversation.

## Who can reach the hub

Only your account. The share lists your account alone, with full access; the folder allows your account, SYSTEM and Administrators. Other accounts on the laptop get no access, and there is no guest or Everyone access. Another administrator account could still take the folder over, as an administrator can with any folder.

## One hub, two names

The hub is one folder on the laptop. The laptop names it by its local path, `C:\afhub`; the desktop names it by its share path, `\\<laptop-name>\afhub`. This is expected: each machine keeps its own name for the hub in its own git settings, never in a tracked file.

## Order of steps

Placeholders: `<laptop-name>` is the laptop's computer name, `<vault folder>` the folder you choose for the vault on the laptop. If Windows refuses to run a script, first run `Set-ExecutionPolicy -Scope Process Bypass` in that PowerShell window; it lasts only for that window.

### 1. On the laptop, by hand

1. Update Windows 11 and sign in with your administrator account.
2. Install Git, Python 3, Google Chrome, Claude Code and Bun. Check each answers in a new PowerShell window: `git --version`, `python --version`, `claude --version`, `bun --version`. Rust is not needed unless you choose to build `af` on the laptop (step 4).
3. Set your git name and email once, or the laptop's commits are authored "AgentFrame":
   ```powershell
   git config --global user.name "<your name>"
   git config --global user.email "<your email>"
   ```
4. If the laptop maker's own app (often a battery or power manager) offers a charge limit, turn it on. The laptop sits on wall power all year, and a battery held at full charge wears out faster.

### 2. On the laptop: settings and the hub share

The vault is not on the laptop yet, so copy this folder (`system\tools\laptop`) from the desktop to the laptop, for example on a USB stick. In an administrator PowerShell, in that copy:

```powershell
.\setup-laptop.ps1 -WhatIf
.\setup-laptop.ps1
```

Read the `-WhatIf` list first. If it says the network is Public and you are on your home network, rerun with `-SetNetworkPrivate`; if it lists more than one adapter, add `-InterfaceAlias` with the name of the home one (for example `Wi-Fi`). If it stops at the folder permission because other accounts still have entries in an existing hub folder, run the `icacls` command it prints, then rerun. Add `-DisplayOffMinutes 5` to have the screen turn off after five minutes. Keep the summary it prints; it holds the undo commands.

### 3. On the desktop: create the hub

1. In File Explorer, open `\\<laptop-name>\afhub` once and sign in with the laptop account the hub is shared with; tick "Remember my credentials". If that laptop account is a Microsoft account, the user name is its email address and the password is the Microsoft account password, not the PIN. If the name does not resolve, use the laptop's IP address instead, here and in the next command.
2. In the desktop's vault folder, create the hub and push both repositories to it:
   ```powershell
   af sync init \\<laptop-name>\afhub --dry-run
   af sync init \\<laptop-name>\afhub
   ```

### 4. On the laptop: the vault

In a normal (not administrator) PowerShell:

1. Get the public code from the hub, with the remote named `hub`:
   ```powershell
   git clone -o hub C:\afhub\agentframe.git "<vault folder>"
   cd "<vault folder>"
   ```
2. Put `af` on the PATH by copying `af.exe` from the desktop; both are Windows x64. For example, into `.cargo\bin` in your user folder, adding that folder to your user PATH if it is not there yet:
   ```powershell
   $bin = Join-Path $env:USERPROFILE '.cargo\bin'
   New-Item -ItemType Directory -Force -Path $bin | Out-Null
   Copy-Item '<folder holding the copied af.exe>\af.exe' $bin
   $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
   if (($userPath -split ';') -notcontains $bin) { [Environment]::SetEnvironmentVariable('Path', "$userPath;$bin", 'User') }
   ```
   Open a new PowerShell window, so it sees the new PATH, and check that `af --help` prints its list of commands, with `sync` among them.
   - If `af.exe` reports a missing `VCRUNTIME140.dll`, either use the statically linked copy the desktop can build, or install Microsoft's Visual C++ redistributable; your call.
   - The fallback is building it on the laptop with `cargo install --path system/af`, which needs Rust installed; also your call.
   - Both machines must run the same build of `af`. It has no version flag yet, so after the desktop rebuilds it, copy the new `af.exe` to the laptop again.
3. Join the hub; this brings down the private layer (`workspace/`):
   ```powershell
   af sync init C:\afhub
   ```
4. Sync once and check the last line says `Done.`:
   ```powershell
   af sync
   ```
5. Check that `system\tools\.env` now exists in the vault; `af sync` carries it. Copy it by hand from the desktop only if it does not. The reminders push and this kit's status messages need its Discord settings.

From here on, use the kit inside the vault, `<vault folder>\system\tools\laptop`; the USB copy is no longer needed.

### 5. On the laptop: Claude Code and Chrome

1. Run `claude` in the vault folder, accept the folder trust question, and sign in with your claude.ai account (Remote Control needs it).
2. In Chrome, install the Claude extension and sign in to it.

### 6. Sign-in at startup

See the section below.

### 7. Discord

In a normal PowerShell, in the vault folder:

1. Run `claude`, then `/plugin install discord@claude-plugins-official`.
2. Run `/discord:configure` and paste your bot's token when asked. It is saved in your user folder under `.claude\channels\discord\`, never in the vault.
3. Exit, then start `claude --channels plugin:discord@claude-plugins-official`.
4. Send your bot a direct message from your phone and pair as the plugin tells you.
5. Run `/discord:access policy allowlist` so only you can reach it. Then exit.

### 8. First run, by hand

In a normal PowerShell, in the kit folder inside the vault:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\start-agent.ps1
```

Answer any question the "AgentFrame session" window asks. Then check from your phone: the session "AgentFrame laptop" shows in Remote Control, and the bot answers a direct message. Check `system\logs\start-agent.log` shows `sync ok`. Stop it afterwards: close the watchdog window first, then the session window.

### 9. Start at logon

```powershell
.\register-agent-logon.ps1
Start-ScheduledTask -TaskName 'AgentFrame agent'
```

Add `-LockScreen` to the first command if the laptop signs in by itself (next section).

### 10. Timers

On the laptop, `.\timers.ps1 -Register`. Then on the desktop, in its own vault's `system\tools\laptop` folder, `.\timers.ps1 -Unregister`. Run `.\timers.ps1` on each to check: the laptop shows `[here]`, the desktop `[absent]`.

Once the laptop's session works, you may want to stop the desktop's own Remote Control server, so your phone lists one session.

## Sign-in at startup

After a power cut or an update restart, the session starts only once someone signs in. The kit stores no password and does not change this; pick one of these by hand.

- **Windows automatic sign-in.** In Settings, Accounts, Sign-in options, turn off "For improved security, only allow Windows Hello sign-in for Microsoft accounts on this device". While it is on, the option below stays hidden. Then press Win+R, run `netplwiz`, untick "Users must enter a user name and password to use this computer", and enter your password once; Windows keeps it. If the tick box is missing, the Windows Hello setting is still on, or you need to sign out and back in after changing it.
- **Microsoft's Sysinternals Autologon** does the same and keeps the password encrypted.

Also turn on "Use my sign-in info to automatically finish setting up after an update" in the same Sign-in options page.

Either way, anyone who opens the laptop is signed in as you, an administrator. Register the logon job with `-LockScreen` so the screen locks as soon as the session is running; the session keeps working while it is locked.

## Stop and undo

- **Stop the session now:** `Stop-ScheduledTask -TaskName 'AgentFrame agent'`, then close the "AgentFrame session" window. Closing only the session window gets it restarted.
- **Stop it starting at logon:** `.\register-agent-logon.ps1 -Unregister`.
- **Move the timers back to the desktop:** `.\timers.ps1 -Unregister` on the laptop, then `.\timers.ps1 -Register` on the desktop.
- **Undo the settings:** run the undo commands from the setup summary in an administrator PowerShell. `Get-Help .\setup-laptop.ps1 -Full` lists them with common default values. In short: `powercfg /setacvalueindex` back to the old values, `Remove-SmbShare -Name 'afhub' -Force`, `icacls 'C:\afhub' /reset /T /C`, `Remove-NetFirewallRule -Group 'AgentFrame hub'`, and deleting the hub folder once nothing in it is needed.
- **Undo sign-in at startup:** tick the box in `netplwiz` again, or turn Autologon off.
- **Discord:** remove the plugin with `/plugin`, and delete `.claude\channels\discord\.env` in your user folder to remove the token.

## What to watch

`system\logs\start-agent.log` has one line per event: the session starting or ending with its exit code, back-off, each status message sent, and each sync's outcome:

- `sync ok`: healthy.
- `sync conflict`: a chat must resolve the files listed (`library/process/sync.md`). You get one message per new set of conflicts.
- `sync not set up yet`: `af sync init` has not run on this machine. Logged once, no message; the session keeps running.
- `sync failed`: the hub could not be reached, another sync held the lock, or it printed problems. Logged each time; one message after four failures in a row (`-SyncFailuresBeforeNotice` on `start-agent.ps1`). The laptop holds the hub itself, so "unreachable" there points at the hub folder.

Also:

- Discord messages reach the session only while it is open. During a back-off they wait or are lost, so check the log if the bot goes quiet.
- If a Claude Code update changes the `--channels` or `--remote-control` flags, the session will stop at once and back off. Fix the block at the top of `start-agent.ps1`.
- Status messages go to the reminders channel through `system\tools\notify.py`; pass `-NotifyChannel` to `register-agent-logon.ps1` for another.
- A remote-control session you start by hand is left alone and logged once; the watchdog still starts its own. If both hold the Discord channel, both answer the bot, so close one.

## Assumptions not yet checked on the laptop

- `--session-id` and `--resume` combine with `--remote-control`, `--channels`, `--chrome` and `--permission-mode`.
- Claude Code keeps each conversation's transcript as `.claude\projects\<folder>\<id>.jsonl` in your user folder; the watchdog looks there to choose between `--resume` and `--session-id`. If it is kept elsewhere, every start uses `--session-id`, which Claude Code may refuse once the id is in use; the session then exits at once and the log shows it.
- `--permission-mode auto` is accepted on your account.
- The desktop finds the laptop by name through the firewall rules the setup adds; if not, it uses the IP address.
