#!/usr/bin/env python3
"""Seal and validate noninteractive PPT Master confirmation contracts.

The vendored PPT Master tree stays untouched. This adapter front-loads the
normal Strategist confirmation result into an immutable, run-bound sibling
artifact and materializes only the exact validated result the vendor consumes.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from system import autonomy_contract  # noqa: E402


KIND = "agentframe-ppt-master-confirmation"
SCHEMA_VERSION = 1
VENDOR_COMMIT = "0ac540b175b0a08ab1dd7101db4badfdf61e6475"
SUFFIX = ".agentframe-confirmation.json"
SOURCE_EXTENSIONS = {
    ".md", ".markdown", ".txt", ".csv", ".tsv", ".json", ".jsonl",
    ".yaml", ".yml",
}
PALETTE_KEYS = {
    "background", "secondary_bg", "primary", "accent",
    "secondary_accent", "body_text",
}
FONT_KEYS = {"cjk", "latin", "css"}
SIZE_KEYS = {"title", "subtitle", "annotation"}
BASE_RESULT_KEYS = {
    "canvas", "page_count", "audience", "content_divergence", "mode",
    "visual_style", "color", "icons", "typography", "delivery_purpose",
    "formula_policy", "image_usage", "image_notes", "generation_mode",
    "refine_spec", "stage", "status", "confirmed_at",
}


class ContractError(ValueError):
    pass


def _read_json(path: Path) -> dict:
    try:
        data = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ContractError(f"cannot read JSON {path}: {exc}") from exc
    if not isinstance(data, dict):
        raise ContractError(f"{path}: top level must be an object")
    return data


def _frontmatter(path: Path) -> str:
    try:
        text = path.read_text(encoding="utf-8-sig")
    except OSError as exc:
        raise ContractError(f"cannot read run file {path}: {exc}") from exc
    if not text.startswith("---\n"):
        raise ContractError(f"{path}: missing frontmatter")
    end = text.find("\n---\n", 4)
    if end < 0:
        raise ContractError(f"{path}: invalid frontmatter")
    return text[4:end]


def _repo_path(raw: str | Path, *, must_exist: bool = True) -> Path:
    path = Path(raw)
    if not path.is_absolute():
        path = ROOT / path
    try:
        resolved = path.resolve(strict=must_exist)
        resolved.relative_to(ROOT.resolve())
    except (OSError, ValueError) as exc:
        raise ContractError(f"path must stay inside the AgentFrame repo: {raw}") from exc
    return resolved


def _rel(path: Path) -> str:
    return path.resolve().relative_to(ROOT.resolve()).as_posix()


def _sha(path: Path) -> str:
    return autonomy_contract.sha256_bytes(path.read_bytes())


def canonical_confirmation_path(ppt_project: str | Path, run_id: str) -> Path:
    project = _repo_path(ppt_project)
    if not project.is_dir():
        raise ContractError(f"PPT project is not a directory: {project}")
    return project.parent / f"{project.name}.{run_id}{SUFFIX}"


def _require_string(value: object, label: str, *, empty: bool = False) -> None:
    if not isinstance(value, str) or (not empty and not value.strip()):
        qualifier = "a string" if empty else "a non-empty string"
        raise ContractError(f"result.{label} must be {qualifier}")


def _exact_keys(value: object, keys: set[str], label: str) -> dict:
    if not isinstance(value, dict):
        raise ContractError(f"result.{label} must be an object")
    actual = set(value)
    if actual != keys:
        raise ContractError(
            f"result.{label} keys must be {sorted(keys)}; found {sorted(actual)}"
        )
    return value


def validate_result(result: object) -> dict:
    if not isinstance(result, dict):
        raise ContractError("result must be an object")
    keys = set(BASE_RESULT_KEYS)
    if "template_adherence" in result:
        keys.add("template_adherence")
    usage = result.get("image_usage")
    if isinstance(usage, list) and "ai" in usage:
        keys.update({"image_ai_path", "image_strategy"})
    if set(result) != keys:
        raise ContractError(
            "result must use the one current final shape; "
            f"expected keys {sorted(keys)}, found {sorted(result)}"
        )

    for field in (
        "canvas", "page_count", "audience", "mode", "visual_style", "icons",
        "delivery_purpose", "formula_policy", "generation_mode",
    ):
        _require_string(result[field], field)
    _require_string(result["content_divergence"], "content_divergence", empty=True)
    _require_string(result["image_notes"], "image_notes", empty=True)
    if result["canvas"] not in {"ppt169", "ppt43"}:
        raise ContractError("result.canvas must be ppt169 or ppt43")
    if result["delivery_purpose"] not in {"text", "balanced", "presentation"}:
        raise ContractError(
            "result.delivery_purpose must be text, balanced, or presentation"
        )
    if result["generation_mode"] not in {"continuous", "split"}:
        raise ContractError("result.generation_mode must be continuous or split")
    if not isinstance(result["refine_spec"], bool):
        raise ContractError("result.refine_spec must be boolean")
    if result["stage"] != "final" or result["status"] != "confirmed":
        raise ContractError("result must have stage: final and status: confirmed")
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}", result["confirmed_at"]):
        raise ContractError(
            "result.confirmed_at must be a fixed YYYY-MM-DDTHH:MM:SS timestamp"
        )
    if "template_adherence" in result and result["template_adherence"] not in {
        "strict", "adaptive",
    }:
        raise ContractError("result.template_adherence must be strict or adaptive")

    color = _exact_keys(result["color"], {"name", "palette"}, "color")
    _require_string(color["name"], "color.name")
    palette = _exact_keys(color["palette"], PALETTE_KEYS, "color.palette")
    for key, value in palette.items():
        if not isinstance(value, str) or not re.fullmatch(r"#[0-9A-Fa-f]{6}", value):
            raise ContractError(f"result.color.palette.{key} must be six-digit HEX")

    typography = _exact_keys(
        result["typography"],
        {"name", "heading", "body", "body_size", "body_size_unit", "sizes"},
        "typography",
    )
    _require_string(typography["name"], "typography.name")
    for role in ("heading", "body"):
        font = _exact_keys(typography[role], FONT_KEYS, f"typography.{role}")
        for key, value in font.items():
            _require_string(value, f"typography.{role}.{key}")
    if (
        isinstance(typography["body_size"], bool)
        or not isinstance(typography["body_size"], (int, float))
        or typography["body_size"] <= 0
    ):
        raise ContractError("result.typography.body_size must be a positive number")
    if typography["body_size_unit"] != "px":
        raise ContractError("result.typography.body_size_unit must be px")
    sizes = _exact_keys(typography["sizes"], SIZE_KEYS, "typography.sizes")
    for key, value in sizes.items():
        if isinstance(value, bool) or not isinstance(value, (int, float)) or value <= 0:
            raise ContractError(
                f"result.typography.sizes.{key} must be a positive number"
            )

    if not isinstance(usage, list) or not usage:
        raise ContractError("result.image_usage must be a non-empty source-id array")
    allowed_usage = {"ai", "web", "provided", "placeholder", "none"}
    if (
        any(not isinstance(item, str) or item not in allowed_usage for item in usage)
        or len(set(usage)) != len(usage)
    ):
        raise ContractError(
            "result.image_usage must contain unique current source ids"
        )
    if "none" in usage and len(usage) != 1:
        raise ContractError("result.image_usage none is exclusive")
    if "ai" in usage:
        _require_string(result["image_ai_path"], "image_ai_path")
        strategy = _exact_keys(
            result["image_strategy"],
            {"name", "rendering", "palette", "visual", "color", "mood"},
            "image_strategy",
        )
        for key, value in strategy.items():
            _require_string(value, f"image_strategy.{key}")
    return result


def discover_project_inputs(ppt_project: Path) -> set[str]:
    discovered: set[str] = set()
    sources = ppt_project / "sources"
    if sources.is_dir():
        for path in sources.rglob("*"):
            if not path.is_file() or path.suffix.lower() not in SOURCE_EXTENSIONS:
                continue
            name = path.name.lower()
            if name.endswith(".conversion_profile.json") or name == "image_manifest.json":
                continue
            discovered.add(_rel(path))
    analysis = ppt_project / "analysis"
    if analysis.is_dir():
        patterns = ("source_profile.json", "*.identity.json", "*.slide_library.json")
        for pattern in patterns:
            for path in analysis.glob(pattern):
                if path.is_file():
                    discovered.add(_rel(path))
    templates = ppt_project / "templates"
    if templates.is_dir():
        for path in templates.rglob("*"):
            if path.is_file():
                discovered.add(_rel(path))
    return discovered


def _locate_run(project: str, run_id: str) -> Path:
    candidates = []
    projects = ROOT / "workspace" / "projects"
    if projects.is_dir():
        for path in projects.rglob(f"knowledge/autonomy/{run_id}.md"):
            try:
                fm = _frontmatter(path)
            except ContractError:
                continue
            if autonomy_contract.scalar(fm, "project") == project:
                candidates.append(path)
    if len(candidates) != 1:
        raise ContractError(
            f"expected one autonomy run for project={project!r}, run_id={run_id!r}; "
            f"found {len(candidates)}"
        )
    return candidates[0]


def _validate_run_membership(
    wrapper_path: Path,
    wrapper: dict,
    *,
    expected_session: str | None = None,
) -> tuple[Path, str]:
    run_path = _locate_run(wrapper["project"], wrapper["run_id"])
    fm = _frontmatter(run_path)
    if autonomy_contract.scalar(fm, "schema_version") != autonomy_contract.SCHEMA_VERSION:
        raise ContractError("autonomy run does not use the current schema")
    ppt_project = _repo_path(wrapper["ppt_project"])
    run_project = run_path.parents[2]
    try:
        ppt_project.relative_to(run_project)
    except ValueError as exc:
        raise ContractError(
            "PPT project must live inside the autonomy run's workspace project"
        ) from exc
    status = autonomy_contract.scalar(fm, "status")
    if status not in {"proposed", "running", "review"}:
        raise ContractError(f"autonomy run status {status!r} cannot own PPT confirmation")
    if expected_session is not None:
        if status != "running":
            raise ContractError("runtime PPT use requires a running autonomy run")
        if autonomy_contract.scalar(fm, "bound_session") != expected_session:
            raise ContractError(
                "PPT confirmation contract is bound to a different harness session"
            )
        stored, observed, issues = autonomy_contract.stored_and_observed(fm, ROOT)
        if issues or stored is None or observed is None or stored != observed:
            detail = "; ".join(issues) or (
                f"stored={stored or 'null'}, observed={observed or 'unavailable'}"
            )
            raise ContractError(f"autonomy contract seal is invalid: {detail}")

    required = {
        record["path"] for record in wrapper["inputs"]
    } | {_rel(wrapper_path)}
    context = {
        item.replace("\\", "/")
        for item in autonomy_contract.list_value(fm, "context_sources")
    }
    frozen = {
        item.replace("\\", "/")
        for item in autonomy_contract.list_value(fm, "frozen_context")
    }
    missing_context = sorted(required - context)
    missing_frozen = sorted(required - frozen)
    if missing_context:
        raise ContractError(
            f"run context_sources omits sealed closure: {missing_context}"
        )
    if missing_frozen:
        raise ContractError(
            f"run frozen_context omits sealed closure: {missing_frozen}"
        )
    _allowed_path_checks(fm, ppt_project, wrapper_path)
    return run_path, fm


def _approval_digest(wrapper: dict, *, result: dict | None = None) -> str:
    common = {
        "mode": wrapper["approval"]["mode"],
        "vendor_commit": wrapper["vendor_commit"],
        "project": wrapper["project"],
        "run_id": wrapper["run_id"],
        "ppt_project": wrapper["ppt_project"],
        "inputs": wrapper["inputs"],
    }
    if result is not None:
        common["result"] = result
    else:
        common["delegation"] = wrapper["approval"]["delegation"]
    return autonomy_contract.sha256_bytes(autonomy_contract.canonical_json(common))


def validate_contract(
    wrapper_path: str | Path,
    *,
    expected_session: str | None = None,
    require_materialized: bool = False,
) -> dict:
    path = _repo_path(wrapper_path)
    wrapper = _read_json(path)
    required_keys = {
        "kind", "schema_version", "vendor_commit", "project", "ppt_project",
        "run_id", "inputs", "approval", "result",
    }
    if set(wrapper) != required_keys:
        raise ContractError(
            f"confirmation wrapper keys must be {sorted(required_keys)}"
        )
    if wrapper["kind"] != KIND or wrapper["schema_version"] != SCHEMA_VERSION:
        raise ContractError("unsupported confirmation wrapper kind/schema")
    if wrapper["vendor_commit"] != VENDOR_COMMIT:
        raise ContractError("confirmation wrapper vendor commit does not match VENDOR.md")
    for field in ("project", "ppt_project", "run_id"):
        _require_string(wrapper[field], field)

    ppt_project = _repo_path(wrapper["ppt_project"])
    canonical = canonical_confirmation_path(ppt_project, wrapper["run_id"])
    if path != canonical.resolve():
        raise ContractError(f"wrapper must use canonical path {_rel(canonical)}")
    if not isinstance(wrapper["inputs"], list) or not wrapper["inputs"]:
        raise ContractError("wrapper.inputs must be a non-empty array")
    input_paths: set[str] = set()
    for record in wrapper["inputs"]:
        if not isinstance(record, dict) or set(record) != {"path", "sha256"}:
            raise ContractError("each wrapper input must contain only path and sha256")
        source = _repo_path(record["path"])
        rel = _rel(source)
        if rel in input_paths:
            raise ContractError(f"duplicate wrapper input: {rel}")
        input_paths.add(rel)
        if record["path"] != rel or record["sha256"] != _sha(source):
            raise ContractError(f"wrapper input hash/path drift: {rel}")
    missing = sorted(discover_project_inputs(ppt_project) - input_paths)
    if missing:
        raise ContractError(f"wrapper input closure omits PPT project facts: {missing}")

    result = validate_result(wrapper["result"])
    approval = wrapper["approval"]
    if not isinstance(approval, dict):
        raise ContractError("wrapper.approval must be an object")
    mode = approval.get("mode")
    if mode == "fixed-values":
        if set(approval) != {"mode", "by", "approved_result_sha256"}:
            raise ContractError("fixed approval shape is invalid")
        expected = _approval_digest(wrapper, result=result)
        if approval["approved_result_sha256"] != expected:
            raise ContractError("fixed approval digest does not match result")
    elif mode == "delegate-strategist":
        if set(approval) != {"mode", "by", "delegation", "delegation_sha256"}:
            raise ContractError("delegated approval shape is invalid")
        if approval["delegation"] != {"fields": "all", "constraints": {}}:
            raise ContractError(
                "delegated approval must be exactly fields=all, constraints={}"
            )
        expected = _approval_digest(wrapper)
        if approval["delegation_sha256"] != expected:
            raise ContractError("delegated approval digest does not match identity")
    else:
        raise ContractError("approval mode must be fixed-values or delegate-strategist")
    if approval.get("by") != "operator":
        raise ContractError("approval.by must record operator")

    _validate_run_membership(path, wrapper, expected_session=expected_session)
    if require_materialized:
        materialized = ppt_project / "confirm_ui" / "result.json"
        if not materialized.is_file() or _read_json(materialized) != result:
            raise ContractError(
                "vendor result.json is absent or differs; run ppt_master_contract.py materialize"
            )
    return wrapper


def _allowed_path_checks(fm: str, ppt_project: Path, wrapper_path: Path) -> None:
    covered = False
    for raw in autonomy_contract.list_value(fm, "allowed_paths"):
        allowed = _repo_path(raw, must_exist=False)
        try:
            ppt_project.resolve().relative_to(allowed)
            covered = True
        except ValueError:
            pass
        try:
            wrapper_path.resolve().relative_to(allowed)
        except ValueError:
            continue
        raise ContractError(
            f"allowed_paths may not equal or contain sealed sibling {_rel(wrapper_path)}"
        )
    if not covered:
        raise ContractError("allowed_paths must cover the mutable PPT project directory")


def seal(ppt_project_raw: str, run_raw: str, draft_raw: str) -> Path:
    ppt_project = _repo_path(ppt_project_raw)
    run_path = _repo_path(run_raw)
    draft_path = _repo_path(draft_raw)
    expected_draft = ppt_project / "agentframe-confirmation.draft.json"
    if draft_path != expected_draft.resolve():
        raise ContractError(
            f"draft must use canonical path {_rel(expected_draft)}"
        )
    fm = _frontmatter(run_path)
    if autonomy_contract.scalar(fm, "schema_version") != autonomy_contract.SCHEMA_VERSION:
        raise ContractError("seal requires a current-schema autonomy run")
    if autonomy_contract.scalar(fm, "status") != "proposed":
        raise ContractError("seal uniquely creates approval for a proposed run")
    run_id = autonomy_contract.scalar(fm, "run_id")
    project = autonomy_contract.scalar(fm, "project")
    if not run_id or not project:
        raise ContractError("run_id and project are required")
    canonical_run = _locate_run(project, run_id)
    if run_path != canonical_run.resolve():
        raise ContractError(
            f"--run must name canonical autonomy file {_rel(canonical_run)}"
        )
    try:
        ppt_project.relative_to(canonical_run.parents[2])
    except ValueError as exc:
        raise ContractError(
            "PPT project must live inside the autonomy run's workspace project"
        ) from exc
    wrapper_path = canonical_confirmation_path(ppt_project, run_id)
    if wrapper_path.exists():
        raise ContractError(f"refusing to overwrite sealed confirmation {wrapper_path}")
    _allowed_path_checks(fm, ppt_project, wrapper_path)

    draft = _read_json(draft_path)
    mode = draft.get("mode")
    keys = {"mode", "by", "inputs", "result"}
    if mode == "delegate-strategist":
        keys.add("delegation")
    if set(draft) != keys or draft.get("by") != "operator":
        raise ContractError(f"draft keys must be {sorted(keys)} with by=operator")
    if mode not in {"fixed-values", "delegate-strategist"}:
        raise ContractError("draft mode must be fixed-values or delegate-strategist")
    if mode == "delegate-strategist" and draft.get("delegation") != {
        "fields": "all", "constraints": {},
    }:
        raise ContractError(
            "delegated draft must be exactly fields=all, constraints={}"
        )
    result = validate_result(draft["result"])
    if not isinstance(draft["inputs"], list) or not draft["inputs"]:
        raise ContractError("draft.inputs must be a non-empty path array")
    input_records = []
    seen_inputs = set()
    for raw in draft["inputs"]:
        if not isinstance(raw, str):
            raise ContractError("draft.inputs entries must be paths")
        source = _repo_path(raw)
        relative = _rel(source)
        if relative in seen_inputs:
            raise ContractError(f"draft.inputs contains duplicate path: {relative}")
        seen_inputs.add(relative)
        input_records.append({"path": relative, "sha256": _sha(source)})
    input_records.sort(key=lambda record: record["path"])

    wrapper = {
        "kind": KIND,
        "schema_version": SCHEMA_VERSION,
        "vendor_commit": VENDOR_COMMIT,
        "project": project,
        "ppt_project": _rel(ppt_project),
        "run_id": run_id,
        "inputs": input_records,
        "approval": {
            "mode": mode,
            "by": "operator",
        },
        "result": result,
    }
    if mode == "fixed-values":
        wrapper["approval"]["approved_result_sha256"] = _approval_digest(
            wrapper, result=result
        )
    else:
        wrapper["approval"]["delegation"] = draft["delegation"]
        wrapper["approval"]["delegation_sha256"] = _approval_digest(wrapper)

    # The proposed run must name the future target before this exclusive create.
    required = {record["path"] for record in input_records} | {_rel(wrapper_path)}
    context = {
        item.replace("\\", "/")
        for item in autonomy_contract.list_value(fm, "context_sources")
    }
    frozen = {
        item.replace("\\", "/")
        for item in autonomy_contract.list_value(fm, "frozen_context")
    }
    if required - context or required - frozen:
        raise ContractError(
            "proposed run must name every input and the future sealed wrapper in "
            "both context_sources and frozen_context"
        )
    missing = sorted(discover_project_inputs(ppt_project) - {
        record["path"] for record in input_records
    })
    if missing:
        raise ContractError(f"draft input closure omits PPT project facts: {missing}")

    try:
        with wrapper_path.open("x", encoding="utf-8", newline="\n") as handle:
            json.dump(wrapper, handle, ensure_ascii=False, indent=2, sort_keys=True)
            handle.write("\n")
    except FileExistsError as exc:
        raise ContractError(f"sealed confirmation already exists: {wrapper_path}") from exc
    validate_contract(wrapper_path)
    return wrapper_path


def materialize(wrapper_raw: str, expected_session: str) -> Path:
    wrapper_path = _repo_path(wrapper_raw)
    wrapper = validate_contract(wrapper_path, expected_session=expected_session)
    ppt_project = _repo_path(wrapper["ppt_project"])
    target = ppt_project / "confirm_ui" / "result.json"
    target.parent.mkdir(parents=True, exist_ok=True)
    fd, temp_name = tempfile.mkstemp(
        prefix=".agentframe-result-", suffix=".json", dir=target.parent
    )
    try:
        with os.fdopen(fd, "w", encoding="utf-8", newline="\n") as handle:
            json.dump(
                wrapper["result"],
                handle,
                ensure_ascii=False,
                indent=2,
                sort_keys=True,
            )
            handle.write("\n")
        os.replace(temp_name, target)
    finally:
        if os.path.exists(temp_name):
            os.unlink(temp_name)
    return target


def contract_candidates(ppt_project_raw: str | Path) -> list[Path]:
    ppt_project = _repo_path(ppt_project_raw)
    return sorted(ppt_project.parent.glob(f"{ppt_project.name}.*{SUFFIX}"))


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    seal_parser = sub.add_parser("seal")
    seal_parser.add_argument("ppt_project")
    seal_parser.add_argument("--run", required=True)
    seal_parser.add_argument("--draft", required=True)
    check_parser = sub.add_parser("check")
    check_parser.add_argument("confirmation")
    materialize_parser = sub.add_parser("materialize")
    materialize_parser.add_argument("confirmation")
    materialize_parser.add_argument("--session-binding", required=True)
    args = parser.parse_args()
    try:
        if args.command == "seal":
            path = seal(args.ppt_project, args.run, args.draft)
            print(f"sealed: {_rel(path)}")
        elif args.command == "check":
            validate_contract(args.confirmation)
            print(f"valid: {_rel(_repo_path(args.confirmation))}")
        else:
            try:
                binding = autonomy_contract.normalize_session_binding(
                    args.session_binding
                )
            except ValueError as exc:
                raise ContractError(str(exc)) from exc
            path = materialize(args.confirmation, expected_session=binding)
            print(f"materialized: {_rel(path)}")
    except ContractError as exc:
        print(f"ppt-master-contract: ERROR: {exc}", file=sys.stderr)
        raise SystemExit(1) from exc


if __name__ == "__main__":
    main()
