#!/usr/bin/env python3
"""Scaffold and verify native-first reference deck redesigns.

The tool intentionally uses only the Python standard library. It reads the
OOXML package directly so a freshly pulled AgentFrame copy does not need
python-pptx just to enforce the preservation contract.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import posixpath
import re
import shutil
import sys
import zipfile
from datetime import datetime, timezone
from pathlib import Path
from xml.etree import ElementTree as ET


P_NS = "http://schemas.openxmlformats.org/presentationml/2006/main"
A_NS = "http://schemas.openxmlformats.org/drawingml/2006/main"
R_NS = "http://schemas.openxmlformats.org/officeDocument/2006/relationships"
PKG_REL_NS = "http://schemas.openxmlformats.org/package/2006/relationships"
NS = {"p": P_NS, "a": A_NS, "r": R_NS}
SHAPE_KINDS = {"sp", "pic", "cxnSp", "graphicFrame", "grpSp"}
VALID_MODES = {"preserve", "native", "rebuild"}


class ReferenceDeckError(RuntimeError):
    pass


def _qname(namespace: str, local: str) -> str:
    return f"{{{namespace}}}{local}"


def _local(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _file_sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _canonical_hash(data: bytes | None) -> str:
    if data is None:
        return _sha256(b"")
    try:
        canonical = ET.canonicalize(xml_data=data.decode("utf-8"))
        return _sha256(canonical.encode("utf-8"))
    except (ET.ParseError, UnicodeDecodeError):
        return _sha256(data)


def _normalized_text(value: str) -> str:
    return " ".join(value.split())


def _slide_paths(archive: zipfile.ZipFile) -> list[str]:
    try:
        presentation = ET.fromstring(archive.read("ppt/presentation.xml"))
        relationships = ET.fromstring(
            archive.read("ppt/_rels/presentation.xml.rels")
        )
    except KeyError as exc:
        raise ReferenceDeckError(f"Invalid PPTX package: missing {exc.args[0]}") from exc

    rel_targets = {
        rel.get("Id"): rel.get("Target")
        for rel in relationships.findall(_qname(PKG_REL_NS, "Relationship"))
    }
    paths: list[str] = []
    for slide_id in presentation.findall("./p:sldIdLst/p:sldId", NS):
        rel_id = slide_id.get(_qname(R_NS, "id"))
        target = rel_targets.get(rel_id)
        if not target:
            raise ReferenceDeckError(
                f"Invalid PPTX package: slide relationship {rel_id!r} has no target"
            )
        paths.append(posixpath.normpath(posixpath.join("ppt", target)))
    return paths


def _shape_xfrm(shape: ET.Element) -> dict[str, str | None]:
    candidates = (
        shape.find("./p:spPr/a:xfrm", NS),
        shape.find("./p:grpSpPr/a:xfrm", NS),
        shape.find("./p:xfrm", NS),
    )
    xfrm = next((candidate for candidate in candidates if candidate is not None), None)
    if xfrm is None:
        return {}

    def attrs(child_name: str, names: tuple[str, ...]) -> dict[str, str | None]:
        child = xfrm.find(f"./a:{child_name}", NS)
        return {
            f"{child_name}.{name}": child.get(name) if child is not None else None
            for name in names
        }

    result: dict[str, str | None] = {
        key: xfrm.get(key) for key in ("rot", "flipH", "flipV")
    }
    result.update(attrs("off", ("x", "y")))
    result.update(attrs("ext", ("cx", "cy")))
    result.update(attrs("chOff", ("x", "y")))
    result.update(attrs("chExt", ("cx", "cy")))
    return result


def _shape_text(shape: ET.Element) -> str:
    if _local(shape.tag) == "grpSp":
        return ""
    paragraphs: list[str] = []
    for paragraph in shape.findall(".//a:p", NS):
        text = "".join(node.text or "" for node in paragraph.findall(".//a:t", NS))
        if text:
            paragraphs.append(text)
    return "\n".join(paragraphs)


def _shape_records(root: ET.Element) -> dict[str, dict]:
    records: dict[str, dict] = {}
    for shape in root.iter():
        kind = _local(shape.tag)
        if kind not in SHAPE_KINDS:
            continue
        c_nv_pr = shape.find(".//p:cNvPr", NS)
        if c_nv_pr is None or not c_nv_pr.get("id"):
            continue
        shape_id = c_nv_pr.get("id")
        records[shape_id] = {
            "kind": kind,
            "name": c_nv_pr.get("name", ""),
            "geometry": _shape_xfrm(shape),
            "text": _shape_text(shape),
        }
    return records


def read_deck(path: Path) -> dict:
    path = path.resolve()
    if not path.is_file():
        raise ReferenceDeckError(f"PPTX not found: {path}")
    try:
        with zipfile.ZipFile(path) as archive:
            presentation = ET.fromstring(archive.read("ppt/presentation.xml"))
            size = presentation.find("./p:sldSz", NS)
            slides = []
            for slide_path in _slide_paths(archive):
                raw = archive.read(slide_path)
                root = ET.fromstring(raw)
                filename = posixpath.basename(slide_path)
                rels_path = posixpath.join(
                    posixpath.dirname(slide_path), "_rels", f"{filename}.rels"
                )
                try:
                    rels_raw = archive.read(rels_path)
                except KeyError:
                    rels_raw = None
                shapes = _shape_records(root)
                slides.append(
                    {
                        "path": slide_path,
                        "xml_hash": _canonical_hash(raw),
                        "rels_hash": _canonical_hash(rels_raw),
                        "shapes": shapes,
                        "all_text": "\n".join(
                            record["text"] for record in shapes.values() if record["text"]
                        ),
                    }
                )
    except (zipfile.BadZipFile, ET.ParseError, KeyError) as exc:
        raise ReferenceDeckError(f"Cannot read PPTX package {path}: {exc}") from exc

    return {
        "path": str(path),
        "sha256": _file_sha256(path),
        "slide_size": {
            "cx": size.get("cx") if size is not None else None,
            "cy": size.get("cy") if size is not None else None,
        },
        "slides": slides,
    }


def _slide_title(slide: dict) -> str:
    for record in slide["shapes"].values():
        text = _normalized_text(record["text"])
        if text:
            return text[:100]
    return ""


def scaffold(source: Path, project: Path, *, force: bool = False) -> Path:
    source = source.resolve()
    project = project.resolve()
    manifest_path = project / "reference-redesign.json"
    if manifest_path.exists() and not force:
        raise ReferenceDeckError(
            f"Manifest already exists: {manifest_path} (pass --force to replace it)"
        )

    deck = read_deck(source)
    reference_dir = project / "reference"
    working_dir = project / "working"
    reference_dir.mkdir(parents=True, exist_ok=True)
    working_dir.mkdir(parents=True, exist_ok=True)
    reference_copy = reference_dir / "source.pptx"
    working_copy = working_dir / "working.pptx"
    shutil.copy2(source, reference_copy)
    if force or not working_copy.exists():
        shutil.copy2(source, working_copy)

    manifest = {
        "schema": "agentframe.reference-redesign.v1",
        "status": "draft",
        "created_at": datetime.now(timezone.utc).isoformat(),
        "source_pptx": "reference/source.pptx",
        "source_sha256": deck["sha256"],
        "working_pptx": "working/working.pptx",
        "slide_count": len(deck["slides"]),
        "slides": [
            {
                "slide": index,
                "title": _slide_title(slide),
                "shape_count": len(slide["shapes"]),
                "mode": "preserve",
                "delta": "",
                "allow_text_shape_ids": [],
                "allow_geometry_shape_ids": [],
            }
            for index, slide in enumerate(deck["slides"], start=1)
        ],
    }
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest_path


def _int_ids(value: object, field: str, slide_number: int) -> set[str]:
    if not isinstance(value, list):
        raise ReferenceDeckError(
            f"Slide {slide_number}: {field} must be a JSON list of shape IDs"
        )
    result = set()
    for item in value:
        if not isinstance(item, int) and not (isinstance(item, str) and item.isdigit()):
            raise ReferenceDeckError(
                f"Slide {slide_number}: {field} contains invalid shape ID {item!r}"
            )
        result.add(str(item))
    return result


def _verify_preserve(source: dict, candidate: dict, slide_number: int) -> list[str]:
    errors = []
    if source["xml_hash"] != candidate["xml_hash"]:
        errors.append(f"slide {slide_number}: preserve slide XML changed")
    if source["rels_hash"] != candidate["rels_hash"]:
        errors.append(f"slide {slide_number}: preserve slide relationships changed")
    return errors


def _verify_native(
    source: dict, candidate: dict, entry: dict, slide_number: int
) -> list[str]:
    errors = []
    allowed_text = _int_ids(
        entry.get("allow_text_shape_ids", []), "allow_text_shape_ids", slide_number
    )
    allowed_geometry = _int_ids(
        entry.get("allow_geometry_shape_ids", []),
        "allow_geometry_shape_ids",
        slide_number,
    )
    source_shapes = source["shapes"]
    candidate_shapes = candidate["shapes"]
    source_ids = set(source_shapes)
    candidate_ids = set(candidate_shapes)
    for missing in sorted(source_ids - candidate_ids, key=int):
        errors.append(f"slide {slide_number}: source shape {missing} was deleted")
    for added in sorted(candidate_ids - source_ids, key=int):
        errors.append(f"slide {slide_number}: unapproved shape {added} was added")

    for shape_id in sorted(source_ids & candidate_ids, key=int):
        before = source_shapes[shape_id]
        after = candidate_shapes[shape_id]
        if before["kind"] != after["kind"]:
            errors.append(
                f"slide {slide_number} shape {shape_id}: kind changed "
                f"from {before['kind']} to {after['kind']}"
            )
        if before["name"] != after["name"]:
            errors.append(f"slide {slide_number} shape {shape_id}: name changed")
        if shape_id not in allowed_geometry and before["geometry"] != after["geometry"]:
            errors.append(
                f"slide {slide_number} shape {shape_id}: geometry changed without approval"
            )
        if shape_id not in allowed_text and before["text"] != after["text"]:
            errors.append(
                f"slide {slide_number} shape {shape_id}: text changed without approval"
            )
    return errors


def _verify_rebuild(
    source: dict, candidate: dict, entry: dict, slide_number: int
) -> list[str]:
    allowed_text = _int_ids(
        entry.get("allow_text_shape_ids", []), "allow_text_shape_ids", slide_number
    )
    candidate_text = _normalized_text(candidate["all_text"])
    errors = []
    required = []
    for shape_id, record in source["shapes"].items():
        text = _normalized_text(record["text"])
        if text and shape_id not in allowed_text and text not in required:
            required.append(text)
    for text in required:
        if text not in candidate_text:
            preview = text if len(text) <= 80 else text[:77] + "..."
            errors.append(
                f"slide {slide_number}: rebuilt slide dropped native source text {preview!r}"
            )
    return errors


def verify(source: Path, candidate: Path, manifest_path: Path) -> dict:
    source_deck = read_deck(source)
    candidate_deck = read_deck(candidate)
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ReferenceDeckError(f"Cannot read manifest {manifest_path}: {exc}") from exc

    if manifest.get("schema") != "agentframe.reference-redesign.v1":
        raise ReferenceDeckError("Manifest schema must be agentframe.reference-redesign.v1")
    if manifest.get("source_sha256") != source_deck["sha256"]:
        raise ReferenceDeckError(
            "Manifest source_sha256 does not match the supplied source PPTX"
        )
    entries = manifest.get("slides")
    if not isinstance(entries, list):
        raise ReferenceDeckError("Manifest slides must be a JSON list")

    errors: list[str] = []
    source_count = len(source_deck["slides"])
    candidate_count = len(candidate_deck["slides"])
    if source_count != candidate_count:
        errors.append(
            f"slide count changed from {source_count} to {candidate_count}; "
            "reference-redesign v1 is 1:1"
        )
    if len(entries) != source_count:
        errors.append(
            f"manifest has {len(entries)} slide entries; source has {source_count} slides"
        )

    slide_results = []
    for expected_number, entry in enumerate(entries[: min(source_count, candidate_count)], start=1):
        if not isinstance(entry, dict):
            raise ReferenceDeckError(f"Manifest slide {expected_number} must be an object")
        slide_number = entry.get("slide")
        if slide_number != expected_number:
            errors.append(
                f"manifest entry {expected_number} declares slide {slide_number!r}; "
                "entries must remain in source order"
            )
        mode = entry.get("mode")
        if mode not in VALID_MODES:
            errors.append(
                f"slide {expected_number}: invalid mode {mode!r}; "
                f"expected one of {sorted(VALID_MODES)}"
            )
            slide_errors = []
        else:
            before = source_deck["slides"][expected_number - 1]
            after = candidate_deck["slides"][expected_number - 1]
            if mode == "preserve":
                slide_errors = _verify_preserve(before, after, expected_number)
            elif mode == "native":
                slide_errors = _verify_native(before, after, entry, expected_number)
            else:
                slide_errors = _verify_rebuild(before, after, entry, expected_number)
            errors.extend(slide_errors)
        slide_results.append(
            {
                "slide": expected_number,
                "mode": mode,
                "ok": not slide_errors,
                "errors": slide_errors,
            }
        )

    return {
        "ok": not errors,
        "source": str(source.resolve()),
        "candidate": str(candidate.resolve()),
        "manifest": str(manifest_path.resolve()),
        "slides": slide_results,
        "errors": errors,
    }


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Scaffold and verify native-first reference deck redesigns."
    )
    subparsers = parser.add_subparsers(dest="command", required=True)

    scaffold_parser = subparsers.add_parser(
        "scaffold", help="copy a source deck and write a preserve-by-default manifest"
    )
    scaffold_parser.add_argument("source", type=Path)
    scaffold_parser.add_argument("project", type=Path)
    scaffold_parser.add_argument("--force", action="store_true")

    verify_parser = subparsers.add_parser(
        "verify", help="verify a candidate deck against the source and manifest"
    )
    verify_parser.add_argument("source", type=Path)
    verify_parser.add_argument("candidate", type=Path)
    verify_parser.add_argument("manifest", type=Path)
    verify_parser.add_argument("--report", type=Path)
    return parser


def main(argv: list[str] | None = None) -> int:
    args = _build_parser().parse_args(argv)
    try:
        if args.command == "scaffold":
            manifest = scaffold(args.source, args.project, force=args.force)
            print(manifest)
            return 0
        report = verify(args.source, args.candidate, args.manifest)
        payload = json.dumps(report, indent=2) + "\n"
        if args.report:
            args.report.parent.mkdir(parents=True, exist_ok=True)
            args.report.write_text(payload, encoding="utf-8")
        sys.stdout.write(payload)
        return 0 if report["ok"] else 1
    except ReferenceDeckError as exc:
        print(f"reference-deck: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
