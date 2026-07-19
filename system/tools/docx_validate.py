#!/usr/bin/env python3
"""Run the vendored DOCX validator with a compatible UTF-8 Python runtime."""

import os
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
VALIDATOR = ROOT / "system" / "skills" / "docx" / "scripts" / "office" / "validate.py"


def validator_env():
    env = os.environ.copy()
    env["PYTHONUTF8"] = "1"
    return env


def runtime_candidates():
    """Candidate interpreter commands, ordered from explicit to local fallbacks."""
    candidates = []
    if os.environ.get("AGENTFRAME_DOCX_PYTHON"):
        candidates.append([os.environ["AGENTFRAME_DOCX_PYTHON"]])
    candidates.append([sys.executable])
    if os.name == "nt":
        candidates.extend([["py", "-3.11"], ["py", "-3.12"]])
    return candidates


def compatible_runtime():
    probe = "import defusedxml.minidom, lxml.etree"
    for candidate in runtime_candidates():
        try:
            result = subprocess.run(
                [*candidate, "-c", probe],
                env=validator_env(),
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                check=False,
            )
        except OSError:
            continue
        if result.returncode == 0:
            return candidate
    raise RuntimeError(
        "DOCX validation needs a Python runtime with defusedxml and lxml. "
        "Set AGENTFRAME_DOCX_PYTHON to a compatible interpreter."
    )


def main():
    try:
        runtime = compatible_runtime()
    except RuntimeError as exc:
        print(f"docx_validate: ERROR: {exc}", file=sys.stderr)
        raise SystemExit(2) from exc
    command = [*runtime, str(VALIDATOR), *sys.argv[1:]]
    raise SystemExit(subprocess.run(command, env=validator_env(), check=False).returncode)


if __name__ == "__main__":
    main()
