"""Shim for chats wired before the af cutover: forwards the hook to `af hook`. Deleted next release."""
import json, os, subprocess, sys
data = sys.stdin.buffer.read()
try: ev = json.loads(data or b"{}").get("hook_event_name", "")
except Exception: ev = ""
root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
event = "post-tool-use" if ev == "PostToolUse" else "pre-tool-use"
sys.exit(subprocess.run(["af", "hook", event], input=data, env=dict(os.environ, AF_ROOT=root)).returncode)
