"""The daily reminder push: due open loops to his phone.

Runs `af open due --json`, writes the due loops as headings and bullets, and
posts them through notify.py on the REMINDERS channel (credential names: see
notify.py; they live in system/tools/.env). Nothing due: exits 0 without a
word. Each real run appends one line to system/logs/reminders-push.log.
push-due-loops.ps1 is the scheduler's entry point; this file does the work.

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
from pathlib import Path

HERE = Path(__file__).resolve().parent
VAULT = HERE.parent.parent
sys.path.insert(0, str(HERE))

import notify  # noqa: E402

LOG = VAULT / "system" / "logs" / "reminders-push.log"
CHANNEL = "reminders"


def due_loops(root: Path) -> list[dict]:
    env = {**os.environ, "AF_ROOT": os.environ.get("AF_ROOT") or str(root)}
    r = subprocess.run(["af", "open", "due", "--json"], capture_output=True, text=True, encoding="utf-8", env=env, cwd=root)
    if r.returncode != 0:
        raise RuntimeError(f"af open due --json failed (exit {r.returncode}): {r.stderr.strip()}")
    return json.loads(r.stdout or "[]")


def render(loops: list[dict], today: dt.date) -> str:
    """Headings per section, one bullet per loop; no tables (they do not render on a phone)."""
    noun = "loop" if len(loops) == 1 else "loops"
    out = [f"# {len(loops)} {noun} due · {today.isoformat()}"]
    sections: list[str] = []
    for l in loops:
        if l.get("section", "") not in sections:
            sections.append(l.get("section", ""))
    for s in sections:
        out += ["", f"## {s or 'Open loops'}", ""]
        for l in loops:
            if l.get("section", "") != s:
                continue
            bullet = f"- **{l['text']}** · nudged {l.get('nudge')}"
            if l.get("every"):
                bullet += f" · every {l['every']}"
            bullet += f" · open {l.get('age_days', 0)}d"
            out.append(bullet)
    return "\n".join(out) + "\n"


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
    try:
        loops = due_loops(VAULT)
    except (OSError, RuntimeError, ValueError) as e:
        if not a.dry_run:
            log(f"af open due failed: {e}")
        print(e, file=sys.stderr)
        return 1
    if not loops:
        if not a.dry_run:
            log("0 due")
        return 0
    text = render(loops, dt.date.today())
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
