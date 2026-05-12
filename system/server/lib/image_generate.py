"""Generate Nano Banana image variants with reproducible sidecar metadata."""

from __future__ import annotations

import argparse
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

PROJECT_ROOT = Path(__file__).resolve().parents[3]
DEFAULT_MODEL = "gemini-3.1-flash-image-preview"
DEFAULT_ASPECT = "4:5"
DEFAULT_VARIANTS = 3
MODEL_ALIASES = {
    "nb2": "gemini-3.1-flash-image-preview",
    "pro": "gemini-3-pro-image-preview",
}
VARIANT_LETTERS = "abcdefghijklmnopqrstuvwxyz"


def _read_env_file(path: Path) -> dict[str, str]:
    if not path.exists():
        return {}
    out: dict[str, str] = {}
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        k, _, v = line.partition("=")
        k = k.strip()
        v = v.strip().strip('"').strip("'")
        if k:
            out[k] = v
    return out


def _load_env() -> dict[str, str]:
    merged: dict[str, str] = {}
    merged.update(_read_env_file(PROJECT_ROOT / ".env"))
    merged.update(os.environ)
    return merged


def build_client(env: dict[str, str]):
    from google import genai

    api_key = env.get("GEMINI_API_KEY") or env.get("GOOGLE_API_KEY")
    if not api_key:
        raise RuntimeError("No Gemini credentials found. Set GEMINI_API_KEY.")
    return genai.Client(api_key=api_key)


def _resolve_model(value: str) -> str:
    return MODEL_ALIASES.get(value.strip().lower(), value)


def _read_prompt(args: argparse.Namespace) -> str:
    if args.prompt and args.prompt_file:
        raise SystemExit("Pass either --prompt OR --prompt-file, not both.")
    if args.prompt_file:
        return Path(args.prompt_file).read_text(encoding="utf-8").strip()
    if args.prompt:
        return args.prompt
    raise SystemExit("Pass --prompt or --prompt-file.")


_ASPECT_CLAUSES = {
    "1:1": "Square 1:1 composition.",
    "4:5": "Tall 4:5 vertical composition, suitable for a LinkedIn carousel slide.",
    "3:4": "Tall 3:4 vertical composition.",
    "9:16": "Tall 9:16 vertical composition, suitable for a phone screen.",
    "16:9": "Wide 16:9 horizontal composition.",
    "4:3": "Wide 4:3 horizontal composition.",
}


def _compose_prompt(prompt: str, aspect: str, negative_prompt: str) -> str:
    parts = [prompt.strip()]
    aspect_clause = _ASPECT_CLAUSES.get(aspect)
    if aspect_clause:
        parts.append(aspect_clause)
    elif aspect:
        parts.append(f"Aspect ratio {aspect}.")
    if negative_prompt:
        parts.append(f"Avoid: {negative_prompt.strip()}.")
    return "\n".join(parts)


def _extract_image_bytes(response) -> bytes | None:
    candidates = getattr(response, "candidates", None) or []
    for cand in candidates:
        content = getattr(cand, "content", None)
        parts = getattr(content, "parts", None) or []
        for part in parts:
            inline = getattr(part, "inline_data", None)
            if inline is None:
                continue
            data = getattr(inline, "data", None)
            if data:
                return data
    return None


def _extract_text(response) -> str:
    candidates = getattr(response, "candidates", None) or []
    chunks: list[str] = []
    for cand in candidates:
        content = getattr(cand, "content", None)
        parts = getattr(content, "parts", None) or []
        for part in parts:
            text = getattr(part, "text", None)
            if text:
                chunks.append(text)
    return " ".join(chunks).strip()


def _display_path(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(PROJECT_ROOT))
    except ValueError:
        return str(path.resolve())


def _archive_existing_variants(out_dir: Path) -> str | None:
    existing = sorted(out_dir.glob("image-variant-*.png"))
    if not existing:
        return None
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    target_dir = out_dir / "history" / stamp
    target_dir.mkdir(parents=True, exist_ok=True)
    for file_path in existing:
        file_path.replace(target_dir / file_path.name)
    return _display_path(target_dir)


