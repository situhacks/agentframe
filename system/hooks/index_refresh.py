"""Shim for chats wired before the af cutover: forwards the hook to `af hook`. Deleted next release."""
import os, subprocess, sys
data = sys.stdin.buffer.read() if not sys.stdin.isatty() else b""
root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
event = "session-start"
sys.exit(subprocess.run(["af", "hook", event], input=data, env=dict(os.environ, AF_ROOT=root)).returncode)
