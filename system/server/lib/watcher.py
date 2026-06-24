"""Watch-glob registration for the preview server.

Encapsulates the rule set for which files trigger a browser refresh, so
`run.py` stays declarative and the globs can be reused by tooling that
wants to know what the server cares about.
"""

from __future__ import annotations

from pathlib import Path
from typing import Iterable

import yaml

DEFAULT_GLOBS: tuple[str, ...] = (
    "workspace/projects/*/phase-*/**/*.html",
    "workspace/projects/*/phase-*/**/*.css",
    "workspace/projects/*/phase-3-planning/design-language/**",
    "workspace/projects/*/phase-4-production/posts/**/visuals/**",
    "workspace/projects/*/phase-4-production/posts/**/video/**",
    "workspace/projects/*/phase-4-production/posts/**/edit/**",
    "workspace/projects/*/phase-3-planning/design-language/preview/assets/tokens.css",
)
DEFAULT_EXCLUDE_GLOBS: tuple[str, ...] = ()


def load_globs(config_path: str | Path) -> list[str]:
    """Load watch globs from `system/server/config.yaml`.

    Falls back to DEFAULT_GLOBS if the file is missing or empty.
    """
    p = Path(config_path)
    if not p.exists():
        return list(DEFAULT_GLOBS)

    data = yaml.safe_load(p.read_text(encoding="utf-8")) or {}
    globs = data.get("watch_globs") or []
    if not globs:
        return list(DEFAULT_GLOBS)
    return [str(g) for g in globs]


def load_exclude_globs(config_path: str | Path) -> list[str]:
    """Load exclusion globs for hub discovery from `system/server/config.yaml`."""
    p = Path(config_path)
    if not p.exists():
        return list(DEFAULT_EXCLUDE_GLOBS)

    data = yaml.safe_load(p.read_text(encoding="utf-8")) or {}
    globs = data.get("exclude_globs") or []
    if not globs:
        return list(DEFAULT_EXCLUDE_GLOBS)
    return [str(g) for g in globs]


def register(server, globs: Iterable[str], *, delay: float = 0.5) -> None:
    """Register each glob with a `livereload.Server` instance."""
    for g in globs:
        server.watch(g, delay=delay)


__all__ = ["DEFAULT_GLOBS", "DEFAULT_EXCLUDE_GLOBS", "load_globs", "load_exclude_globs", "register"]
