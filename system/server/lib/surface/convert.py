"""LibreOffice headless PPTX/DOCX -> cached PDF.

Cache key covers path, mtime, size, and converter version, so an edited deck
reconverts and an unchanged one never does. Conversion happens in a per-key
temp dir to keep concurrent requests from clobbering each other's output.
"""

from __future__ import annotations

import hashlib
import shutil
import subprocess
import threading
from pathlib import Path

CONVERTER_VERSION = 1
CONVERT_TIMEOUT_S = 120

# Serializes conversions: two requests for the same (or different) files can
# arrive from the thread pool; LibreOffice instances also fight over the same
# user profile when run concurrently.
_CONVERT_LOCK = threading.Lock()

SOFFICE_CANDIDATES = (
    r"C:\Program Files\LibreOffice\program\soffice.exe",
    r"C:\Program Files (x86)\LibreOffice\program\soffice.exe",
    "/usr/bin/soffice",
    "/Applications/LibreOffice.app/Contents/MacOS/soffice",
)


class ConversionError(RuntimeError):
    pass


def find_soffice() -> str | None:
    found = shutil.which("soffice")
    if found:
        return found
    for candidate in SOFFICE_CANDIDATES:
        if Path(candidate).is_file():
            return candidate
    return None


def cache_path(src: Path, cache_dir: Path) -> Path:
    src = Path(src)
    st = src.stat()
    key = hashlib.md5(
        f"{src.resolve()}|{st.st_mtime_ns}|{st.st_size}|v{CONVERTER_VERSION}".encode()
    ).hexdigest()[:20]
    return Path(cache_dir) / f"{src.stem}-{key}.pdf"


def convert_to_pdf(src: Path, cache_dir: Path, *, soffice: str | None = "auto") -> Path:
    """Convert src to PDF in cache_dir; cache hit returns without converting."""
    src = Path(src)
    cache_dir = Path(cache_dir)
    target = cache_path(src, cache_dir)
    if target.is_file():
        return target

    with _CONVERT_LOCK:
        return _convert_locked(src, cache_dir, target, soffice)


def _convert_locked(src: Path, cache_dir: Path, target: Path, soffice: str | None) -> Path:
    if target.is_file():  # a queued duplicate request finished while we waited
        return target
    if soffice == "auto":
        soffice = find_soffice()
    if not soffice:
        raise ConversionError(
            "LibreOffice not found — install it (soffice must be on PATH or in the "
            "default install location) to preview PPTX/DOCX files"
        )

    workdir = cache_dir / f"tmp-{target.stem}"
    workdir.mkdir(parents=True, exist_ok=True)
    try:
        proc = subprocess.run(
            [soffice, "--headless", "--norestore", "--convert-to", "pdf",
             "--outdir", str(workdir), str(src)],
            capture_output=True,
            timeout=CONVERT_TIMEOUT_S,
        )
        produced = workdir / f"{src.stem}.pdf"
        if proc.returncode != 0 or not produced.is_file():
            detail = (proc.stderr or proc.stdout or b"").decode(errors="replace").strip()
            raise ConversionError(f"LibreOffice conversion failed for {src.name}: {detail or 'no output produced'}")
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(produced), str(target))
        return target
    except FileNotFoundError as exc:
        raise ConversionError(f"LibreOffice not runnable at '{soffice}': {exc}") from exc
    except subprocess.TimeoutExpired as exc:
        raise ConversionError(f"LibreOffice conversion timed out for {src.name}") from exc
    finally:
        shutil.rmtree(workdir, ignore_errors=True)
