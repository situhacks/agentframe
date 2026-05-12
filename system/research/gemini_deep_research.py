"""Gemini Deep Research runner and output normalizer.

The campaign artifact stays Markdown for downstream agents. The native Gemini
interaction JSON is preserved as source material for audit and re-extraction.
"""

from __future__ import annotations

import argparse
import base64
import json
import os
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Mapping

PROJECT_ROOT = Path(__file__).resolve().parents[2]
INTERACTIONS_ENDPOINT = "https://generativelanguage.googleapis.com/v1beta/interactions"


class ExtractionError(RuntimeError):
    """Raised when a Gemini interaction cannot be turned into a report."""


@dataclass(frozen=True)
class ExtractedInteraction:
    report_text: str
    extracted_text_paths: list[str]
    generated_media_paths: list[str]


@dataclass(frozen=True)
class ArtifactWriteResult:
    artifact_path: Path
    raw_path: Path
    metadata_path: Path
    extracted: ExtractedInteraction


def _utc_now() -> str:
    return datetime.now(timezone.utc).replace(microsecond=0).isoformat()


def _posix(path: str | Path) -> str:
    return str(path).replace("\\", "/")


def _load_env_file(env_path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not env_path.exists():
        return values
    for line in env_path.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if not stripped or stripped.startswith("#") or "=" not in stripped:
            continue
        key, value = stripped.split("=", 1)
        values[key.strip()] = value.strip().strip('"').strip("'")
    return values


def load_gemini_api_key(project_root: Path = PROJECT_ROOT) -> str:
    env_values = _load_env_file(project_root / ".env")
    api_key = env_values.get("GEMINI_API_KEY") or os.environ.get("GEMINI_API_KEY")
    if not api_key:
        raise RuntimeError("GEMINI_API_KEY is missing or empty.")
    return api_key


def _request_json(
    *,
    method: str,
    url: str,
    api_key: str,
    payload: Mapping[str, Any] | None = None,
    timeout_seconds: float = 90,
) -> dict[str, Any]:
    data = None if payload is None else json.dumps(payload).encode("utf-8")
    request = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers={"Content-Type": "application/json", "x-goog-api-key": api_key},
    )
    try:
        with urllib.request.urlopen(request, timeout=timeout_seconds) as response:
            return json.loads(response.read().decode("utf-8"))
    except urllib.error.HTTPError as error:
        body = error.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"Gemini Interactions API HTTP {error.code}: {body}") from error


def start_interaction(
    *,
    api_key: str,
    prompt: str,
    previous_interaction_id: str | None = None,
    collaborative_planning: bool,
    agent: str = "deep-research-preview-04-2026",
) -> dict[str, Any]:
    body: dict[str, Any] = {
        "agent": agent,
        "input": prompt,
        "agent_config": {
            "type": "deep-research",
            "thinking_summaries": "auto",
            "collaborative_planning": collaborative_planning,
        },
        "background": True,
    }
    if previous_interaction_id:
        body["previous_interaction_id"] = previous_interaction_id
    return _request_json(
        method="POST",
        url=INTERACTIONS_ENDPOINT,
        api_key=api_key,
        payload=body,
    )


def poll_interaction(
    *,
    api_key: str,
    interaction_id: str,
    poll_interval_seconds: float = 10,
    max_polls: int = 180,
) -> dict[str, Any]:
    result: dict[str, Any] = {"id": interaction_id, "status": "unknown"}
    for _ in range(max_polls):
        result = _request_json(
            method="GET",
            url=f"{INTERACTIONS_ENDPOINT}/{interaction_id}",
            api_key=api_key,
        )
        if result.get("status") in {"completed", "failed", "cancelled"}:
            return result
        time.sleep(poll_interval_seconds)
    return result


