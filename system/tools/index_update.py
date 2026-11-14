"""The batch run's last step: `af index update`, so the search index is fresh after every run.

No model call: the index embeds with the local model or falls back to keywords. It prints af's summary
(the last `af index:` line that is not a warning or an error; else the last line) and, when af also warned,
` · warning: ` and the first 80 characters of the warning. It exits with `af`'s exit code. The batch run (watch_batch.py) starts it as a
step with a 240-second limit and kills this process and `af` with it past that.

    python system/tools/index_update.py
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def summary(lines: list[str]) -> str:
    """The one line the batch report shows. af warns on stderr when embeddings are unavailable, and that
    line comes last; the counts are the `af index:` line that is not a warning or an error."""
    warnings = [x for x in lines if x.startswith("af index:") and "WARNING" in x]
    counts = [x for x in lines if x.startswith("af index:") and "WARNING" not in x and "ERROR" not in x]
    if not counts:
        return lines[-1] if lines else ""
    if not warnings:
        return counts[-1]
    note = warnings[0].removeprefix("af index:").strip()
    return f"{counts[-1]} · warning: {note[:80]}"


def main() -> int:
    root = Path(os.environ.get("AF_ROOT") or Path(__file__).resolve().parent.parent.parent)
    try:
        r = subprocess.run(["af", "index", "update"], capture_output=True, text=True, encoding="utf-8", errors="replace",
                           env={**os.environ, "AF_ROOT": str(root)}, cwd=root)
    except OSError as e:
        print(f"af could not be started: {e}")
        return 127
    lines = [x for x in (r.stdout + "\n" + r.stderr).splitlines() if x.strip()]
    print(summary(lines))
    return r.returncode


if __name__ == "__main__":
    sys.exit(main())
