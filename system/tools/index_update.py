"""The batch run's last step: `af index update`, so the search index is fresh after every run.

No model call: the index embeds with the local model or falls back to keywords. It prints the last
line `af` printed and exits with `af`'s exit code. The batch run (watch_batch.py) starts it as a
step with a 240-second limit and kills this process and `af` with it past that.

    python system/tools/index_update.py
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path


def main() -> int:
    root = Path(os.environ.get("AF_ROOT") or Path(__file__).resolve().parent.parent.parent)
    try:
        r = subprocess.run(["af", "index", "update"], capture_output=True, text=True, encoding="utf-8", errors="replace",
                           env={**os.environ, "AF_ROOT": str(root)}, cwd=root)
    except OSError as e:
        print(f"af could not be started: {e}")
        return 127
    lines = [x for x in (r.stdout + "\n" + r.stderr).splitlines() if x.strip()]
    print(lines[-1] if lines else "")
    return r.returncode


if __name__ == "__main__":
    sys.exit(main())