def generate(
    *,
    prompt: str,
    out_dir: Path,
    variants: int = DEFAULT_VARIANTS,
    aspect: str = DEFAULT_ASPECT,
    negative_prompt: str = "",
    model: str = DEFAULT_MODEL,
    env: dict[str, str] | None = None,
) -> dict:
    if variants < 1 or variants > len(VARIANT_LETTERS):
        raise ValueError(f"variants must be 1..{len(VARIANT_LETTERS)}")

    env = env if env is not None else _load_env()
    out_dir.mkdir(parents=True, exist_ok=True)
    archived_dir = _archive_existing_variants(out_dir)
    client = build_client(env)
    full_prompt = _compose_prompt(prompt, aspect, negative_prompt)

    written: list[str] = []
    refusals: list[str] = []
    for i in range(variants):
        response = client.models.generate_content(
            model=model,
            contents=[full_prompt],
        )
        image_bytes = _extract_image_bytes(response)
        if image_bytes is None:
            refusals.append(_extract_text(response) or "(no text returned)")
            continue

        letter = VARIANT_LETTERS[i]
        target = out_dir / f"image-variant-{letter}.png"
        target.write_bytes(image_bytes)
        written.append(_display_path(target))

    if not written:
        joined = " | ".join(refusals[:3]) if refusals else "no response detail"
        raise RuntimeError(
            f"Nano Banana returned 0 images across {variants} call(s). "
            f"Sample model output: {joined}"
        )

    meta = {
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "model": model,
        "aspect_ratio": aspect,
        "variants_requested": variants,
        "variants_written": len(written),
        "refusals": refusals,
        "negative_prompt": negative_prompt,
        "prompt": prompt,
        "composed_prompt": full_prompt,
        "files": written,
    }
    if archived_dir:
        meta["archived_previous_variants_to"] = archived_dir
    meta_path = out_dir / "_image_meta.json"
    meta_path.write_text(json.dumps(meta, indent=2), encoding="utf-8")
    return {"meta_path": _display_path(meta_path), **meta}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description="Generate Nano Banana image variants for a post.")
    parser.add_argument("--prompt", help="Prompt text (use --prompt-file for long prompts)")
    parser.add_argument("--prompt-file", help="Path to a file containing the prompt")
    parser.add_argument("--negative-prompt", default="", help="Negative prompt clause(s); injected into prompt body")
    parser.add_argument("--out-dir", required=True, help="Directory for image-variant-{a,b,c}.png and _image_meta.json")
    parser.add_argument("--variants", "--n", dest="variants", type=int, default=DEFAULT_VARIANTS)
    parser.add_argument("--aspect", default=DEFAULT_ASPECT, help="e.g. 1:1, 4:5, 16:9; injected into prompt body")
    parser.add_argument(
        "--model",
        default="nb2",
        help="Tier alias (nb2 or pro) or raw model id",
    )
    args = parser.parse_args(argv)

    prompt = _read_prompt(args)
    out_dir = Path(args.out_dir).resolve()
    model = _resolve_model(args.model)
    print(f"[image] model={model} variants={args.variants}")
    print(f"[image] out_dir={_display_path(out_dir)}")

    try:
        result = generate(
            prompt=prompt,
            out_dir=out_dir,
            variants=args.variants,
            aspect=args.aspect,
            negative_prompt=args.negative_prompt,
            model=model,
        )
    except Exception as e:
        print(f"[image] error: {e}", file=sys.stderr)
        return 1

    print(f"[image] wrote {result['variants_written']}/{result['variants_requested']} variant(s) to {out_dir}")
    for f in result["files"]:
        print(f"  - {f}")
    if result["refusals"]:
        print(f"[image] {len(result['refusals'])} call(s) returned no image (likely safety filter).")
    print(f"[image] meta: {result['meta_path']} (model: {result['model']})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
