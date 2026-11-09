"""The daily reminder push: due open loops to his phone, once a day.

Runs `af open due --json`, writes the due loops as headings and bullets, and
posts them through notify.py on the REMINDERS channel (credential names: see
notify.py; they live in system/tools/.env). Nothing due: exits 0 without a
word. Each real run appends one line to system/logs/reminders-push.log.
push-due-loops.ps1 is the scheduler's entry point; this file does the work.

The loop sweep runs just before and hands over today's overdue list
(workspace/life/systems/loop-sweep-overdue.json, library/schema/loop-sweep.md):
those loops lead the message in his own words and are left out of the
sections below, so each goes out once. A stale or missing handoff changes
nothing. Discord is a third-party service, so every loop passes the
off_discord filter: one that looks sensitive or holds a private term shows
only in a count, and amounts and long numbers are masked. Without his private
terms file the push sends counts only. While the sweep runs (its marker
workspace/life/systems/loop-sweep.running) the push waits, up to 7 minutes;
with no handoff for today it waits up to 45 seconds for one.

Usage:
    python system/tools/push_due_loops.py --dry-run    # print what would be sent
    python system/tools/push_due_loops.py              # send
"""

from __future__ import annotations

import argparse
import contextlib
import datetime as dt
import io
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
VAULT = HERE.parent.parent
sys.path.insert(0, str(HERE))

import notify  # noqa: E402
import off_discord  # noqa: E402

LOG = VAULT / "system" / "logs" / "reminders-push.log"
CHANNEL = "reminders"
HANDOFF = Path("workspace/life/systems/loop-sweep-overdue.json")
OVERDUE = "Overdue, not confirmed"
SWEEP_RUNNING = Path("workspace/life/systems/loop-sweep.running")  # loop_sweep.RUNNING
WAIT_S, POLL_S, STALE_S = 7 * 60, 15, 30 * 60  # the push task stops at 10 minutes
HANDOFF_WAIT_S, HANDOFF_POLL_S = 45, 5          # today's handoff missing: give the sweep a moment to start
AF_TIMEOUT_S = 120


def due_loops(root: Path) -> list[dict]:
    env = {**os.environ, "AF_ROOT": os.environ.get("AF_ROOT") or str(root)}
    try:
        r = subprocess.run(["af", "open", "due", "--json"], capture_output=True, text=True, encoding="utf-8", env=env,
                           cwd=root, timeout=AF_TIMEOUT_S)
    except subprocess.TimeoutExpired:
        raise RuntimeError(f"af open due --json timed out after {AF_TIMEOUT_S}s") from None
    if r.returncode != 0:
        raise RuntimeError(f"af open due --json failed (exit {r.returncode}): {r.stderr.strip()}")
    return json.loads(r.stdout or "[]")