def _extract_output_texts(interaction: Mapping[str, Any]) -> tuple[list[str], list[str]]:
    texts: list[str] = []
    paths: list[str] = []
    outputs = interaction.get("outputs")
    if isinstance(outputs, list):
        for index, output in enumerate(outputs):
            if not isinstance(output, Mapping):
                continue
            text = output.get("text")
            output_type = output.get("type")
            if isinstance(text, str) and text.strip() and output_type in {None, "text"}:
                texts.append(text.strip())
                paths.append(f"outputs[{index}].text")
    if texts:
        return texts, paths

    steps = interaction.get("steps")
    if isinstance(steps, list):
        for step_index, step in enumerate(steps):
            if not isinstance(step, Mapping):
                continue
            content = step.get("content")
            if not isinstance(content, list):
                continue
            for content_index, item in enumerate(content):
                if not isinstance(item, Mapping):
                    continue
                text = item.get("text")
                if isinstance(text, str) and text.strip():
                    texts.append(text.strip())
                    paths.append(
                        f"steps[{step_index}].content[{content_index}].text"
                    )
    return texts, paths


def _media_extension(mime_type: str) -> str | None:
    return {
        "image/png": "png",
        "image/jpeg": "jpg",
        "image/webp": "webp",
    }.get(mime_type)


def _extract_output_media(
    interaction: Mapping[str, Any],
    source_dir: Path,
    *,
    source_material_prefix: str,
) -> list[str]:
    generated: list[str] = []
    outputs = interaction.get("outputs")
    if not isinstance(outputs, list):
        return generated

    source_dir.mkdir(parents=True, exist_ok=True)
    for index, output in enumerate(outputs):
        if not isinstance(output, Mapping) or output.get("type") != "image":
            continue
        mime_type = output.get("mime_type")
        data = output.get("data")
        if not isinstance(mime_type, str) or not isinstance(data, str):
            continue
        extension = _media_extension(mime_type)
        if extension is None:
            continue
        filename = f"gemini-deep-research-output-{index}.{extension}"
        (source_dir / filename).write_bytes(base64.b64decode(data))
        generated.append(_posix(Path(source_material_prefix) / filename))
    return generated


def extract_interaction_outputs(
    interaction: Mapping[str, Any],
    source_dir: Path,
    *,
    source_material_prefix: str = "source-material",
) -> ExtractedInteraction:
    text_parts, text_paths = _extract_output_texts(interaction)
    media_paths = _extract_output_media(
        interaction,
        source_dir,
        source_material_prefix=source_material_prefix,
    )
    report_text = "\n\n".join(text_parts).strip()
    if not report_text:
        raise ExtractionError(
            "No report text found in Gemini interaction. Raw JSON was preserved; "
            "inspect outputs[] or steps[] before retrying extraction."
        )
    return ExtractedInteraction(
        report_text=report_text,
        extracted_text_paths=text_paths,
        generated_media_paths=media_paths,
    )


def _frontmatter_source_rows(raw_path: str, metadata_path: str, media_paths: list[str]) -> str:
    rows = [
        f"  - {{path: {raw_path}, kind: raw_export}}",
        f"  - {{path: {metadata_path}, kind: raw_export}}",
    ]
    rows.extend(f"  - {{path: {path}, kind: raw_export}}" for path in media_paths)
    return "\n".join(rows)


def write_research_artifact(
    interaction: Mapping[str, Any],
    research_dir: Path,
    *,
    title: str,
    raw_filename: str,
    metadata_filename: str,
    artifact_filename: str,
    last_updated: str | None = None,
) -> ArtifactWriteResult:
    research_dir.mkdir(parents=True, exist_ok=True)
    source_dir = research_dir / "source-material"
    source_dir.mkdir(parents=True, exist_ok=True)

    raw_path = source_dir / raw_filename
    metadata_path = source_dir / metadata_filename
    artifact_path = research_dir / artifact_filename
    raw_path.write_text(json.dumps(interaction, indent=2, ensure_ascii=False), encoding="utf-8")

    extracted = extract_interaction_outputs(interaction, source_dir)
    raw_rel = _posix(Path("source-material") / raw_filename)
    metadata_rel = _posix(Path("source-material") / metadata_filename)
    metadata = {
        "interaction_id": interaction.get("id"),
        "status": interaction.get("status"),
        "error": interaction.get("error"),
        "usage": interaction.get("usage") or {},
        "created_at_utc": _utc_now(),
        "extracted_text_paths": extracted.extracted_text_paths,
        "generated_images": extracted.generated_media_paths,
        "report_chars": len(extracted.report_text),
    }
    metadata_path.write_text(json.dumps(metadata, indent=2, ensure_ascii=False), encoding="utf-8")

    timestamp = last_updated or _utc_now()
    artifact = (
        "---\n"
        "status: drafting\n"
        f"last_updated: {timestamp}\n"
        "current_version: 1\n"
        "version_history:\n"
        "  - {v: 1, date: 2026-05-09, note: \"Initial Gemini Deep Research API output\"}\n"
        "research_method: gemini_deep_research_api\n"
        "source_material:\n"
        f"{_frontmatter_source_rows(raw_rel, metadata_rel, extracted.generated_media_paths)}\n"
        "---\n\n"
        f"# {title}\n\n"
        "> Gemini Deep Research API output. Review before locking or moving to Phase 2.\n\n"
        f"{extracted.report_text}\n"
    )
    artifact_path.write_text(artifact, encoding="utf-8")
    return ArtifactWriteResult(
        artifact_path=artifact_path,
        raw_path=raw_path,
        metadata_path=metadata_path,
        extracted=extracted,
    )


