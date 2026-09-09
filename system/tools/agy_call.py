#!/usr/bin/env python3
"""Delegate one bounded task to Gemini through the Antigravity CLI (`agy`) in print mode.

The connector AgentFrame uses when a task needs native video or audio perception, very long
multimodal context, or cheap bulk classification, and the session model should keep its context
for judgment. Runs on the operator's Antigravity login (Google AI Pro); nothing local, no GPU.

Usage:
    python system/tools/agy_call.py --prompt "..." [--file PATH ...] [--schema JSON|path] [--model SLUG]
                                    [--timeout 5m] [--out result.json] [--allow-commands]

Prints the structured output (when a schema is given) or the response text to stdout; the full
envelope (status, usage, conversation_id, denied_actions) goes to --out when asked.

The fence, and why it is shaped this way (measured on agy 1.1.27, 2026-09-08):
  * Print mode does NOT gate file writes inside the agent's workspace: neither a view-only
    permissions allow-list nor --mode plan stopped a write_to_file. Only shell commands are gated.
    So the agent never sees a real folder. Every input is copied read-only into a disposable temp
    directory, that directory is the whole workspace (cwd + --add-dir), and it is deleted after.
  * --dangerously-skip-permissions is never passed by default; shell commands stay denied and
    show up in `denied_actions`. --allow-commands opts in for a task that truly needs them.
  * Media is handed over by naming its ABSOLUTE PATH in the prompt. The `@file` syntax does not
    attach media in print mode; it makes the agent search the filesystem and burn tokens.
  * Effort lives in the model slug suffix (…-low|medium|high); passing --effort as well is rejected.
Provenance and refresh: agy_call.VENDOR.md.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import stat
import subprocess
import sys
import tempfile

DEFAULT_MODEL = "gemini-3.8-flash-medium"
DEFAULT_TIMEOUT = "5m"
CANDIDATES = (
    os.path.join(os.environ.get("LOCALAPPDATA", ""), "agy", "bin", "agy.exe"),
    os.path.expanduser("~/.local/bin/agy"),
)
LARGE_FILE_MB = 1024


def die(msg, code=1):
    print(f"agy_call: ERROR: {msg}", file=sys.stderr)
    sys.exit(code)


def find_agy():
    for c in CANDIDATES:
        if c and os.path.isfile(c):
            return c
    found = shutil.which("agy")
    if found:
        return found
    die("Antigravity CLI not found. Install: irm https://antigravity.google/cli/install.ps1 | iex "
        "(Windows) or curl -fsSL https://antigravity.google/cli/install.sh | bash, then run `agy` once to sign in.")


def build_prompt(prompt, staged):
    """Name each staged file by absolute path, up front, so the agent opens it instead of searching."""
    if not staged:
        return prompt
    lines = ["Open and view these files with your file-viewing tool (absolute paths; do not search, do not run shell commands):"]
    lines += [f"- {p}" for p in staged]
    return "\n".join(lines) + "\n\n" + prompt


def build_command(agy, prompt, workdir, schema, model, timeout, allow_commands):
    cmd = [agy, "-p", prompt, "--output-format", "json", "--print-timeout", timeout,
           "--model", model, "--disable-slash-commands", "--add-dir", workdir]
    if schema:
        cmd += ["--json-schema", schema]
    if allow_commands:
        cmd += ["--dangerously-skip-permissions"]
    return cmd


def stage(files, workdir):
    """Read-only copies in the disposable workspace; the originals are never in the agent's reach."""
    staged = []
    for f in files:
        src = os.path.abspath(f)
        os.path.isfile(src) or die(f"file not found: {src}")
        size_mb = os.path.getsize(src) / 1e6
        if size_mb > LARGE_FILE_MB:
            print(f"[agy_call] staging {size_mb:.0f} MB copy of {os.path.basename(src)}; pass a proxy if this is routine",
                  file=sys.stderr)
        dst = os.path.join(workdir, os.path.basename(src))
        n = 1
        while os.path.exists(dst):
            root, ext = os.path.splitext(os.path.basename(src))
            dst = os.path.join(workdir, f"{root}-{n}{ext}")
            n += 1
        shutil.copy2(src, dst)
        os.chmod(dst, stat.S_IREAD)
        staged.append(dst)
    return staged


