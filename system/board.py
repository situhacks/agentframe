#!/usr/bin/env python3
"""AgentFrame board — the cross-project index of work in flight.

The board is one Markdown file, ``workspace/board.md``, with one heading per
lane and one line per card. It is orchestrator machinery first and a visual
second: the Workspace Dashboard's Board tab is a read of this file. Only this
module writes it. Every transition here is deterministic and spends no model
tokens; the judgment (what to brief, what to dispatch, what to approve) stays
with the operator and the orchestrator session.

Grammar (schema_version 1)::

    ---
    board: agentframe
    schema_version: 1
    stale_after: 3d
    auto_close_after: 7d
    ---

    ## Queued

    - [ ] T-2026-09-09-01 · joyce-hair-pilot · booking-page-copy · @joyce-hair-pilot/booking-page-copy · by: orchestrator · since: 2026-09-09 · brief: tasks/T-2026-09-09-01.task.md

Four positional tokens separated by `` · ``: card id, project slug, deliverable
slug, owner session name prefixed with ``@``. Then only known ``key: value``
tokens. ``- [x]`` appears only in ``Done`` and means closed; ``- [ ]`` in
``Done`` means closing.

Motion invariant: a card is either moving or waiting on the operator, and the
only lane where a card waits on the operator is ``Needs you``. ``sync`` converts
anything that stopped moving into a ``Needs you`` card with a reason, and after
``auto_close_after`` in that lane the card leaves the board to the monthly
archive as ``dropped``. Nothing is deleted.
"""

from __future__ import annotations

import datetime as dt
import glob
import json
import os
import re
import shutil
import subprocess
from dataclasses import dataclass, field
from pathlib import Path


SCHEMA_VERSION = 1
LANES = ("Queued", "In progress", "Needs you", "Done")
FIELD_KEYS = ("by", "since", "ask", "reason", "session", "receipt", "brief", "note", "waiting_since")
ARCHIVE_KEYS = ("outcome", "closed_at")
BY_VALUES = ("orchestrator", "human")
ASK_VALUES = ("review", "input")
OUTCOMES = ("closed", "dropped")
RECEIPT_STATUSES = ("done", "blocked", "failed")
DEFAULT_META = {"board": "agentframe", "schema_version": str(SCHEMA_VERSION),
                "stale_after": "3d", "auto_close_after": "7d", "worker_model": "sonnet"}
BIND_FILE = "orchestrator.json"
SEP = " · "
ID_RE = re.compile(r"^T-(\d{4}-\d{2}-\d{2})-(\d{2,})$")
SLUG_RE = re.compile(r"^[a-z0-9][a-z0-9._-]*$")
WORKING_STATES = {"working", "busy", "generating", "running"}
NEEDS_INPUT_STATES = {"needs input", "needs_input", "waiting"}


class BoardError(Exception):
    """A refused transition or an invalid board."""


# ---------------------------------------------------------------- time helpers

def now_local() -> dt.datetime:
    return dt.datetime.now().astimezone()


def parse_duration(value: str | None) -> dt.timedelta | None:
    """``7d`` / ``12h`` / ``30m`` → timedelta; ``off``/empty → None."""
    text = (value or "").strip().lower()
    if text in ("", "off", "none", "never"):
        return None
    m = re.match(r"^(\d+)\s*([dhm])$", text)
    if not m:
        raise BoardError(f"invalid duration '{value}' (use e.g. 7d, 12h, 30m, or off)")
    n, unit = int(m.group(1)), m.group(2)
    if unit == "d":
        return dt.timedelta(days=n)
    if unit == "h":
        return dt.timedelta(hours=n)
    return dt.timedelta(minutes=n)


def _parse_when(value: str | None) -> dt.datetime | None:
    if not value:
        return None
    text = value.strip()
    try:
        if len(text) == 10:
            parsed = dt.datetime.fromisoformat(text + "T00:00:00")
        else:
            parsed = dt.datetime.fromisoformat(text.replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=now_local().tzinfo)
    return parsed


