"""Keep Discord boards and forum posts current by editing them in place.

Standard library only; reuses notify.py's .env loading and message split.
A text channel holds one pinned board, edited on every run. A forum channel
holds one post per item (a slug), its starter message edited on every run.
Nothing is date-bound: each run appends "_Updated YYYY-MM-DD_" (local date).

Paths resolve from the vault root: AF_ROOT, else the repo holding this file.

    <root>/system/tools/.env                      DISCORD_BOT_TOKEN, <KEY>_DISCORD_CHANNEL_ID
    <root>/workspace/life/systems/discord/state.json

The channel id comes from <KEY>_DISCORD_CHANNEL_ID, else state.json's
channels.<key>.id; forum tag names come from channels.<key>.tags. This tool
owns state.json's "boards" and "posts" and leaves every other key alone.

Usage:
    python system/tools/discord_board.py board open board.md
    python system/tools/discord_board.py post food ramen-spot note.md --title "Ramen spot" --tag Plan
    python system/tools/discord_board.py board open board.md --dry-run   # print, send nothing, write nothing

On a later post run, omitting --tag keeps the thread's current tags.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from notify import LIMIT, chunks, load_env  # noqa: E402

API = "https://discord.com/api/v10"
USER_AGENT = "AgentFrame-board (local, 1.0)"
ENV_REL = Path("system/tools/.env")
STATE_REL = Path("workspace/life/systems/discord/state.json")
MAX_RETRIES = 5
THREAD_NAME_MAX = 100


class BoardError(Exception):
    """A refusal or failure whose message is safe to print."""


class DiscordError(BoardError):
    pass


class NotFound(DiscordError):
    pass


def redact(text: str) -> str:
    """Hide snowflake ids in anything printed."""
    return re.sub(r"\d{15,}", "<id>", text)


def root() -> Path:
    return Path(os.environ.get("AF_ROOT") or Path(__file__).resolve().parent.parent.parent)


# --- Discord HTTP ---------------------------------------------------------


class Discord:
    """A minimal bot client: JSON in and out, 429 retried, 404 raised as NotFound."""

    def __init__(self, token: str, opener=None, sleep=time.sleep):
        self._token = token
        self._opener = opener
        self._sleep = sleep

    def call(self, method: str, path: str, body: dict | None = None):
        data = json.dumps(body).encode("utf-8") if body is not None else None
        headers = {"Authorization": f"Bot {self._token}", "User-Agent": USER_AGENT}
        if data is not None:
            headers["Content-Type"] = "application/json"
        opener = self._opener or urllib.request.urlopen
        for attempt in range(MAX_RETRIES + 1):
            req = urllib.request.Request(API + path, data=data, headers=headers, method=method)
            try:
                with opener(req, timeout=30) as resp:
                    raw = resp.read()
                return json.loads(raw) if raw else None
            except urllib.error.HTTPError as e:
                payload = _json_or_empty(e)
                if e.code == 429 and attempt < MAX_RETRIES:
                    wait = payload.get("retry_after") or (e.headers.get("Retry-After") if e.headers else None) or 1
                    self._sleep(float(wait) + 0.1)
                    continue
                why = redact(f"{method} {path}: HTTP {e.code} {payload.get('message') or e.reason}")
                raise (NotFound if e.code == 404 else DiscordError)(why) from None
            except (urllib.error.URLError, TimeoutError, OSError) as e:
                raise DiscordError(redact(f"{method} {path}: {getattr(e, 'reason', e)}")) from None
        raise DiscordError(redact(f"{method} {path}: still rate limited after {MAX_RETRIES} retries"))

    def send(self, channel: str, content: str) -> str:
        return self.call("POST", f"/channels/{channel}/messages", {"content": content})["id"]

    def edit(self, channel: str, message: str, content: str) -> None:
        self.call("PATCH", f"/channels/{channel}/messages/{message}", {"content": content})

    def delete(self, channel: str, message: str) -> None:
        try:
            self.call("DELETE", f"/channels/{channel}/messages/{message}")
        except NotFound:
            pass

    def pin(self, channel: str, message: str) -> None:
        self.call("PUT", f"/channels/{channel}/pins/{message}")


def _json_or_empty(e: urllib.error.HTTPError) -> dict:
    try:
        out = json.loads(e.read() or b"{}")
        return out if isinstance(out, dict) else {}
    except Exception:  # noqa: BLE001
        return {}


# --- content and state ------------------------------------------------------


def compose(text: str, today: dt.date) -> list[str]:
    """The messages for a file: its text plus the Updated line, split under the limit."""
    text = text.strip()
    if not text:
        return []
    out: list[str] = []
    for part in chunks(f"{text}\n\n_Updated {today.isoformat()}_"):
        while len(part) > LIMIT:  # a single line longer than the limit
            out.append(part[:LIMIT])
            part = part[LIMIT:]
        if part.strip():
            out.append(part)
    return out


def load_state(path: Path) -> dict:
    if not path.exists():
        return {}
    return json.loads(path.read_text(encoding="utf-8"))


def save_state(path: Path, state: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(state, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    os.replace(tmp, path)


def channel_info(state: dict, env: dict, key: str, want: str) -> tuple[str | None, dict]:
    """The channel id and its state entry; refuses a channel of the wrong type."""
    entry = (state.get("channels") or {}).get(key) or {}
    kind = entry.get("type")
    if kind and kind != want:
        verb = "board" if want == "text" else "post"
        raise BoardError(f"channel '{key}' is a {kind} channel; '{verb}' needs a {want} channel.")
    return env.get(f"{key.upper()}_DISCORD_CHANNEL_ID") or entry.get("id"), entry


def resolve_tags(entry: dict, key: str, names: list[str]) -> list[tuple[str, str]]:
    """Tag names to (name, id), exact then case-insensitive; unknown names refuse."""
    known: dict[str, str] = entry.get("tags") or {}
    folded = {k.casefold(): k for k in known}
    out, bad = [], []
    for name in names:
        real = name if name in known else folded.get(name.casefold())
        if real is None:
            bad.append(name)
        elif real not in [n for n, _ in out]:
            out.append((real, str(known[real])))
    if bad:
        valid = ", ".join(sorted(known)) or "(none configured)"
        raise BoardError(f"unknown tag(s) for '{key}': {', '.join(bad)}. Valid tags: {valid}.")
    return out


def sync_messages(api: Discord, channel: str, record: list[str], parts: list[str]) -> None:
    """Make the messages in `record` carry `parts`, in order, editing in place.

    `record` is mutated as each call lands, so it is the truth even after a failure.
    A stored message deleted by hand breaks the order, so it and every later one
    are replaced by fresh posts."""
    for i, part in enumerate(parts):
        if i < len(record):
            try:
                api.edit(channel, record[i], part)
                continue
            except NotFound:
                for mid in record[i + 1:]:
                    api.delete(channel, mid)
                del record[i:]
        record.append(api.send(channel, part))
    while len(record) > len(parts):
        api.delete(channel, record[-1])
        record.pop()


# --- commands ---------------------------------------------------------------


def print_parts(parts: list[str], actions: list[str]) -> None:
    for i, part in enumerate(parts):
        print(f"--- message {i + 1} of {len(parts)} ({actions[i]}, {len(part)} chars) ---")
        print(part)


def run_board(a, state: dict, env: dict, api: Discord | None, parts: list[str]) -> str:
    channel, _ = channel_info(state, env, a.key, "text")
    boards = state.setdefault("boards", {})
    stored = list(boards.get(a.key) or [])
    if a.dry_run:
        print(f"board {a.key}: {len(parts)} message(s); {len(stored)} stored")
        print_parts(parts, ["edit" if i < len(stored) else "post" for i in range(len(parts))])
        if not stored:
            print("would pin message 1")
        if len(stored) > len(parts):
            print(f"would delete {len(stored) - len(parts)} leftover message(s)")
        return "dry run: nothing sent, state unchanged."
    if not channel:
        raise BoardError(f"no {a.key.upper()}_DISCORD_CHANNEL_ID in the .env and no channels.{a.key}.id in state.json.")
    first = stored[0] if stored else None
    record = stored
    boards[a.key] = record
    sync_messages(api, channel, record, parts)
    if record[0] != first:
        api.pin(channel, record[0])
    return f"board {a.key}: {len(parts)} message(s) current" + (", pinned" if record[0] != first else "") + "."


def run_post(a, state: dict, env: dict, api: Discord | None, parts: list[str]) -> str:
    forum, entry = channel_info(state, env, a.key, "forum")
    tags = resolve_tags(entry, a.key, a.tag or [])
    title = a.title.strip()[:THREAD_NAME_MAX]
    if not title:
        raise BoardError("--title is empty.")
    posts = state.setdefault("posts", {}).setdefault(a.key, {})
    item = posts.get(a.slug) or {}
    thread = item.get("thread")
    extra = list(item.get("messages") or [])
    if a.dry_run:
        if not thread:
            print(f"post {a.key}/{a.slug}: would create thread \"{title}\" tags [{', '.join(n for n, _ in tags)}]")
            print_parts(parts, ["starter"] + ["post"] * (len(parts) - 1))
        else:
            if item.get("title") != title:
                print(f"would rename thread to \"{title}\"")
            if a.tag and sorted(item.get("tags") or []) != sorted(n for n, _ in tags):
                print(f"would set tags [{', '.join(n for n, _ in tags)}]")
            print(f"post {a.key}/{a.slug}: {len(parts)} message(s); {1 + len(extra)} stored")
            print_parts(parts, ["edit starter"] + ["edit" if i < len(extra) else "post" for i in range(len(parts) - 1)])
            if len(extra) > len(parts) - 1:
                print(f"would delete {len(extra) - len(parts) + 1} leftover message(s)")
        return "dry run: nothing sent, state unchanged."
    if not forum:
        raise BoardError(f"no {a.key.upper()}_DISCORD_CHANNEL_ID in the .env and no channels.{a.key}.id in state.json.")

    info = None
    if thread:
        try:
            info = api.call("GET", f"/channels/{thread}")
        except NotFound:
            info = None
    if info is None:  # first run, or the thread was deleted by hand
        body: dict = {"name": title, "message": {"content": parts[0]}}
        if tags:
            body["applied_tags"] = [t for _, t in tags]
        thread = api.call("POST", f"/channels/{forum}/threads", body)["id"]
        record: list[str] = []
        posts[a.slug] = {"thread": thread, "title": title, "tags": [n for n, _ in tags], "messages": record}
        sync_messages(api, thread, record, parts[1:])
        return f"post {a.key}/{a.slug}: created with {len(parts)} message(s)."

    if (info.get("thread_metadata") or {}).get("archived"):
        api.call("PATCH", f"/channels/{thread}", {"archived": False})
    change: dict = {}
    if info.get("name") != title:
        change["name"] = title
    if a.tag and sorted(map(str, info.get("applied_tags") or [])) != sorted(t for _, t in tags):
        change["applied_tags"] = [t for _, t in tags]
    if change:
        api.call("PATCH", f"/channels/{thread}", change)
    item["title"] = title
    if a.tag:
        item["tags"] = [n for n, _ in tags]
    record = extra
    item["messages"] = record
    api.edit(thread, thread, parts[0])  # a forum thread's starter message id is the thread id
    sync_messages(api, thread, record, parts[1:])
    return f"post {a.key}/{a.slug}: {len(parts)} message(s) current" + (f", updated {' and '.join(change)}" if change else "") + "."


def main(argv: list[str] | None = None, *, opener=None, sleep=time.sleep, today: dt.date | None = None) -> int:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    b = sub.add_parser("board", help="a text channel's pinned board")
    b.add_argument("key", help="channel key, e.g. open")
    b.add_argument("file", type=Path)
    f = sub.add_parser("post", help="one forum post per item")
    f.add_argument("key", help="forum key, e.g. food")
    f.add_argument("slug", help="the item's stable name in state.json")
    f.add_argument("file", type=Path)
    f.add_argument("--title", required=True)
    f.add_argument("--tag", action="append", help="forum tag name; repeat for several")
    for sp in (b, f):
        sp.add_argument("--dry-run", action="store_true", help="print what would be sent; change nothing")
        sp.add_argument("--env", type=Path, default=None, help=f"credentials file (default <root>/{ENV_REL.as_posix()})")
        sp.add_argument("--state", type=Path, default=None, help=f"state file (default <root>/{STATE_REL.as_posix()})")
    a = p.parse_args(argv)
    try:
        sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:  # noqa: BLE001
        pass
    a.key = a.key.strip().lower()
    env_path = a.env or root() / ENV_REL
    state_path = a.state or root() / STATE_REL
    state: dict = {}
    api = None
    try:
        parts = compose(a.file.read_text(encoding="utf-8"), today or dt.date.today())
        if not parts:
            raise BoardError(f"{a.file} is empty; nothing to post.")
        state = load_state(state_path)
        before = json.dumps(state, sort_keys=True)
        env = {**os.environ, **load_env(env_path)}
        if not a.dry_run:
            token = env.get("DISCORD_BOT_TOKEN")
            if not token:
                raise BoardError(f"no DISCORD_BOT_TOKEN in {env_path} or the environment; nothing sent.")
            api = Discord(token, opener=opener, sleep=sleep)
        run = run_board if a.cmd == "board" else run_post
        try:
            print(run(a, state, env, api, parts))
        finally:
            # keep every id that landed, even after a failure; a dry run never writes
            if api is not None and json.dumps(state, sort_keys=True) != before:
                save_state(state_path, state)
    except BoardError as e:
        print(f"discord_board: {e}", file=sys.stderr)
        return 2
    except OSError as e:
        print(f"discord_board: {redact(str(e))}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
