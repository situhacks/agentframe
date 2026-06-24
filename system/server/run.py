"""AgentFrame Marketing preview server.

Serves the project root over `http://localhost:8080` and pushes browser
refreshes via LiveReload whenever files in the configured watch globs
change.

Usage:
    python system/server/run.py
    python system/server/run.py --port 8081
    python system/server/run.py --campaign marketingos
"""

from __future__ import annotations

import argparse
import sys
import webbrowser
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[2]

if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

CONFIG_PATH = PROJECT_ROOT / "system" / "server" / "config.yaml"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="AgentFrame Marketing preview server")
    parser.add_argument("--port", type=int, default=None, help="Port (default 8080 from config)")
    parser.add_argument("--host", default=None, help="Host (default localhost from config)")
    parser.add_argument(
        "--campaign",
        default=None,
        help="Open the browser to this campaign's design-language preview after start",
    )
    parser.add_argument(
        "--no-open",
        action="store_true",
        help="Do not open a browser tab even if --campaign is set",
    )
    return parser.parse_args()


def load_config() -> dict:
    if not CONFIG_PATH.exists():
        return {}
    import yaml

    return yaml.safe_load(CONFIG_PATH.read_text(encoding="utf-8")) or {}


def main() -> int:
    args = parse_args()
    cfg = load_config()

    port = args.port or int(cfg.get("port", 8080))
    host = args.host or str(cfg.get("host", "localhost"))
    delay = float(cfg.get("debounce_delay_seconds", 0.5))

    from system.server.lib import server as server_lib
    from system.server.lib import watcher

    globs = watcher.load_globs(CONFIG_PATH)
    exclude_globs = watcher.load_exclude_globs(CONFIG_PATH)

    if args.campaign and not args.no_open:
        url = (
            f"http://{host}:{port}/workspace/projects/{args.campaign}"
            "/phase-3-planning/design-language/preview/design-language.html"
        )
        try:
            webbrowser.open_new_tab(url)
        except Exception:
            pass

    print(f"[preview] Serving {PROJECT_ROOT} at http://{host}:{port}")
    print(f"[preview] Hub:      http://{host}:{port}/")
    print(f"[preview] Watching {len(globs)} glob(s):")
    for g in globs:
        print(f"  - {g}")
    if exclude_globs:
        print(f"[preview] Excluding {len(exclude_globs)} hub glob(s):")
        for g in exclude_globs:
            print(f"  - {g}")
    print("[preview] LiveReload on port 35729 (auto-injected into served HTML).")
    print("[preview] Ctrl+C to stop.\n", flush=True)

    server_lib.serve(
        PROJECT_ROOT,
        port=port,
        host=host,
        watch_globs=globs,
        exclude_globs=exclude_globs,
        delay=delay,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