def _days(delta: dt.timedelta) -> int:
    return max(0, int(delta.total_seconds() // 86400))


# ---------------------------------------------------------------- data model

@dataclass
class Card:
    id: str
    project: str
    deliverable: str
    owner: str
    lane: str
    done: bool = False
    fields: dict = field(default_factory=dict)

    def line(self, extra: dict | None = None) -> str:
        toks = [self.id, self.project, self.deliverable, f"@{self.owner}"]
        for key in FIELD_KEYS:
            value = self.fields.get(key)
            if value not in (None, ""):
                toks.append(f"{key}: {value}")
        for key, value in (extra or {}).items():
            toks.append(f"{key}: {value}")
        return f"- [{'x' if self.done else ' '}] " + SEP.join(toks)

    def state_label(self) -> str:
        if self.lane == "Done":
            return "closed" if self.done else "closing"
        return ""


@dataclass
class Board:
    meta: dict
    lanes: dict

    def cards(self) -> list[Card]:
        return [c for lane in LANES for c in self.lanes.get(lane, [])]

    def find(self, card_id: str) -> Card | None:
        for card in self.cards():
            if card.id == card_id:
                return card
        return None

    def move(self, card: Card, lane: str, *, done: bool = False) -> None:
        if lane not in LANES:
            raise BoardError(f"unknown lane '{lane}'")
        self.lanes[card.lane] = [c for c in self.lanes.get(card.lane, []) if c is not card]
        card.lane = lane
        card.done = done
        self.lanes.setdefault(lane, []).append(card)

    def remove(self, card: Card) -> None:
        self.lanes[card.lane] = [c for c in self.lanes.get(card.lane, []) if c is not card]

    def stale_after(self) -> dt.timedelta | None:
        return parse_duration(self.meta.get("stale_after", DEFAULT_META["stale_after"]))

    def auto_close_after(self) -> dt.timedelta | None:
        return parse_duration(self.meta.get("auto_close_after", DEFAULT_META["auto_close_after"]))


def empty_board() -> Board:
    return Board(meta=dict(DEFAULT_META), lanes={lane: [] for lane in LANES})


# ---------------------------------------------------------------- grammar

def parse_card_line(line: str, lane: str, allowed_keys=FIELD_KEYS + ARCHIVE_KEYS) -> tuple[Card | None, str | None]:
    m = re.match(r"^- \[( |x)\] (.+)$", line.rstrip())
    if not m:
        return None, f"malformed card line: {line.strip()[:80]}"
    toks = [t.strip() for t in m.group(2).split("·")]
    toks = [t for t in toks if t]
    if len(toks) < 4 or not toks[3].startswith("@"):
        return None, f"card needs id · project · deliverable · @owner: {line.strip()[:80]}"
    card = Card(id=toks[0], project=toks[1], deliverable=toks[2], owner=toks[3][1:], lane=lane,
                done=(m.group(1) == "x"))
    if not ID_RE.match(card.id):
        return None, f"{card.id}: bad card id (T-YYYY-MM-DD-NN)"
    for tok in toks[4:]:
        key, sep, value = tok.partition(":")
        key = key.strip()
        if not sep or key not in allowed_keys:
            return None, f"{card.id}: unknown token '{tok}'"
        card.fields[key] = value.strip()
    return card, None


def parse(text: str) -> tuple[Board, list[str]]:
    """Parse board.md. Returns (board, errors); errors never abort parsing."""
    board = empty_board()
    errors: list[str] = []
    body = text
    if text.startswith("---\n"):
        end = text.find("\n---\n", 4)
        if end >= 0:
            for raw in text[4:end].splitlines():
                key, sep, value = raw.partition(":")
                if sep:
                    board.meta[key.strip()] = value.strip()
            body = text[end + 5:]
    lane = None
    for raw in body.splitlines():
        line = raw.rstrip()
        if line.startswith("## "):
            lane = line[3:].strip()
            if lane not in LANES:
                errors.append(f"unknown lane '{lane}'")
                lane = None
            continue
        if not line.startswith("- [") or lane is None:
            continue
        card, err = parse_card_line(line, lane, FIELD_KEYS)
        if err:
            errors.append(err)
            continue
        if card.done and lane != "Done":
            errors.append(f"{card.id}: [x] outside Done")
        if lane == "Needs you" and card.fields.get("ask") not in ASK_VALUES:
            errors.append(f"{card.id}: Needs you card without ask: review|input")
        if card.fields.get("by") not in (None, *BY_VALUES):
            errors.append(f"{card.id}: by must be orchestrator|human")
        if board.find(card.id):
            errors.append(f"{card.id}: duplicate card id")
        board.lanes[lane].append(card)
    try:
        board.stale_after(); board.auto_close_after()
    except BoardError as exc:
        errors.append(str(exc))
    return board, errors


def render(board: Board) -> str:
    out = ["---"]
    for key in DEFAULT_META:
        out.append(f"{key}: {board.meta.get(key, DEFAULT_META[key])}")
    for key, value in board.meta.items():
        if key not in DEFAULT_META:
            out.append(f"{key}: {value}")
    out.append("---")
    for lane in LANES:
        out.append("")
        out.append(f"## {lane}")
        cards = board.lanes.get(lane, [])
        if cards:
            out.append("")
            out.extend(card.line() for card in cards)
    out.append("")
    return "\n".join(out)


# ---------------------------------------------------------------- paths + io

def paths(root: str | Path) -> dict:
    root = Path(root)
    base = root / "workspace" / "board"
    return {
        "root": root,
        "board": root / "workspace" / "board.md",
        "dir": base,
        "tasks": base / "tasks",
        "receipts": base / "receipts",
        "applied": base / "receipts" / "applied",
        "archive": base / "archive",
        "bind": base / BIND_FILE,
    }


def exists(root: str | Path) -> bool:
    return paths(root)["board"].is_file()


def init(root: str | Path) -> Board:
    p = paths(root)
    for key in ("tasks", "receipts", "applied", "archive"):
        p[key].mkdir(parents=True, exist_ok=True)
    if not p["board"].is_file():
        save(root, empty_board())
    return load(root)


def load(root: str | Path, *, strict: bool = True) -> Board:
    p = paths(root)
    if not p["board"].is_file():
        raise BoardError("no board yet: run 'af board init'")
    board, errors = parse(p["board"].read_text(encoding="utf-8-sig"))
    if errors and strict:
        raise BoardError("board.md grammar errors:\n  - " + "\n  - ".join(errors))
    return board


def save(root: str | Path, board: Board) -> None:
    p = paths(root)
    p["board"].parent.mkdir(parents=True, exist_ok=True)
    tmp = p["board"].with_suffix(".md.tmp")
    tmp.write_text(render(board), encoding="utf-8")
    os.replace(tmp, p["board"])


def _rel(p: dict, path: Path) -> str:
    return path.relative_to(p["dir"]).as_posix()


# ---------------------------------------------------------------- orchestrator binding

BIND_RE = re.compile(r"^[a-z][a-z0-9-]*:[A-Za-z0-9._-]{6,}$")


def bind(root: str | Path, key: str, *, now: dt.datetime | None = None) -> dict:
    """Bind the board to one orchestrator session (``<harness>:<session-id>``).

    The bound session is the one whose writes the hook confines to workspace/board/.
    This is a routing fact, not authority: it grants nothing the operator has not said.
    """
    key = (key or "").strip()
    if not BIND_RE.match(key):
        raise BoardError("session key must look like <harness>:<session-id>, as printed at session start")
    p = paths(root)
    p["dir"].mkdir(parents=True, exist_ok=True)
    record = {"session": key, "bound_at": (now or now_local()).replace(microsecond=0).isoformat()}
    p["bind"].write_text(json.dumps(record, indent=2), encoding="utf-8")
    return record


def unbind(root: str | Path) -> bool:
    p = paths(root)["bind"]
    if p.exists():
        p.unlink()
        return True
    return False


def bound_session(root: str | Path) -> str | None:
    p = paths(root)["bind"]
    if not p.is_file():
        return None
    try:
        data = json.loads(p.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return None
    key = data.get("session") if isinstance(data, dict) else None
    return key if isinstance(key, str) and BIND_RE.match(key) else None


def session_is_bound(root: str | Path, session_id: str | None) -> bool:
    key = bound_session(root)
    if not key or not session_id:
        return False
    return key.split(":", 1)[1] == str(session_id)


def write_allowed(root: str | Path, target: str | Path) -> tuple[bool, str]:
    """Whether a bound orchestrator session may write ``target``: only workspace/board/."""
    root = Path(root).resolve()
    path = Path(target)
    if not path.is_absolute():
        path = root / path
    try:
        rel = path.resolve().relative_to(root)
    except (OSError, ValueError):
        return False, f"writes outside the vault are refused ({target})"
    if rel.as_posix().startswith("workspace/board"):
        return True, ""
    return False, (f"this session is bound to the board as the orchestrator and writes only under workspace/board/ "
                   f"(briefs). Make a card for a worker instead of editing {rel.as_posix()}.")


# ---------------------------------------------------------------- ids + cards

def next_id(root: str | Path, board: Board, day: dt.date) -> str:
    stamp = day.isoformat()
    taken = 0
    for card in board.cards():
        m = ID_RE.match(card.id)
        if m and m.group(1) == stamp:
            taken = max(taken, int(m.group(2)))
    for line in _archive_lines(root):
        m = re.match(r"^- \[.\] (T-(\d{4}-\d{2}-\d{2})-(\d+))", line)
        if m and m.group(2) == stamp:
            taken = max(taken, int(m.group(3)))
    return f"T-{stamp}-{taken + 1:02d}"


def _check_slug(value: str, what: str) -> str:
    if not SLUG_RE.match(value or ""):
        raise BoardError(f"{what} must be a slug (lowercase letters, digits, . _ -): '{value}'")
    return value


def add(root: str | Path, board: Board, *, project: str, deliverable: str, by: str = "orchestrator",
        owner: str | None = None, goal: str = "", done_when: str = "", note: str = "",
        now: dt.datetime | None = None, write_brief: bool = True) -> Card:
    now = now or now_local()
    if by not in BY_VALUES:
        raise BoardError("by must be orchestrator|human")
    _check_slug(project, "project"); _check_slug(deliverable, "deliverable")
    card = Card(id=next_id(root, board, now.date()), project=project, deliverable=deliverable,
                owner=owner or f"{project}/{deliverable}", lane="Queued",
                fields={"by": by, "since": now.date().isoformat()})
    if note:
        card.fields["note"] = _clean(note)
    if write_brief:
        p = paths(root)
        p["tasks"].mkdir(parents=True, exist_ok=True)
        brief = p["tasks"] / f"{card.id}.task.md"
        if not brief.exists():
            brief.write_text(brief_scaffold(root, card, goal=goal, done_when=done_when), encoding="utf-8")
        card.fields["brief"] = _rel(p, brief)
    board.lanes["Queued"].append(card)
    return card


def _clean(value: str) -> str:
    """Field values never contain the card separator or newlines."""
    return " ".join(str(value).replace("·", "-").split())


def brief_scaffold(root: str | Path, card: Card, *, goal: str = "", done_when: str = "") -> str:
    p = paths(root)
    receipt = (p["receipts"] / f"{card.id}.result.json")
    return f"""---
id: {card.id}
project: {card.project}
deliverable: {card.deliverable}
goal: {goal or "(one sentence)"}
done_when: {done_when or "(observable evidence)"}
allowed_paths:
  - workspace/projects/{card.project}/
receipt: {receipt.relative_to(p["root"]).as_posix()}
owner_session: {card.owner}
by: {card.fields.get("by", "orchestrator")}
---

## Brief

(The operator's verbatim ask for this slice, plus the orchestrator's clarifications.)

## Rules

- Work in this checkout directly. Never create or enter a git worktree: the vault's project and board files are gitignored and do not exist in a worktree.
- Write only inside allowed_paths. Create folders under them as needed.
- Do not ask questions. If something blocks you, write the receipt with status blocked and say what you need in operator_action.

## Receipt

When the work is complete, or when you are blocked, write the receipt path named above as one JSON object:

{{"schema_version": 1, "task_id": "{card.id}", "session": "{card.owner}", "status": "done | blocked | failed", "summary": "one useful paragraph", "outputs": ["workspace-relative/path"], "operator_action": null}}

Never claim done before the verification in done_when passes.
"""


# ---------------------------------------------------------------- roster

def claude_bin() -> str | None:
    env = os.environ.get("AGENTFRAME_CLAUDE_BIN")
    if env and Path(env).exists():
        return env
    found = shutil.which("claude")
    if found:
        return found
    for name in ("claude.exe", "claude", "claude.cmd"):
        candidate = Path.home() / ".local" / "bin" / name
        if candidate.exists():
            return str(candidate)
    return None


def roster(root: str | Path, *, timeout: float = 10.0) -> list[dict] | None:
    """Live sessions in this workspace from ``claude agents --json --all``; None if unavailable."""
    exe = claude_bin()
    if not exe:
        return None
    try:
        proc = subprocess.run([exe, "agents", "--json", "--all"], capture_output=True, text=True,
                              timeout=timeout, cwd=str(root))
        rows = json.loads(proc.stdout or "[]")
    except (OSError, subprocess.SubprocessError, ValueError):
        return None
    if not isinstance(rows, list):
        return None
    want = os.path.normcase(os.path.normpath(str(root)))
    return [r for r in rows if isinstance(r, dict)
            and os.path.normcase(os.path.normpath(str(r.get("cwd", "")))) == want]


def roster_row(rows: list[dict] | None, card: Card) -> dict | None:
    if not rows:
        return None
    for row in rows:
        if row.get("name") == card.owner:
            return row
    prefix = card.fields.get("session")
    if prefix:
        for row in rows:
            sid = str(row.get("sessionId") or "")
            if sid.startswith(prefix) or row.get("id") == prefix:
                return row
    return None


def row_state(row: dict | None) -> str:
    if row is None:
        return "absent"
    raw = str(row.get("state") or row.get("status") or "").strip().lower()
    if raw in WORKING_STATES:
        return "working"
    if raw in NEEDS_INPUT_STATES:
        return "needs input"
    if raw in ("", "idle", "completed", "stopped", "failed", "exited", "offline"):
        return raw or "idle"
    return raw


def transcript_mtime(session_prefix: str | None, *, home: str | Path | None = None) -> dt.datetime | None:
    if not session_prefix:
        return None
    base = Path(home) if home else Path(os.environ.get("CLAUDE_CONFIG_DIR") or (Path.home() / ".claude"))
    newest = None
    for path in glob.glob(str(base / "projects" / "*" / f"{session_prefix}*.jsonl")):
        try:
            stamp = dt.datetime.fromtimestamp(os.path.getmtime(path)).astimezone()
        except OSError:
            continue
        if newest is None or stamp > newest:
            newest = stamp
    return newest


def last_activity(card: Card, row: dict | None, now: dt.datetime, *, home=None) -> dt.datetime:
    if row_state(row) == "working":
        return now
    prefix = card.fields.get("session") or (str(row.get("sessionId") or "")[:8] if row else None)
    stamp = transcript_mtime(prefix, home=home)
    if stamp:
        return stamp
    return _parse_when(card.fields.get("waiting_since")) or _parse_when(card.fields.get("since")) or now


# ---------------------------------------------------------------- transitions

def _require_lane(card: Card, *lanes: str) -> None:
    if card.lane not in lanes:
        raise BoardError(f"{card.id} is in '{card.lane}', expected {' or '.join(lanes)}")


def _enter_needs_you(card: Card, board: Board, *, ask: str, reason: str, now: dt.datetime) -> None:
    card.fields["ask"] = ask
    card.fields["reason"] = _clean(reason)[:160]
    card.fields["waiting_since"] = now.replace(microsecond=0).isoformat()
    board.move(card, "Needs you")


def _leave_needs_you(card: Card) -> None:
    for key in ("ask", "reason", "waiting_since"):
        card.fields.pop(key, None)


def dispatch(board: Board, card_id: str, *, session: str | None = None, rows: list[dict] | None = None,
             force: bool = False) -> Card:
    card = board.find(card_id)
    if not card:
        raise BoardError(f"no card {card_id}")
    _require_lane(card, "Queued")
    if not force:
        for other in board.lanes["In progress"]:
            if other.project == card.project:
                raise BoardError(f"{card.project} already has {other.id} in progress (one worker per project; --force to override)")
    if session:
        card.fields["session"] = session[:8]
    else:
        row = roster_row(rows, card)
        if row and row.get("sessionId"):
            card.fields["session"] = str(row["sessionId"])[:8]
    board.move(card, "In progress")
    return card


def approve(board: Board, card_id: str) -> Card:
    card = board.find(card_id)
    if not card:
        raise BoardError(f"no card {card_id}")
    _require_lane(card, "Needs you")
    if card.fields.get("ask") != "review":
        raise BoardError(f"{card.id} is waiting for input, not review; answer in the chat or 'af board resume'")
    _leave_needs_you(card)
    board.move(card, "Done", done=True)
    return card


def return_(board: Board, card_id: str, feedback: str, root: str | Path | None = None) -> Card:
    card = board.find(card_id)
    if not card:
        raise BoardError(f"no card {card_id}")
    _require_lane(card, "Needs you")
    _leave_needs_you(card)
    card.fields["note"] = _clean(f"returned: {feedback}")[:160]
    board.move(card, "In progress")
    if root and card.fields.get("brief"):
        brief = paths(root)["dir"] / card.fields["brief"]
        if brief.is_file():
            with brief.open("a", encoding="utf-8") as fh:
                fh.write(f"\n## Follow-up ({now_local().date().isoformat()})\n\n{feedback.strip()}\n")
    return card


def resume(board: Board, card_id: str) -> Card:
    card = board.find(card_id)
    if not card:
        raise BoardError(f"no card {card_id}")
    _require_lane(card, "Needs you")
    _leave_needs_you(card)
    board.move(card, "In progress")
    return card


def close(board: Board, card_id: str) -> Card:
    card = board.find(card_id)
    if not card:
        raise BoardError(f"no card {card_id}")
    _leave_needs_you(card)
    board.move(card, "Done", done=True)
    return card


def drop(root: str | Path, board: Board, card_id: str, *, now: dt.datetime | None = None, note: str = "") -> Card:
    card = board.find(card_id)
    if not card:
        raise BoardError(f"no card {card_id}")
    if note:
        card.fields["note"] = _clean(note)[:160]
    archive_card(root, board, card, outcome="dropped", now=now or now_local())
    return card


# ---------------------------------------------------------------- receipts

def read_receipt(path: Path) -> dict | None:
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, ValueError):
        return None
    if not isinstance(data, dict) or data.get("status") not in RECEIPT_STATUSES:
        return None
    return data


def write_receipt(root: str | Path, card_id: str, *, session: str, status: str, summary: str,
                  outputs: list[str] | None = None, operator_action: str | None = None,
                  auto: str | None = None) -> Path:
    if status not in RECEIPT_STATUSES:
        raise BoardError("receipt status must be done|blocked|failed")
    p = paths(root)
    p["receipts"].mkdir(parents=True, exist_ok=True)
    path = p["receipts"] / f"{card_id}.result.json"
    payload = {"schema_version": 1, "task_id": card_id, "session": session, "status": status,
               "summary": summary, "outputs": outputs or [], "operator_action": operator_action}
    if auto:
        payload["auto"] = auto
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return path


def _apply_receipt(root, board: Board, card: Card, path: Path, data: dict, now: dt.datetime) -> str:
    p = paths(root)
    p["applied"].mkdir(parents=True, exist_ok=True)
    stamp = now.strftime("%Y%m%dT%H%M%S")
    applied = p["applied"] / f"{card.id}.{stamp}.result.json"
    os.replace(path, applied)
    card.fields["receipt"] = _rel(p, applied)
    if data.get("session") and not card.fields.get("session"):
        card.fields["session"] = str(data["session"])[:8]
    summary = str(data.get("summary") or "").strip()
    if data["status"] == "done":
        _enter_needs_you(card, board, ask="review", reason=summary or "done, review the outputs", now=now)
        return f"{card.id}: done receipt -> Needs you (review)"
    action = str(data.get("operator_action") or "").strip()
    _enter_needs_you(card, board, ask="input", reason=f"{data['status']}: {action or summary}", now=now)
    return f"{card.id}: {data['status']} receipt -> Needs you (input)"


# ---------------------------------------------------------------- archive

def _archive_files(root) -> list[Path]:
    return sorted(paths(root)["archive"].glob("*.md"))


def _archive_lines(root) -> list[str]:
    lines: list[str] = []
    for path in _archive_files(root):
        try:
            lines.extend(path.read_text(encoding="utf-8-sig").splitlines())
        except OSError:
            continue
    return lines


def archive_card(root: str | Path, board: Board, card: Card, *, outcome: str, now: dt.datetime) -> Path:
    if outcome not in OUTCOMES:
        raise BoardError("outcome must be closed|dropped")
    p = paths(root)
    month_dir = p["archive"] / now.strftime("%Y-%m")
    month_file = p["archive"] / f"{now.strftime('%Y-%m')}.md"
    month_dir.mkdir(parents=True, exist_ok=True)
    for key in ("brief", "receipt"):
        rel = card.fields.get(key)
        if not rel:
            continue
        src = p["dir"] / rel
        if src.is_file():
            dst = month_dir / src.name
            os.replace(src, dst)
            card.fields[key] = _rel(p, dst)
    board.remove(card)
    card.lane = "Done"
    card.done = True
    extra = {"outcome": outcome, "closed_at": now.replace(microsecond=0).isoformat()}
    header = f"# Board archive {now.strftime('%Y-%m')}\n\n"
    if not month_file.exists():
        month_file.write_text(header, encoding="utf-8")
    with month_file.open("a", encoding="utf-8") as fh:
        fh.write(card.line(extra) + "\n")
    return month_file


def archive_cards(root: str | Path, *, months: int = 2, now: dt.datetime | None = None) -> list[dict]:
    """Closed and dropped cards from the most recent archive months, newest first."""
    now = now or now_local()
    files = _archive_files(root)[-months:] if months else _archive_files(root)
    out: list[dict] = []
    for path in files:
        try:
            lines = path.read_text(encoding="utf-8-sig").splitlines()
        except OSError:
            continue
        for line in lines:
            if not line.startswith("- ["):
                continue
            card, err = parse_card_line(line, "Done")
            if err:
                continue
            entry = {"id": card.id, "project": card.project, "deliverable": card.deliverable, "owner": card.owner,
                     "outcome": card.fields.get("outcome", "closed"), "closed_at": card.fields.get("closed_at"),
                     "month": path.stem}
            for key in ("by", "since", "note", "receipt", "brief", "session"):
                if key in card.fields:
                    entry[key] = card.fields[key]
            out.append(entry)
    out.sort(key=lambda e: e.get("closed_at") or "", reverse=True)
    return out


def reopen(root: str | Path, board: Board, card_id: str, *, now: dt.datetime | None = None) -> Card:
    now = now or now_local()
    p = paths(root)
    for path in _archive_files(root):
        lines = path.read_text(encoding="utf-8-sig").splitlines()
        for idx, line in enumerate(lines):
            if not line.startswith("- [") or f" {card_id} " not in f" {line[6:].split(SEP)[0].strip()} ":
                continue
            card, err = parse_card_line(line, "Done")
            if err:
                raise BoardError(err)
            if board.find(card.id):
                raise BoardError(f"{card.id} is already on the board")
            del lines[idx]
            path.write_text("\n".join(lines).rstrip("\n") + "\n", encoding="utf-8")
            for key in ARCHIVE_KEYS + ("receipt", "ask", "reason", "waiting_since"):
                card.fields.pop(key, None)
            rel = card.fields.get("brief")
            if rel:
                src = p["dir"] / rel
                if src.is_file():
                    p["tasks"].mkdir(parents=True, exist_ok=True)
                    dst = p["tasks"] / src.name
                    os.replace(src, dst)
                    card.fields["brief"] = _rel(p, dst)
            card.fields["note"] = _clean(f"reopened {now.date().isoformat()}")
            card.lane, card.done = "Queued", False
            board.lanes["Queued"].append(card)
            return card
    raise BoardError(f"{card_id} not found in the archive")


# ---------------------------------------------------------------- sync (deterministic maintenance)

def sync(root: str | Path, board: Board, *, now: dt.datetime | None = None, rows: list[dict] | None = None,
         roster_available: bool | None = None, home=None) -> list[str]:
    """Apply receipts, the staleness rules, auto-drop, and the closed-card sweep. Returns event lines."""
    now = now or now_local()
    if roster_available is None:
        roster_available = rows is not None
    p = paths(root)
    events: list[str] = []
    stale_after = board.stale_after()
    auto_close = board.auto_close_after()

    # 1. receipts land -> Needs you
    if p["receipts"].is_dir():
        for path in sorted(p["receipts"].glob("*.result.json")):
            card_id = path.name[: -len(".result.json")]
            card = board.find(card_id)
            data = read_receipt(path)
            if not card or card.lane == "Done":
                continue
            if not data:
                events.append(f"{card_id}: unreadable receipt left in place")
                continue
            events.append(_apply_receipt(root, board, card, path, data, now))

    # 2. In progress: a live working session, or it is not in progress
    for card in list(board.lanes["In progress"]):
        row = roster_row(rows, card)
        state = row_state(row)
        if state == "working":
            continue
        if state == "needs input":
            _enter_needs_you(card, board, ask="input", reason="worker is waiting on a prompt", now=now)
            events.append(f"{card.id}: worker needs input -> Needs you")
            continue
        by = card.fields.get("by", "orchestrator")
        if by == "orchestrator" and roster_available:
            why = "worker session gone with no receipt" if state == "absent" else f"worker {state} with no receipt"
            _enter_needs_you(card, board, ask="input", reason=f"{why}. resume, close, or drop?", now=now)
            events.append(f"{card.id}: {why} -> Needs you")
            continue
        if stale_after is None:
            continue
        idle = now - last_activity(card, row, now, home=home)
        if idle > stale_after:
            _enter_needs_you(card, board, ask="input",
                             reason=f"idle {_days(idle)}d with no receipt. resume, close, or drop?", now=now)
            events.append(f"{card.id}: idle {_days(idle)}d -> Needs you")

    # 3. Needs you: auto-return when the worker is working again; auto-drop after the deadline
    for card in list(board.lanes["Needs you"]):
        row = roster_row(rows, card)
        if card.fields.get("ask") == "input" and row_state(row) == "working":
            _leave_needs_you(card)
            board.move(card, "In progress")
            events.append(f"{card.id}: worker resumed -> In progress")
            continue
        waiting = _parse_when(card.fields.get("waiting_since"))
        if waiting is None:
            card.fields["waiting_since"] = now.replace(microsecond=0).isoformat()
            continue
        if auto_close is not None and now - waiting > auto_close:
            days = _days(now - waiting)
            card.fields["note"] = _clean(f"dropped after {days}d in Needs you")
            archive_card(root, board, card, outcome="dropped", now=now)
            events.append(f"{card.id}: {days}d in Needs you -> dropped (archive)")

    # 4. sweep closed cards to the archive
    for card in list(board.lanes["Done"]):
        if card.done:
            archive_card(root, board, card, outcome="closed", now=now)
            events.append(f"{card.id}: closed -> archive")
    return events


# ---------------------------------------------------------------- doctor + snapshot

def issues(root: str | Path, board: Board, *, now: dt.datetime | None = None, rows: list[dict] | None = None) -> list[str]:
    now = now or now_local()
    out: list[str] = []
    stale_after = board.stale_after()
    p = paths(root)
    for card in board.cards():
        if card.fields.get("brief") and not (p["dir"] / card.fields["brief"]).is_file():
            out.append(f"{card.id}: brief missing ({card.fields['brief']})")
        since = _parse_when(card.fields.get("since"))
        if card.lane == "Queued" and stale_after and since and now - since > stale_after:
            out.append(f"{card.id}: queued {_days(now - since)}d, dispatch or drop it")
        if card.lane == "Done" and not card.done and since and stale_after and now - since > stale_after:
            out.append(f"{card.id}: still closing after {_days(now - since)}d; closeout did not run")
        if card.lane == "In progress" and rows is not None and roster_row(rows, card) is None and card.fields.get("by") == "orchestrator":
            out.append(f"{card.id}: in progress with no live session")
        if card.lane == "Needs you":
            waiting = _parse_when(card.fields.get("waiting_since"))
            if waiting and stale_after and now - waiting > stale_after:
                out.append(f"{card.id}: waiting on you for {_days(now - waiting)}d ({card.fields.get('ask')})")
    return out


def snapshot(root: str | Path, board: Board, *, rows: list[dict] | None = None, now: dt.datetime | None = None,
             home=None) -> dict:
    now = now or now_local()
    lanes = []
    waiting = 0
    for lane in LANES:
        cards = []
        for card in board.lanes.get(lane, []):
            row = roster_row(rows, card)
            state = row_state(row) if rows is not None else "unknown"
            if lane == "Done":
                state = card.state_label()
            elif state == "absent":
                state = "no session"
            since = _parse_when(card.fields.get("since"))
            entry = {"id": card.id, "project": card.project, "deliverable": card.deliverable, "owner": card.owner,
                     "lane": lane, "state": state, "session_id": (row or {}).get("sessionId") or None,
                     "session_prefix": card.fields.get("session"), "age_hours": int((now - since).total_seconds() // 3600) if since else None}
            for key in FIELD_KEYS:
                if key in card.fields:
                    entry[key] = card.fields[key]
            cards.append(entry)
            if lane == "Needs you":
                waiting += 1
        lanes.append({"name": lane, "cards": cards})
    return {"schema_version": SCHEMA_VERSION, "meta": dict(board.meta), "generated_at": now.replace(microsecond=0).isoformat(),
            "roster_available": rows is not None, "open": sum(1 for c in board.cards() if not c.done),
            "waiting_on_you": waiting, "lanes": lanes}


# ---------------------------------------------------------------- background worker launch

def launch_background(root: str | Path, card: Card, *, model: str | None = None,
                      permission_mode: str = "acceptEdits", runner=None) -> str:
    """Start ``claude --bg --name <owner>`` on the card's brief. Returns the session id prefix."""
    p = paths(root)
    brief = card.fields.get("brief")
    if not brief:
        raise BoardError(f"{card.id} has no brief to dispatch")
    exe = claude_bin()
    if not exe and runner is None:
        raise BoardError("claude CLI not found; install it or set AGENTFRAME_CLAUDE_BIN")
    brief_path = (p["dir"] / brief).resolve()
    kickoff = (f"You are a dispatched AgentFrame worker for card {card.id}. Read the task brief at {brief_path} "
               f"and execute it exactly, writing only inside the paths it names. Write the receipt it names when you "
               f"finish or when you are blocked. Work in this checkout directly: never create or enter a git worktree, "
               f"because the vault's project and board files are gitignored and do not exist in a worktree.")
    cmd = [exe or "claude", "--bg", "--name", card.owner, "--permission-mode", permission_mode,
           "--settings", json.dumps({"crossSessionInbound": "accept"})]
    if model:
        cmd += ["--model", model]
    cmd.append(kickoff)
    run = runner or (lambda argv: subprocess.run(argv, capture_output=True, text=True, encoding="utf-8",
                                                 errors="replace", timeout=60, cwd=str(root)).stdout)
    out = run(cmd) or ""
    # first 8-hex word after "backgrounded"; tolerant of console mojibake around the separators
    m = re.search(r"backgrounded.*?\b([0-9a-f]{8})\b", out)
    if not m:
        raise BoardError(f"could not read the session id from claude --bg output:\n{out[:400]}")
    return m.group(1)
