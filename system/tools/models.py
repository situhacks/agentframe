"""The model ids the headless runners pass to `claude -p`, read from system/models.json.

One file names each model so a change is one line. The tools read `HAIKU` and
`SONNET`; `consolidate-weekly.ps1` reads `.sonnet` from the same file. Which model to choose
and what each is: `library/schema/models.md`.
"""

from __future__ import annotations

import json
from pathlib import Path

MODELS_FILE = Path(__file__).resolve().parent.parent / "models.json"


def load(path: Path = MODELS_FILE) -> dict[str, str]:
    return json.loads(path.read_text(encoding="utf-8"))


_IDS = load()
FABLE = _IDS["fable"]
OPUS = _IDS["opus"]
SONNET = _IDS["sonnet"]
HAIKU = _IDS["haiku"]
ALL = (FABLE, OPUS, SONNET, HAIKU)