def load_handoff(root: Path, today: dt.date) -> list[dict] | None:
    """Today's overdue list from the loop sweep; None when there is none for today."""
    try:
        data = json.loads((root / HANDOFF).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    if not isinstance(data, dict) or data.get("date") != today.isoformat() or not isinstance(data.get("items"), list):
        return None
    return [x for x in data["items"] if isinstance(x, dict) and isinstance(x.get("text"), str)]


def overdue_section(items: list[dict], terms: list[str] | None) -> tuple[str, int]:
    """The sweep's overdue loops as the message's first section, in his own words, and how many it
    kept off Discord."""
    lines, hidden = [], 0
    for x in items:
        if terms is None or off_discord.is_private((x["text"],), terms):
            hidden += 1
            continue
        lines.append(f"- **{off_discord.mask(x['text'])}** · due {x.get('due')} · {x.get('days')}d over")
    text = "\n".join([f"## {OVERDUE}", "", *lines]) if lines else ""
    return text, hidden


def render(loops: list[dict], today: dt.date, overdue: list[dict] | None = None,
           terms: list[str] | None = None) -> str:
    """Headings per section, one bullet per loop; no tables (they do not render on a phone).
    The sweep's overdue loops come first and once; a loop that must stay off Discord counts.
    Amounts and long numbers are masked. With no private terms file (terms is None) nothing that
    names a loop goes out: only the counts."""
    due_texts = {l["text"] for l in loops}
    overdue = [x for x in overdue or [] if x["text"] in due_texts]
    handed = {x["text"] for x in overdue}
    noun = "loop" if len(loops) == 1 else "loops"
    out = [f"# {len(loops)} {noun} due · {today.isoformat()}"]
    if terms is None:
        over = f", {len(overdue)} overdue and not confirmed" if overdue else ""
        return "\n".join(out + ["", f"- {len(loops)} due{over}; open AgentFrame to see them.",
                                f"- Loop text is held back until {off_discord.TERMS_FILE.as_posix()} exists."]) + "\n"
    first, hidden = overdue_section(overdue, terms)
    if first:
        out += ["", first]
    rest = [l for l in loops if l["text"] not in handed]
    shown = [l for l in rest if not off_discord.is_private((l["text"],), terms)]
    hidden += len(rest) - len(shown)
    sections: list[str] = []
    for l in shown:
        if l.get("section", "") not in sections:
            sections.append(l.get("section", ""))
    for s in sections:
        out += ["", f"## {s or 'Open loops'}", ""]
        for l in shown:
            if l.get("section", "") != s:
                continue
            bullet = f"- **{off_discord.mask(l['text'])}** · nudged {l.get('nudge')}"
            if l.get("every"):
                bullet += f" · every {l['every']}"
            bullet += f" · open {l.get('age_days', 0)}d"
            out.append(bullet)
    if hidden:
        out += ["", f"- {hidden} more kept off Discord; see Do in AgentFrame"]
    return "\n".join(out) + "\n"


def wait_for_sweep(root: Path, *, now=time.time, sleep=time.sleep) -> bool:
    """Wait while the loop sweep runs (both can start together at logon), so the push carries
    today's handoff and the sweep's changes. A marker older than the sweep's 30-minute limit is
    a crashed run and is ignored; after WAIT_S the push goes anyway. True when it waited it out."""
    marker = root / SWEEP_RUNNING
    deadline = now() + WAIT_S
    while True:
        try:
            age = now() - marker.stat().st_mtime
        except OSError:
            return True
        if age > STALE_S or now() >= deadline:
            return False
        sleep(POLL_S)


def wait_for_handoff(root: Path, today: dt.date, *, now=time.time, sleep=time.sleep) -> list[dict] | None:
    """Today's handoff, waiting up to HANDOFF_WAIT_S when it is missing: at logon the push can
    start a moment before the sweep has written its marker. If the marker appears meanwhile,
    wait for the sweep as usual. None when no handoff came; the push then goes without it."""
    deadline = now() + HANDOFF_WAIT_S
    while True:
        items = load_handoff(root, today)
        if items is not None:
            return items
        if (root / SWEEP_RUNNING).exists():
            wait_for_sweep(root, now=now, sleep=sleep)
            return load_handoff(root, today)
        if now() >= deadline:
            return None
        sleep(HANDOFF_POLL_S)


def log(line: str) -> None:
    LOG.parent.mkdir(parents=True, exist_ok=True)
    with LOG.open("a", encoding="utf-8") as f:
        f.write(f"- {dt.datetime.now():%Y-%m-%d %H:%M} · {line}\n")


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--dry-run", action="store_true", help="print what would be sent; send and log nothing")
    a = p.parse_args(argv)
    try:
        sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:  # noqa: BLE001
        pass
    root = Path(os.environ.get("AF_ROOT") or VAULT)
    today = dt.date.today()
    if a.dry_run:
        handoff = load_handoff(root, today)
    else:
        wait_for_sweep(root)
        handoff = wait_for_handoff(root, today)
    try:
        loops = due_loops(root)
    except (OSError, RuntimeError, ValueError) as e:
        if not a.dry_run:
            log(f"af open due failed: {e}")
        print(e, file=sys.stderr)
        return 1
    if not loops:
        if not a.dry_run:
            log("0 due")
        return 0
    text = render(loops, today, handoff, off_discord.private_terms(root))
    with tempfile.NamedTemporaryFile("w", encoding="utf-8", suffix=".md", delete=False) as f:
        f.write(text)
        tmp = f.name
    err = io.StringIO()
    try:
        args = [tmp, "--channel", CHANNEL] + (["--dry-run"] if a.dry_run else [])
        with contextlib.redirect_stderr(err):
            code = notify.main(args)
    except Exception as e:  # noqa: BLE001 - a scheduled run logs, never tracebacks silently
        print(f"notify crashed: {e}", file=err)
        code = 2
    finally:
        os.unlink(tmp)
    failures = [l for l in err.getvalue().splitlines() if l.strip()]
    for line in failures:
        print(line, file=sys.stderr)
    if not a.dry_run:
        detail = "; ".join(failures)
        log(f"{len(loops)} due · " + ("sent" if code == 0 else f"notify exit {code}: {detail}"))
    return code


if __name__ == "__main__":
    sys.exit(main())
