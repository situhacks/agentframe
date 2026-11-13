"""The model ids the headless runners pass to `claude -p`, read from system/models.json.

One file names each model so a change is one line. The tools read `HAIKU` and
`SONNET`; `consolidate-weekly.ps1` reads `.sonnet` from the same file.
"""

from __future__ import annotations

import json
from pathlib import Path

MODELS_FILE = Path(__file__).resolve().parent.parent / "models.json"


def load(path: Path = MODELS_FILE) -> dict[str, str]:
    return json.loads(path.read_text(encoding="utf-8"))


_IDS = load()
HAIKU = _IDS["haiku"]
SONNET = _IDS["sonnet"]
