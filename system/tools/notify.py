"""Post a markdown file to his phone through Discord: the shared notifier.

Standard library only. Credentials come from a git-ignored .env beside this
file (system/tools/.env), or the file --env names, then the process
environment; never from a tracked file. Bot first, if both are set:

    DISCORD_BOT_TOKEN=<the bot's token>
    <CHANNEL>_DISCORD_CHANNEL_ID=<channel id>
or
    <CHANNEL>_DISCORD_WEBHOOK=https://discord.com/api/webhooks/<id>/<token>

<CHANNEL> is --channel in upper case: REMINDERS for the due-loop push,
GROCERIES for the grocery list. Without --channel the names are
DISCORD_CHANNEL_ID and DISCORD_WEBHOOK.

Usage:
    python system/tools/notify.py note.md --channel reminders
    python system/tools/notify.py note.md --title "Thursday list"
    python system/tools/notify.py - --channel reminders < note.md    # read stdin
    python system/tools/notify.py note.md --dry-run                  # print, send nothing
    python system/tools/notify.py - --test --channel reminders       # send one test line, report the result

Discord takes 2,000 characters a message, so a long file goes out as several
messages in order. Markdown tables do not render on a phone; write headings
and bullets.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ENV = HERE / ".env"
LIMIT = 1900


def load_env(path: Path) -> dict[str, str]:
    out: dict[str, str] = {}
    if not path.exists():
        return out
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, v = line.split("=", 1)
        out[k.strip()] = v.strip().strip('"').strip("'")
    return out


def chunks(text: str, limit: int = LIMIT) -> list[str]:
    """Split on paragraph breaks, then lines, so no message cuts a sentence."""
    parts: list[str] = []
    cur = ""
    for para in text.split("\n\n"):
        piece = para if not cur else "\n\n" + para
        if len(cur) + len(piece) <= limit:
            cur += piece
            continue
        if cur:
            parts.append(cur)
            cur = ""
        if len(para) <= limit:
            cur = para
            continue
        for line in para.split("\n"):
            if len(cur) + len(line) + 1 > limit:
                parts.append(cur)
                cur = line
            else:
                cur = line if not cur else cur + "\n" + line
    if cur:
        parts.append(cur)
    return parts


def names(channel: str | None) -> tuple[str, str]:
    """The channel-id and webhook variable names for a channel."""
    prefix = f"{channel.strip().upper()}_" if channel and channel.strip() else ""
    return f"{prefix}DISCORD_CHANNEL_ID", f"{prefix}DISCORD_WEBHOOK"


def target_from_env(env: dict[str, str], channel: str | None = None) -> dict[str, str] | None:
    chan_key, hook_key = names(channel)
    token, chan = env.get("DISCORD_BOT_TOKEN"), env.get(chan_key)
    if token and chan:
        return {"token": token, "channel": chan}
    if env.get(hook_key):
        return {"webhook": env[hook_key]}
    return None


def post(target: dict[str, str], content: str) -> dict:
    """Send one message. Returns the response JSON (the bot path returns the message); a webhook
    answers with an empty body, and the result is then {}."""
    body = json.dumps({"content": content}).encode("utf-8")
    headers = {"Content-Type": "application/json", "User-Agent": "agentframe-notify/1"}
    if target.get("token"):
        url = f"https://discord.com/api/v10/channels/{target['channel']}/messages"
        headers["Authorization"] = f"Bot {target['token']}"
    else:
        url = target["webhook"]
    req = urllib.request.Request(url, data=body, headers=headers)
    with urllib.request.urlopen(req, timeout=30) as resp:
        raw = resp.read()
    try:
        data = json.loads(raw.decode("utf-8")) if raw.strip() else {}
    except ValueError:
        return {}
    return data if isinstance(data, dict) else {}


def read_input(src: str) -> str:
    if src == "-":
        try:
            sys.stdin.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
        except Exception:  # noqa: BLE001
            pass
        return sys.stdin.read()
    return Path(src).read_text(encoding="utf-8")


def compose(text: str, title: str | None) -> list[str]:
    """The messages to send: a bold title (default: the file's first heading), then the body."""
    text = text.strip()
    if title is None and text.startswith("# "):
        title, _, text = text.partition("\n")
        title = title[2:].strip()
    if title:
        text = f"**{title}**\n\n{text.strip()}"
    return chunks(text)


def test_text(now: dt.datetime | None = None) -> str:
    return f"AgentFrame notify test {(now or dt.datetime.now()).strftime('%Y-%m-%d %H:%M')}"


def send_test(a: argparse.Namespace) -> int:
    """--test: one line to --channel, then the delivery result. A missing target names its keys."""
    text = test_text()
    if a.dry_run:
        print(f"--- test message ({len(text)} chars) ---")
        print(text)
        return 0
    target = target_from_env({**os.environ, **load_env(a.env)}, a.channel)
    if not target:
        return missing_target(a)
    try:
        sent = post(target, text)
    except (urllib.error.URLError, TimeoutError, OSError) as e:
        print(f"notify failed: {e}", file=sys.stderr)
        return 2
    if target.get("token"):
        if not isinstance(sent, dict) or not sent.get("id"):
            print("notify failed: the bot's reply carried no message id", file=sys.stderr)
            return 2
        print(f"delivered, message id {sent['id']}")
    else:
        print("delivered")
    return 0


def missing_target(a: argparse.Namespace) -> int:
    chan_key, hook_key = names(a.channel)
    print(f"no DISCORD_BOT_TOKEN + {chan_key}, and no {hook_key}, in {a.env} or the environment; nothing sent.", file=sys.stderr)
    return 2


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("file", nargs="?", help="markdown file to send, or - for stdin (not read with --test)")
    p.add_argument("--title", default=None, help="bold first line; defaults to the file's first heading")
    p.add_argument("--channel", default=None, help="variable-name prefix, e.g. reminders or groceries")
    p.add_argument("--env", type=Path, default=ENV, help=f"credentials file (default {ENV})")
    p.add_argument("--dry-run", action="store_true")
    p.add_argument("--test", action="store_true", help="send one 'AgentFrame notify test <date time>' line to --channel and print the result")
    a = p.parse_args(argv)
    if not a.test and a.file is None:
        p.error("the file argument is required unless --test is given")
    try:
        sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:  # noqa: BLE001
        pass
    if a.test:
        return send_test(a)
    parts = compose(read_input(a.file), a.title)
    if not parts:
        print("nothing to send.", file=sys.stderr)
        return 0
    if a.dry_run:
        for i, part in enumerate(parts, 1):
            print(f"--- message {i} of {len(parts)} ({len(part)} chars) ---")
            print(part)
        return 0
    target = target_from_env({**os.environ, **load_env(a.env)}, a.channel)
    if not target:
        return missing_target(a)
    errors = send_all(target, parts)
    for err in errors:
        print(f"notify failed: {err}", file=sys.stderr)
    print(f"sent {len(parts) - len(errors)} of {len(parts)} message(s) to Discord.")
    return 2 if errors else 0


def send_all(target: dict[str, str], parts: list[str]) -> list[str]:
    """Post each part in order; a failed part (HTTP error, timeout, network) is recorded
    and the rest still go. Returns one line per failure."""
    errors: list[str] = []
    for i, part in enumerate(parts):
        if i:
            time.sleep(0.5)
        try:
            post(target, part)
        except (urllib.error.URLError, TimeoutError, OSError) as e:
            errors.append(f"message {i + 1} of {len(parts)}: {e}")
    return errors


if __name__ == "__main__":
    sys.exit(main())
