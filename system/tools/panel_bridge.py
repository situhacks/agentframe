"""Client for the AgentFrame VS Code bridge extension.

    panel_bridge.py <action> [--json '<args>'] [--timeout 20]

Writes system/logs/panel-bridge/requests/<id>.json in the vault root (never a worktree), waits for
results/<id>.json, prints it, and exits 0 on ok, 1 on error, 2 on timeout. Actions: ping, tabs, open,
reveal, close-tab, group-pick. On Windows with Git Bash, pass text that starts with a slash through
--json-file, because the shell rewrites a leading slash in an argument.
"""
import argparse
import json
import os
import re
import sys
import time
import uuid
from pathlib import Path


def vault_root() -> Path:
    env = os.environ.get("AF_ROOT")
    if env:
        return Path(env)
    p = Path(__file__).resolve().parent.parent.parent
    s = str(p)
    m = re.search(r"[\\/]\.claude[\\/]worktrees[\\/][^\\/]+$", s)
    if m:
        p = Path(s[: m.start()])
    return p


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="Send one action to the AgentFrame VS Code bridge.")
    ap.add_argument("action")
    ap.add_argument("--json", default=None, help="the action's arguments as a JSON object")
    ap.add_argument("--json-file", default=None, help="read the arguments from a JSON file instead")
    ap.add_argument("--timeout", type=float, default=20.0)
    ns = ap.parse_args(argv)

    args = {}
    if ns.json_file:
        args = json.loads(Path(ns.json_file).read_text(encoding="utf-8-sig"))
    elif ns.json:
        args = json.loads(ns.json)
    if not isinstance(args, dict):
        print("arguments must be a JSON object", file=sys.stderr)
        return 1

    base = vault_root() / "system" / "logs" / "panel-bridge"
    req_dir = base / "requests"
    res_dir = base / "results"
    req_dir.mkdir(parents=True, exist_ok=True)
    res_dir.mkdir(parents=True, exist_ok=True)

    rid = time.strftime("%Y%m%dT%H%M%S") + "-" + uuid.uuid4().hex[:8]
    req = {"id": rid, "action": ns.action, "args": args, "ts": int(time.time() * 1000)}
    tmp = req_dir / (rid + ".tmp")
    tmp.write_text(json.dumps(req), encoding="utf-8")
    os.replace(tmp, req_dir / (rid + ".json"))

    result_path = res_dir / (rid + ".json")
    deadline = time.time() + ns.timeout
    while time.time() < deadline:
        if result_path.exists():
            try:
                body = json.loads(result_path.read_text(encoding="utf-8-sig"))
            except (ValueError, OSError):
                time.sleep(0.05)
                continue
            try:
                result_path.unlink()
            except OSError:
                pass
            print(json.dumps(body, indent=2))
            return 0 if body.get("ok") else 1
        time.sleep(0.05)

    try:
        (req_dir / (rid + ".json")).unlink()  # not picked up: do not leave it to run later
    except OSError:
        pass
    print(json.dumps({"id": rid, "ok": False, "error": "timeout: no result within %s s" % ns.timeout}), file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
