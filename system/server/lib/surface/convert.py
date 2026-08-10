"""Native Microsoft Office PPTX/DOCX -> cached PDF.

Rendering goes through the installed desktop application over COM
(``system/tools/office_render.ps1``), never through LibreOffice. LibreOffice
substitutes fonts it cannot resolve: on a real client deck it rendered Calibri
as Cooper Black, which widened every headline ~40%, reflowed it to three lines,
overflowed its text box, and clipped text mid-word. A preview that quietly
lies about the deck is worse than no preview, so there is no fallback — when
PowerPoint/Word is absent the conversion fails with instructions.

Cache key covers path, mtime, size, and converter version, so an edited deck
reconverts and an unchanged one never does. Conversion happens in a per-key
temp dir to keep concurrent requests from clobbering each other's output.
"""

from __future__ import annotations

import hashlib
import os
import shutil
import subprocess
import threading
from pathlib import Path

# v2 retires every PDF cached from the LibreOffice era so a stale mangled
# render can never be served from disk after the upgrade.
CONVERTER_VERSION = 2
CONVERT_TIMEOUT_S = 180

ROOT = Path(__file__).resolve().parents[4]
RENDERER = ROOT / "system" / "tools" / "office_render.ps1"

POWERPOINT_SUFFIXES = frozenset({".pptx", ".ppt", ".pptm", ".ppsx"})
WORD_SUFFIXES = frozenset({".docx", ".doc", ".docm", ".rtf"})
SUPPORTED_SUFFIXES = POWERPOINT_SUFFIXES | WORD_SUFFIXES

# Serializes conversions: requests arrive from the thread pool, and a single
# Office automation server should drive one document at a time.
_CONVERT_LOCK = threading.Lock()

_REQUIREMENT = (
    "AgentFrame renders Office files with the installed Microsoft Office desktop "
    "app and has no LibreOffice fallback (LibreOffice substitutes fonts and "
    "corrupts the render). Install PowerPoint/Word on this machine, or keep the "
    "deliverable as HTML."
)


class ConversionError(RuntimeError):
    pass


def find_powershell() -> str | None:
    for name in ("powershell.exe", "pwsh.exe", "powershell", "pwsh"):
        found = shutil.which(name)
        if found:
            return found
    return None


def cache_path(src: Path, cache_dir: Path) -> Path:
    src = Path(src)
    st = src.stat()
    key = hashlib.md5(
        f"{src.resolve()}|{st.st_mtime_ns}|{st.st_size}|v{CONVERTER_VERSION}".encode()
    ).hexdigest()[:20]
    return Path(cache_dir) / f"{src.stem}-{key}.pdf"


def convert_to_pdf(src: Path, cache_dir: Path, *, renderer: str | None = "auto") -> Path:
    """Convert src to PDF in cache_dir; cache hit returns without converting."""
    src = Path(src)
    cache_dir = Path(cache_dir)
    target = cache_path(src, cache_dir)
    if target.is_file():
        return target

    with _CONVERT_LOCK:
        return _convert_locked(src, cache_dir, target, renderer)


def _convert_locked(
    src: Path, cache_dir: Path, target: Path, renderer: str | None
) -> Path:
    if target.is_file():  # a queued duplicate request finished while we waited
        return target

    suffix = src.suffix.lower()
    if suffix not in SUPPORTED_SUFFIXES:
        raise ConversionError(f"{src.name}: not a PowerPoint or Word file")

    if renderer == "auto":
        renderer = str(RENDERER) if RENDERER.is_file() else None
    if not renderer:
        raise ConversionError(
            f"Native Office renderer is missing (expected {RENDERER}). {_REQUIREMENT}"
        )

    if os.name != "nt":
        raise ConversionError(
            f"Cannot convert {src.name}: Office COM automation needs Windows. "
            f"{_REQUIREMENT}"
        )

    powershell = find_powershell()
    if not powershell:
        raise ConversionError(
            f"Cannot convert {src.name}: PowerShell was not found on PATH, so the "
            f"native Office renderer cannot be launched. {_REQUIREMENT}"
        )

    workdir = cache_dir / f"tmp-{target.stem}"
    workdir.mkdir(parents=True, exist_ok=True)
    produced = workdir / f"{src.stem}.pdf"
    try:
        proc = subprocess.run(
            [
                powershell, "-NoProfile", "-NonInteractive",
                "-ExecutionPolicy", "Bypass", "-File", str(renderer),
                "pdf", "-Source", str(src), "-Output", str(produced), "-Force",
            ],
            capture_output=True,
            timeout=CONVERT_TIMEOUT_S,
        )
        if proc.returncode != 0 or not produced.is_file():
            detail = (proc.stderr or proc.stdout or b"").decode(errors="replace").strip()
            app = "PowerPoint" if suffix in POWERPOINT_SUFFIXES else "Word"
            raise ConversionError(
                f"{app} could not convert {src.name}: {detail or 'no output produced'}"
            )
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(produced), str(target))
        return target
    except FileNotFoundError as exc:
        raise ConversionError(
            f"Native Office renderer not runnable ('{powershell}'): {exc}"
        ) from exc
    except subprocess.TimeoutExpired as exc:
        raise ConversionError(
            f"Office conversion timed out after {CONVERT_TIMEOUT_S}s for {src.name}"
        ) from exc
    finally:
        shutil.rmtree(workdir, ignore_errors=True)