def _rm_rw(func, path, _):
    os.chmod(path, stat.S_IWRITE)
    func(path)


def run(prompt, files=(), schema=None, model=DEFAULT_MODEL, timeout=DEFAULT_TIMEOUT, allow_commands=False):
    """Return the parsed envelope dict (status, response, structured_output, usage, denied_actions, ...)."""
    agy = find_agy()
    if schema and os.path.isfile(schema):
        schema = open(schema, encoding="utf-8").read()
    workdir = tempfile.mkdtemp(prefix="agy-call-")
    try:
        staged = stage(files, workdir)
        cmd = build_command(agy, build_prompt(prompt, staged), workdir, schema, model, timeout, allow_commands)
        r = subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", cwd=workdir)
        leftovers = sorted(set(os.listdir(workdir)) - {os.path.basename(p) for p in staged})
    finally:
        shutil.rmtree(workdir, onerror=_rm_rw)
    try:
        env = json.loads(r.stdout or "{}")
    except json.JSONDecodeError:
        env = {"status": "PARSE_ERROR", "response": r.stdout[:2000]}
    env["exit_code"] = r.returncode
    env["stderr_tail"] = (r.stderr or "").strip()[-600:]
    env["model"] = model
    env["workspace_leftovers"] = leftovers   # files the agent created in the disposable workspace
    return env


def main():
    ap = argparse.ArgumentParser(description="Delegate a bounded task to Gemini via the Antigravity CLI (print mode).")
    ap.add_argument("--prompt", help="the task; or read from --prompt-file")
    ap.add_argument("--prompt-file")
    ap.add_argument("--file", action="append", default=[], help="media or document to view (copied read-only into a disposable workspace)")
    ap.add_argument("--schema", help="JSON schema string or path; response is constrained to it")
    ap.add_argument("--model", default=DEFAULT_MODEL, help="agy model slug, effort in the suffix (see `agy models`)")
    ap.add_argument("--timeout", default=DEFAULT_TIMEOUT, help="print-mode wait, e.g. 5m")
    ap.add_argument("--allow-commands", action="store_true", help="let the agent run shell commands (off by default)")
    ap.add_argument("--out", help="write the full JSON envelope here")
    args = ap.parse_args()
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8")
        except Exception:
            pass
    prompt = args.prompt or (open(args.prompt_file, encoding="utf-8").read() if args.prompt_file else None)
    prompt or die("--prompt or --prompt-file is required")
    env = run(prompt, args.file, args.schema, args.model, args.timeout, args.allow_commands)
    if args.out:
        with open(args.out, "w", encoding="utf-8", newline="\n") as fh:
            json.dump(env, fh, indent=1, ensure_ascii=False)
    if env.get("status") != "SUCCESS":
        print(f"agy_call: status {env.get('status')} (exit {env.get('exit_code')}); {env.get('error') or env.get('stderr_tail')}", file=sys.stderr)
    out = env.get("structured_output")
    print(json.dumps(out, ensure_ascii=False) if out is not None else (env.get("response") or ""))
    usage = env.get("usage") or {}
    print(f"[agy_call] {env.get('model')} · {usage.get('total_tokens', '?')} tokens · {env.get('duration_seconds', '?')}s"
          + (f" · denied: {[d.get('display_name') for d in env.get('denied_actions') or []]}" if env.get("denied_actions") else "")
          + (f" · leftovers discarded: {env['workspace_leftovers']}" if env.get("workspace_leftovers") else ""), file=sys.stderr)
    sys.exit(0 if env.get("status") == "SUCCESS" else 2)


if __name__ == "__main__":
    main()