def _interaction_id(response: Mapping[str, Any]) -> str:
    raw = response.get("id") or response.get("name") or response.get("interaction_id")
    if not isinstance(raw, str) or not raw.strip():
        raise RuntimeError("Gemini Interactions API did not return an interaction id.")
    return raw


def run_deep_research(
    *,
    prompt: str,
    research_dir: Path,
    title: str,
    previous_interaction_id: str | None,
    collaborative_planning: bool,
    artifact_filename: str,
    raw_filename: str,
    metadata_filename: str,
    agent: str = "deep-research-preview-04-2026",
    project_root: Path = PROJECT_ROOT,
) -> ArtifactWriteResult:
    api_key = load_gemini_api_key(project_root)
    created = start_interaction(
        api_key=api_key,
        prompt=prompt,
        previous_interaction_id=previous_interaction_id,
        collaborative_planning=collaborative_planning,
        agent=agent,
    )
    interaction = poll_interaction(api_key=api_key, interaction_id=_interaction_id(created))
    if interaction.get("status") != "completed":
        source_dir = research_dir / "source-material"
        source_dir.mkdir(parents=True, exist_ok=True)
        (source_dir / raw_filename).write_text(
            json.dumps(interaction, indent=2, ensure_ascii=False),
            encoding="utf-8",
        )
        raise RuntimeError(
            f"Gemini Deep Research ended with status={interaction.get('status')!r}."
        )
    return write_research_artifact(
        interaction,
        research_dir,
        title=title,
        raw_filename=raw_filename,
        metadata_filename=metadata_filename,
        artifact_filename=artifact_filename,
    )


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Run Gemini Deep Research for Phase 1.")
    parser.add_argument("--research-dir", required=True)
    parser.add_argument("--prompt-file", required=True)
    parser.add_argument("--title", required=True)
    parser.add_argument("--previous-interaction-id")
    parser.add_argument("--agent", default="deep-research-preview-04-2026")
    parser.add_argument(
        "--mode",
        choices=["plan", "run"],
        required=True,
        help="'plan' uses collaborative planning; 'run' executes the full report.",
    )
    parser.add_argument("--artifact-filename", default="research-artifact-vF.md")
    parser.add_argument("--raw-filename", default="gemini-deep-research-response.json")
    parser.add_argument("--metadata-filename", default="gemini-deep-research-metadata.json")
    return parser


def main() -> int:
    args = _build_parser().parse_args()
    result = run_deep_research(
        prompt=Path(args.prompt_file).read_text(encoding="utf-8"),
        research_dir=Path(args.research_dir),
        title=args.title,
        previous_interaction_id=args.previous_interaction_id,
        collaborative_planning=args.mode == "plan",
        artifact_filename=args.artifact_filename,
        raw_filename=args.raw_filename,
        metadata_filename=args.metadata_filename,
        agent=args.agent,
    )
    print(
        json.dumps(
            {
                "artifact": _posix(result.artifact_path),
                "raw": _posix(result.raw_path),
                "metadata": _posix(result.metadata_path),
                "report_chars": len(result.extracted.report_text),
                "generated_media": result.extracted.generated_media_paths,
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
