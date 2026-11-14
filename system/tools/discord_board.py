"""Keep Discord boards and forum posts current by editing them in place.

Standard library only; reuses notify.py's .env loading and message split.
A text channel holds one pinned board, edited on every run. A forum channel
holds one post per item (a slug), its starter message edited on every run.
Nothing is date-bound: each run appends "_Updated YYYY-MM-DD_" (local date).
An edit alone shows him nothing (Discord marks a thread unread only on a new
message), so a post or board that already existed and whose content changed
gets one short "Updated YYYY-MM-DD" message at its end, the previous one
deleted first; --no-bump skips it. A new post or first board gets none. The
content hash is stored once the bump has landed, so a failed bump is retried
by the next run; each bump deletes the previous one first. A send that lands
but whose answer is lost leaves a message no run deletes.

Paths resolve from the vault root: AF_ROOT, else the repo holding this file.

    <root>/system/tools/.env                      DISCORD_BOT_TOKEN, <KEY>_DISCORD_CHANNEL_ID
    <root>/workspace/life/systems/discord/state.json

The channel id comes from <KEY>_DISCORD_CHANNEL_ID, else state.json's
channels.<key>.id; forum tag names come from channels.<key>.tags. This tool
owns state.json's "boards" and "posts" and leaves every other key alone.
A run holds state.json.lock while it reads and writes state.json.

Usage:
    python system/tools/discord_board.py board open board.md
    python system/tools/discord_board.py post food ramen-spot note.md --title "Ramen spot" --tag Plan
    python system/tools/discord_board.py board open board.md --dry-run   # print, send nothing, write nothing

On a later post run, omitting --tag keeps the thread's current tags.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
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
MAX_WAIT = 60.0
LOCK_STALE = 600
THREAD_NAME_MAX = 100
UNKNOWN_MESSAGE = 10008
NO_MENTIONS = {"parse": []}


class BoardError(Exception):
    """A refusal or failure whose message is safe to print."""


class DiscordError(BoardError):
    def __init__(self, message: str, status: int | None = None, code: int | None = None):
        super().__init__(message)
        self.status, self.code = status, code


class NotFound(DiscordError):
    """Any 404: stops the run; only UnknownMessage proves a message is gone."""


class UnknownMessage(NotFound):
    """404 with Discord code 10008: the stored message was deleted."""


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
        where = redact(f"{method} {path}")
        for attempt in range(MAX_RETRIES + 1):
            req = urllib.request.Request(API + path, data=data, headers=headers, method=method)
            try:
                with opener(req, timeout=30) as resp:
                    raw = resp.read()
            except urllib.error.HTTPError as e:
                payload = _json_or_empty(e)
                if e.code == 429:
                    wait = float(payload.get("retry_after") or (e.headers.get("Retry-After") if e.headers else None) or 1)
                    if wait > MAX_WAIT:
                        raise DiscordError(f"{where}: rate limited for {wait:.0f}s, over the {MAX_WAIT:.0f}s cap; stopped, try again later.", 429) from None
                    if attempt < MAX_RETRIES:
                        self._sleep(wait + 0.1)
                        continue
                    raise DiscordError(f"{where}: still rate limited after {MAX_RETRIES} retries.", 429) from None
                code = payload.get("code")
                why = f"{where}: HTTP {e.code}" + (f" code {code}" if code else "") + f" {payload.get('message') or e.reason}"
                kind = DiscordError
                if e.code == 404:
                    kind = UnknownMessage if code == UNKNOWN_MESSAGE else NotFound
                raise kind(redact(why), e.code, code) from None
            except (urllib.error.URLError, TimeoutError, OSError) as e:
                raise DiscordError(f"{where}: {redact(str(getattr(e, 'reason', e)))}") from None
            if not raw:
                return None
            try:
                return json.loads(raw)
            except ValueError:
                raise DiscordError(f"{where}: Discord answered with a body that is not JSON.") from None
        raise DiscordError(f"{where}: no answer.")  # unreachable

    def _id(self, method: str, path: str, body: dict) -> str:
        out = self.call(method, path, body)
        if not isinstance(out, dict) or not out.get("id"):
            raise DiscordError(redact(f"{method} {path}: Discord's answer carries no id."))
        return str(out["id"])

    def send(self, channel: str, content: str) -> str:
        return self._id("POST", f"/channels/{channel}/messages", {"content": content, "allowed_mentions": NO_MENTIONS})

    def edit(self, channel: str, message: str, content: str) -> None:
        self.call("PATCH", f"/channels/{channel}/messages/{message}", {"content": content, "allowed_mentions": NO_MENTIONS})

    def delete(self, channel: str, message: str) -> None:
        try:
            self.call("DELETE", f"/channels/{channel}/messages/{message}")
        except UnknownMessage:
            pass

    def pin(self, channel: str, message: str) -> None:
        try:
            self.call("PUT", f"/channels/{channel}/messages/pins/{message}")
        except DiscordError as e:
            if e.status not in (404, 405):
                raise
            self.call("PUT", f"/channels/{channel}/pins/{message}")  # the older route

    def create_thread(self, forum: str, title: str, content: str, tag_ids: list[str]) -> str:
        body: dict = {"name": title, "message": {"content": content, "allowed_mentions": NO_MENTIONS}}
        if tag_ids:
            body["applied_tags"] = tag_ids
        return self._id("POST", f"/channels/{forum}/threads", body)


def _json_or_empty(e: urllib.error.HTTPError) -> dict:
    try:
        out = json.loads(e.read() or b"{}")
        return out if isinstance(out, dict) else {}
    except Exception:  # noqa: BLE001
        return {}


# --- content, state, lock --------------------------------------------------


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


def content_hash(title: str | None, text: str, tags: list[str] | None = None) -> str:
    """SHA-256 over the title, the tag names (a post) and the source text as the
    tool was given it, before it is composed and stamped: the daily "_Updated_"
    line never enters it, and a line the user wrote that looks like it still does."""
    body = [title, sorted(tags) if tags is not None else None, text.strip()]
    return hashlib.sha256(json.dumps(body, ensure_ascii=False).encode("utf-8")).hexdigest()


def load_state(path: Path) -> dict:
    if not path.exists():
        return {}
    try:
        state = json.loads(path.read_text(encoding="utf-8"))
    except ValueError as e:
        raise BoardError(f"{path} is not valid JSON ({e}); fix or restore it, nothing sent.") from None
    if not isinstance(state, dict):
        raise BoardError(f"{path} is not a JSON object; fix or restore it, nothing sent.")
    return state


def save_state(path: Path, state: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(path.suffix + ".tmp")
    tmp.write_text(json.dumps(state, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    os.replace(tmp, path)


class StateLock:
    """state.json.lock, created exclusively; a lock older than LOCK_STALE seconds is taken over."""

    def __init__(self, state_path: Path):
        self.path = state_path.with_name(state_path.name + ".lock")

    def __enter__(self):
        self.path.parent.mkdir(parents=True, exist_ok=True)
        for _ in range(2):
            try:
                fd = os.open(self.path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
            except FileExistsError:
                try:
                    age = time.time() - self.path.stat().st_mtime
                except FileNotFoundError:
                    continue  # released meanwhile
                if age <= LOCK_STALE:
                    raise BoardError(f"{self.path} is held by another run ({age:.0f}s old); try again shortly, "
                                     f"or delete it if no run is active. Nothing sent.") from None
                try:
                    os.remove(self.path)  # stale: take it over
                except FileNotFoundError:
                    pass
                continue
            with os.fdopen(fd, "w", encoding="utf-8") as f:
                f.write(f"pid {os.getpid()} at {dt.datetime.now().isoformat(timespec='seconds')}\n")
            return self
        raise BoardError(f"could not take {self.path}; try again. Nothing sent.")

    def __exit__(self, *exc):
        try:
            os.remove(self.path)
        except FileNotFoundError:
            pass
        return False


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


def sync_messages(api: Discord, channel: str, record: list[str], parts: list[str]) -> bool:
    """Make the messages in `record` carry `parts`, in order, editing in place.

    `record` changes only when a change is confirmed: an id leaves it when Discord
    says the message is gone (10008) or its delete succeeds, and joins it when a
    post lands. Any other failure stops the run with every stored id kept. A
    message deleted by hand breaks the order, so every later one is reposted too.
    Returns whether any new message was sent (an edit does not count)."""
    sent = False
    plan = list(record)
    for i, part in enumerate(parts):
        if i < len(plan):
            try:
                api.edit(channel, plan[i], part)
                continue
            except UnknownMessage:
                record.remove(plan[i])
                for mid in plan[i + 1:]:
                    api.delete(channel, mid)
                    record.remove(mid)
                del plan[i:]
        mid = api.send(channel, part)
        sent = True
        record.append(mid)
        plan.append(mid)
    for mid in plan[len(parts):]:
        api.delete(channel, mid)
        record.remove(mid)
    return sent


def drop_bump(api: Discord, channel: str, holder: dict) -> None:
    """Delete the stored bump (already gone is fine) and forget its id once the
    delete is confirmed; any other failure keeps the stored id."""
    old = holder.get("bump")
    if old:
        api.delete(channel, old)
        del holder["bump"]


def settle_bump(api: Discord, channel: str, holder: dict, *, existed: bool, changed: bool,
                sent: bool, no_bump: bool, text: str) -> str:
    """Keep the one bump last. A changed item that already existed is bumped; so is
    one that had a message sent after its bump was stored (the bump would sit
    mid-content). With --no-bump that stale bump is deleted instead."""
    stale = sent and bool(holder.get("bump"))
    if no_bump:
        if stale:
            drop_bump(api, channel, holder)
        return ""
    if existed and (changed or stale):
        drop_bump(api, channel, holder)
        holder["bump"] = api.send(channel, text)
        return ", bumped"
    return ""


# --- commands ---------------------------------------------------------------


def print_parts(parts: list[str], actions: list[str]) -> None:
    for i, part in enumerate(parts):
        print(f"--- message {i + 1} of {len(parts)} ({actions[i]}, {len(part)} chars) ---")
        print(part)


def no_channel(key: str) -> BoardError:
    return BoardError(f"no {key.upper()}_DISCORD_CHANNEL_ID in the .env and no channels.{key}.id in state.json.")


def run_board(a, state: dict, env: dict, api: Discord | None, parts: list[str]) -> str:
    channel, _ = channel_info(state, env, a.key, "text")
    boards = state.setdefault("boards", {})
    board = boards.get(a.key) or {}
    stored = list(board.get("messages") or [])
    existed = bool(board.get("hash") or board.get("pinned"))  # a half-made first board is still first
    digest = content_hash(None, a.source)
    changed = board.get("hash") != digest
    if a.dry_run:
        print(f"board {a.key}: {len(parts)} message(s); {len(stored)} stored")
        print_parts(parts, ["edit" if i < len(stored) else "post" for i in range(len(parts))])
        if not stored or board.get("pinned") != stored[0]:
            print("would pin message 1")
        if len(stored) > len(parts):
            print(f"would delete {len(stored) - len(parts)} leftover message(s)")
        if existed and changed and not a.no_bump:
            print("would bump")
        return "dry run: nothing sent, state unchanged."
    if not channel:
        raise no_channel(a.key)
    board = boards[a.key] = {**board, "messages": stored, "pinned": board.get("pinned")}
    sent = sync_messages(api, channel, stored, parts)
    pinned = ""
    if board["pinned"] != stored[0]:
        api.pin(channel, stored[0])
        board["pinned"] = stored[0]  # only once the pin has landed
        pinned = ", pinned"
    bumped = settle_bump(api, channel, board, existed=existed, changed=changed, sent=sent,
                         no_bump=a.no_bump, text=f"Updated {a.today.isoformat()}")
    board["hash"] = digest  # only once the bump has landed, so a failed bump is retried
    return f"board {a.key}: {len(parts)} message(s) current{pinned}{bumped}."


def create_post(api: Discord, posts: dict, forum: str, slug: str, title: str,
                tags: list[tuple[str, str]], parts: list[str], digest: str) -> None:
    """A fresh thread; the stored item is replaced only once the thread exists.
    A new thread is not bumped: creating it already shows."""
    thread = api.create_thread(forum, title, parts[0], [t for _, t in tags])
    record: list[str] = []
    posts[slug] = {"thread": thread, "title": title, "tags": [n for n, _ in tags], "messages": record, "hash": digest}
    sync_messages(api, thread, record, parts[1:])


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
    tag_names = [n for n, _ in tags] if a.tag else list(item.get("tags") or [])
    digest = content_hash(title, a.source, tag_names)
    changed = item.get("hash") != digest
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
            if changed and not a.no_bump:
                print("would bump")
        return "dry run: nothing sent, state unchanged."
    if not forum:
        raise no_channel(a.key)
    if not thread:
        create_post(api, posts, forum, a.slug, title, tags, parts, digest)
        return f"post {a.key}/{a.slug}: created with {len(parts)} message(s)."

    try:
        info = api.call("GET", f"/channels/{thread}") or {}
    except NotFound as e:
        raise BoardError(f"{e}. The thread for {a.key}/{a.slug} is not reachable; if it was deleted by hand, "
                         f"remove posts.{a.key}.{a.slug} from state.json and rerun.") from None
    if (info.get("thread_metadata") or {}).get("archived"):
        api.call("PATCH", f"/channels/{thread}", {"archived": False})
    try:
        api.edit(thread, thread, parts[0])  # a forum thread's starter message id is the thread id
    except UnknownMessage:  # the starter was deleted by hand: start the post over
        try:
            drop_bump(api, thread, item)  # best effort: the thread may be gone
        except Exception:  # noqa: BLE001
            pass
        if not a.tag:  # keep the tags it had
            known = entry.get("tags") or {}
            tags = [(n, str(known[n])) for n in item.get("tags") or [] if n in known]
        create_post(api, posts, forum, a.slug, title, tags, parts, digest)
        return f"post {a.key}/{a.slug}: starter was gone; created afresh with {len(parts)} message(s)."
    item = posts[a.slug] = {**item, "messages": extra}
    change: dict = {}
    if item.get("title") != title:  # against what we last sent, never Discord's normalised name
        change["name"] = title
    if a.tag and sorted(map(str, info.get("applied_tags") or [])) != sorted(t for _, t in tags):
        change["applied_tags"] = [t for _, t in tags]
    if change:
        api.call("PATCH", f"/channels/{thread}", change)
    item["title"] = title
    if a.tag:
        item["tags"] = [n for n, _ in tags]
    sent = sync_messages(api, thread, extra, parts[1:])
    bumped = settle_bump(api, thread, item, existed=True, changed=changed, sent=sent,
                         no_bump=a.no_bump, text=f"Updated {a.today.isoformat()}: {title}")
    item["hash"] = digest  # only once the bump has landed, so a failed bump is retried
    return (f"post {a.key}/{a.slug}: {len(parts)} message(s) current"
            + (f", updated {' and '.join(change)}" if change else "") + bumped + ".")


def execute(a, state_path: Path, env: dict, api: Discord | None, parts: list[str]) -> str:
    state = load_state(state_path)
    before = json.dumps(state, sort_keys=True)
    run = run_board if a.cmd == "board" else run_post
    try:
        return run(a, state, env, api, parts)
    finally:
        # keep every id that landed, even after a failure; a dry run never writes
        if api is not None and json.dumps(state, sort_keys=True) != before:
            save_state(state_path, state)


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
        sp.add_argument("--no-bump", action="store_true", help='skip the "Updated" message that shows a changed post or board as unread')
        sp.add_argument("--env", type=Path, default=None, help=f"credentials file (default <root>/{ENV_REL.as_posix()})")
        sp.add_argument("--state", type=Path, default=None, help=f"state file (default <root>/{STATE_REL.as_posix()})")
    a = p.parse_args(argv)
    try:
        sys.stdout.reconfigure(encoding="utf-8")  # type: ignore[attr-defined]
    except Exception:  # noqa: BLE001
        pass
    a.key = a.key.strip().lower()
    a.today = today or dt.date.today()
    env_path = a.env or root() / ENV_REL
    state_path = a.state or root() / STATE_REL
    try:
        a.source = a.file.read_text(encoding="utf-8")
        parts = compose(a.source, a.today)
        if not parts:
            raise BoardError(f"{a.file} is empty; nothing to post.")
        env = {**os.environ, **load_env(env_path)}
        if a.dry_run:
            print(execute(a, state_path, env, None, parts))
            return 0
        token = env.get("DISCORD_BOT_TOKEN")
        if not token:
            raise BoardError(f"no DISCORD_BOT_TOKEN in {env_path} or the environment; nothing sent.")
        with StateLock(state_path):
            print(execute(a, state_path, env, Discord(token, opener=opener, sleep=sleep), parts))
    except BoardError as e:
        print(f"discord_board: {e}", file=sys.stderr)
        return 2
    except OSError as e:
        print(f"discord_board: {redact(str(e))}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
