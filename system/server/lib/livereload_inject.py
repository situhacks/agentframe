"""Inject the LiveReload client script into HTML strings or files.

The `livereload` Python package auto-injects its client script when serving
HTML over its built-in HTTP server, so this helper is only needed for two
edge cases:

1. The user opens a preview file directly via `file://` in the browser
   (no server in front of it). Auto-injection cannot run, so the agent
   should write the explicit `<script>` tag into the HTML at author time.
2. A custom Tornado handler bypasses the package's static file pipeline.

Default LiveReload port is 35729; the script source is `/livereload.js`
when behind the server, or `http://localhost:35729/livereload.js` when
opened over `file://`.
"""

from __future__ import annotations

import re
from pathlib import Path

LIVERELOAD_PORT = 35729

SERVED_TAG = '<script src="/livereload.js"></script>'
STANDALONE_TAG = (
    f'<script src="http://localhost:{LIVERELOAD_PORT}/livereload.js"></script>'
)

_HEAD_CLOSE_RE = re.compile(r"</head\s*>", re.IGNORECASE)
_HAS_LIVERELOAD_RE = re.compile(r"livereload\.js", re.IGNORECASE)


def inject(html: str, *, standalone: bool = False) -> str:
    """Return `html` with a LiveReload script tag injected before </head>.

    If the script tag is already present, the input is returned unchanged.
    If there is no </head> tag, the script is appended to the end so the
    browser still picks it up.
    """
    if _HAS_LIVERELOAD_RE.search(html):
        return html

    tag = STANDALONE_TAG if standalone else SERVED_TAG

    if _HEAD_CLOSE_RE.search(html):
        return _HEAD_CLOSE_RE.sub(f"  {tag}\n</head>", html, count=1)

    return html + "\n" + tag + "\n"


def inject_file(path: str | Path, *, standalone: bool = True) -> bool:
    """Inject the LiveReload tag into an HTML file on disk in place.

    Returns True if the file was modified, False if it already contained
    a livereload reference.
    """
    p = Path(path)
    original = p.read_text(encoding="utf-8")
    updated = inject(original, standalone=standalone)
    if updated == original:
        return False
    p.write_text(updated, encoding="utf-8")
    return True


__all__ = ["inject", "inject_file", "LIVERELOAD_PORT"]
