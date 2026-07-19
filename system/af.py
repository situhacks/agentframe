#!/usr/bin/env python3
"""AgentFrame state-transition CLI — the buttons.

Owns the MECHANICS of project state changes. Binds to the
project-frontmatter schema only — never to flows, templates, or content.
The spine is a generic engine; everything domain-specific lives in a pack
under library/domains/{domain}/ which this host reads and dispatches to. The
spine names no domain.

Each command performs its bookkeeping atomically, writes the project paper
trail (activity.md) as a side effect, and prints back the JUDGMENT checklist
the agent must still run. Stdlib only.

Commands:
  python system/af.py ready <project> <deliverable-slug-or-path>
  python system/af.py publish <project> <deliverable-slug-or-path> [--url URL] [--posted-at ISO] [--platform P] [--media PATH ...]
  python system/af.py version <project> <deliverable-slug> [--artifact <artifact-name>]
  python system/af.py draft <project> <deliverable-slug> (--file <project-relative-v1.md> | --artifact <artifact-name>)
  python system/af.py adopt <project> <deliverable-slug> --file <existing-project-relative.md>
  python system/af.py new-project <slug> [--domain project-mgmt] [--flow open-flow] [--name NAME]
  python system/af.py doctor [project|pipeline]
  python system/af.py sync-harnesses --check|--write
  python system/af.py automation init|ready|activate|pause|retire ...
  python system/af.py autonomy init|check|start|checkpoint|finish ...
  python system/af.py pipe save --company C --role R --url U [--ats A] [--source S] [--posted D] [--deadline D] [--salary S] [--slug K]
  python system/af.py pipe start <slug>
  python system/af.py pipe stage <slug> <stage>
  python system/af.py pipe board

`pipe` verbs drive the pipeline-topology surface (workspace/pipeline/): a
stage-based funnel whose board (pipeline.md `applications:` rows) is the single
owner of stage state. Application folders reuse the generic deliverable
machinery — ready/version/doctor work on application.md exactly as on
project.md. The spine still names no domain; a pack opts into the pipeline
topology by declaring `topology: pipeline`.
"""

import argparse
import datetime
import glob
import hashlib
import importlib.util
import json
import os
import re
import shutil
import sys
import tempfile
import types
from pathlib import Path

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROJECTS = os.path.join(ROOT, "workspace", "projects")
DOMAINS = os.path.join(ROOT, "library", "domains")
PIPELINE = os.path.join(ROOT, "workspace", "pipeline")

STATUS_ENUM = {"not_started", "drafting", "ready", "published", "deferred"}
LIFECYCLE_ENUM = {"active", "complete", "cancelled"}
PROJECT_SCHEMA_VERSION = "2026-07-19-v2"
EXPORTABLE_INGREDIENTS = ("image-prompts",)  # cross-domain names; packs add their own via pack.md `exportable:`

# Pipeline stage machine (pipeline-topology packs; board = pipeline.md).
PIPE_STAGES = ("saved", "preparing", "applied", "interviewing", "offer", "rejected", "ghosted", "dropped")
PIPE_TRANSITIONS = {
    "saved": {"preparing", "dropped"},
    "preparing": {"applied", "dropped"},
    "applied": {"interviewing", "rejected", "ghosted", "dropped"},
    "interviewing": {"offer", "rejected", "ghosted", "dropped"},
    "ghosted": {"interviewing", "rejected", "dropped"},  # late replies happen
    "offer": set(), "rejected": set(), "dropped": set(),
}
PIPE_NUDGE_DAYS = 7        # applied/interviewing rows silent this long → follow-up note
PIPE_STALE_SAVED_DAYS = 30 # saved rows older than this → drop-or-start note
DEFAULT_DOMAIN = "project-mgmt"
DEFAULT_FLOW = "open-flow"
FLOWS = {"marketing-solo-flow": "1-research-and-architecture",
         "marketing-standard-flow": "1-research",
         "open-flow": "active",
         "project-mgmt-open-flow": "active"}

AUTONOMY_SCHEMA_VERSION = "2026-07-10"
AUTONOMY_STATUSES = {"proposed", "running", "blocked", "review", "complete"}
AUTONOMY_LEVELS = {"plan-only", "assisted", "unattended"}
AUTONOMY_REVIEWER_MODES = {"independent", "same-context", "human"}
AUTONOMY_COMPLETION_GATES = {"human", "independent-review"}
AUTONOMY_MODEL_TIERS = {"premium", "workhorse", "economical", "current", "none"}
AUTOMATION_SCHEMA_VERSION = "2026-07-12"
AUTOMATION_STATUSES = {"proposed", "ready", "active", "paused", "retired"}
AUTOMATION_TRANSITIONS = {
    "proposed": {"ready", "retired"},
    "ready": {"active", "retired"},
    "active": {"paused", "retired"},
    "paused": {"active", "retired"},
    "retired": set(),
}


def die(msg):
    print(f"af: ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def today():
    return datetime.date.today().isoformat()


def stamp():
    return datetime.datetime.now().strftime("%Y-%m-%d %H:%M")


def now_iso():
    return datetime.datetime.now().astimezone().isoformat(timespec="seconds")


def read(path):
    with open(path, "r", encoding="utf-8-sig") as fh:  # -sig: tolerate BOMs from Windows editors
        return fh.read()


def write(path, text):
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)


def project_dir(arg):
    """Find a project by folder name, else by its `slug` frontmatter field.
    Pipeline application folders (application.md) resolve here too, so
    ready/version/doctor work on them unchanged."""
    for base in (PROJECTS, os.path.join(PROJECTS, "completed")):
        d = os.path.join(base, arg)
        if os.path.isfile(os.path.join(d, "project.md")):
            return d
    d = os.path.join(PIPELINE, "applications", arg)
    if os.path.isfile(os.path.join(d, "application.md")):
        return d
    for base in (PROJECTS, os.path.join(PROJECTS, "completed")):
        if not os.path.isdir(base):
            continue
        for name in sorted(os.listdir(base)):
            sp = os.path.join(base, name, "project.md")
            if os.path.isfile(sp) and get_scalar(split_fm(read(sp), sp)[0], "slug") == arg:
                return os.path.join(base, name)
    die(f"project '{arg}' not found under workspace/projects/ or workspace/pipeline/applications/")


def state_doc(cdir):
    """The state file for a work folder: application.md on the pipeline surface, else project.md."""
    return "application.md" if os.path.isfile(os.path.join(cdir, "application.md")) else "project.md"


def split_fm(text, path="file"):
    parsed = split_fm_optional(text)
    if parsed is None:
        die(f"{path} has no frontmatter block")
    return parsed


def split_fm_optional(text):
    """Return frontmatter/body without emitting a fatal CLI error."""
    m = re.match(r"\A---\r?\n(.*?)\r?\n---\r?\n?", text, re.S)
    if not m:
        return None
    return m.group(1), text[m.end():]


def join_fm(fm, body):
    return f"---\n{fm}\n---\n{body}"


def set_scalar(fm, key, value, path="frontmatter"):
    """Replace `key: ...` at any indent; error if absent (no silent schema invention)."""
    pat = re.compile(rf"^(\s*{re.escape(key)}:)[ \t]*.*$", re.M)
    if not pat.search(fm):
        die(f"{path}: field '{key}' not found — fix the file or the schema first")
    return pat.sub(rf"\g<1> {value}", fm, count=1)


def upsert_scalar(fm, key, value, before="deliverables"):
    """Set an optional top-level scalar, or insert it before a known owner block."""
    if has_field(fm, key):
        return set_scalar(fm, key, value)
    if before is None:
        return fm.rstrip("\n") + f"\n{key}: {value}\n"
    marker = re.search(rf"^{re.escape(before)}:", fm, re.M)
    if not marker:
        die(f"frontmatter: cannot add optional field '{key}' before missing '{before}' block")
    return fm[:marker.start()] + f"{key}: {value}\n" + fm[marker.start():]


def clean_value(v):
    return re.sub(r"\s+#.*$", "", v).strip().strip('"')


def yaml_quote(value):
    """One-line double-quoted YAML scalar for CLI-provided text."""
    if value in (None, ""):
        return "null"
    clean = " ".join(str(value).split())
    clean = clean.replace("\\", "\\\\").replace('"', '\\"')
    return f'"{clean}"'


def get_scalar(fm, key):
    m = re.search(rf"^\s*{re.escape(key)}:[ \t]*(.*?)\s*$", fm, re.M)
    return clean_value(m.group(1)) if m else None


def has_field(fm, key):
    return re.search(rf"^\s*{re.escape(key)}:", fm, re.M) is not None


def fm_list(fm, key):
    """Parse a `key: [a, b, c]` inline list from a frontmatter block."""
    m = re.search(rf"^\s*{re.escape(key)}:\s*\[(.*?)\]\s*$", fm, re.M)
    if not m:
        return []
    return [i.strip() for i in m.group(1).split(",") if i.strip()]


def deliverables_span(fm):
    """Span and shape of the top-level deliverables mapping."""
    m = re.search(r"^deliverables:\s*(\{\})?\s*$", fm, re.M)
    if not m:
        return None
    if m.group(1):
        return m.start(), m.end(), True
    rest = fm[m.end():]
    nxt = re.search(r"^\S", rest, re.M)
    end = m.end() + (nxt.start() if nxt else len(rest))
    return m.start(), end, False


def row_span(fm, slug):
    """Span of a deliverable row, or a pipeline application row on pipeline boards."""
    dspan = deliverables_span(fm)
    if not dspan:
        m = re.search(r"^applications:\s*(\{\})?\s*$", fm, re.M)
        if not m or m.group(1):
            return None
        rest = fm[m.end():]
        nxt = re.search(r"^\S", rest, re.M)
        dspan = (m.start(), m.end() + (nxt.start() if nxt else len(rest)), False)
    if dspan[2]:
        return None
    ds, de, _ = dspan
    block = fm[ds:de]
    m = re.search(rf"^  {re.escape(slug)}:\s*$", block, re.M)
    if not m:
        return None
    start = ds + m.start()
    rest = fm[ds + m.end():de]
    nxt = re.search(r"^(  \S|\S)", rest, re.M)
    end = ds + m.end() + (nxt.start() if nxt else len(rest))
    return start, end


def row_set(fm, slug, key, value):
    span = row_span(fm, slug)
    if not span:
        die(f"tracker row '{slug}' not found")
    s, e = span
    block = fm[s:e]
    pat = re.compile(rf"^(    {re.escape(key)}:)[ \t]*.*$", re.M)
    if pat.search(block):
        block = pat.sub(rf"\g<1> {value}", block, count=1)
    else:
        block = block.rstrip("\n") + f"\n    {key}: {value}\n"
    return fm[:s] + block + fm[e:]


def row_get(fm, slug, key):
    span = row_span(fm, slug)
    if not span:
        return None
    m = re.search(rf"^    {re.escape(key)}:[ \t]*(.*?)\s*$", fm[span[0]:span[1]], re.M)
    return clean_value(m.group(1)) if m else None


def row_add(fm, slug, fields):
    """Append a new row to deliverables, expanding an empty inline map."""
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", slug):
        die("deliverable slug must contain only letters, numbers, underscores, and hyphens")
    if row_span(fm, slug):
        die(f"project.md: tracker row '{slug}' already exists")
    dspan = deliverables_span(fm)
    if not dspan:
        die("project.md: top-level deliverables mapping is missing")
    block = "\n".join([f"  {slug}:"] + [f"    {key}: {value}" for key, value in fields])
    ds, de, inline = dspan
    if inline:
        return fm[:ds] + "deliverables:\n" + block + fm[de:]
    current = fm[ds:de].rstrip("\n")
    return fm[:ds] + current + "\n" + block + "\n" + fm[de:].lstrip("\n")


def all_rows(fm):
    dspan = deliverables_span(fm)
    if not dspan or dspan[2]:
        return []
    ds, de, _ = dspan
    block = fm[ds:de]
    return re.findall(r"^  ([A-Za-z0-9_-]+):\s*$", block, re.M)


def resolve_deliverable_target(fm, target):
    """Return (tracker row, project-relative file) for a row slug or direct path."""
    norm = target.replace("\\", "/")
    if "/" not in norm:
        return target, row_get(fm, target, "file") or die(f"row '{target}' has no file pointer")
    matches = [row for row in all_rows(fm) if row_get(fm, row, "file") == norm]
    return (matches[0] if len(matches) == 1 else None), norm


def versions_in(folder, name):
    """All strictly-versioned {name}-v{N}.md numbers in a folder (ignores -v12-FINAL.md style names)."""
    out = []
    for p in glob.glob(os.path.join(folder, f"{name}-v*.md")):
        m = re.fullmatch(rf"{re.escape(name)}-v(\d+)\.md", os.path.basename(p))
        if m:
            out.append(int(m.group(1)))
    return out


def head_of(path):
    """Verify a versioned file is the highest v{N} in its folder; return (name, N)."""
    m = re.fullmatch(r"(.+)-v(\d+)\.md", os.path.basename(path))
    if not m:
        return None
    name, n = m.group(1), int(m.group(2))
    highest = max(versions_in(os.path.dirname(path), name))
    if n != highest:
        die(f"{os.path.basename(path)} is not the head ({name}-v{highest}.md exists) — point at the head or run doctor")
    return name, n


def touch_lifecycle(fm):
    return set_scalar(fm, "last_activity", now_iso(), "project.md")


def append_activity(cdir, line):
    path = os.path.join(cdir, "activity.md")
    text = read(path) if os.path.isfile(path) else ""
    if text and not text.endswith("\n"):
        text += "\n"
    write(path, text + f"{stamp()} — {line}\n")


# ---------------------------------------------------------------- plugin host

def project_domain(cfm):
    return get_scalar(cfm, "domain")


def load_pack(domain):
    """(descriptor_fm | None, pack_dir | None). The pack is the only artifact that knows a domain."""
    if not domain:
        return None, None
    pack_dir = os.path.join(DOMAINS, domain)
    desc = os.path.join(pack_dir, "pack.md")
    if not os.path.isfile(desc):
        return None, pack_dir
    fm, _ = split_fm(read(desc), "pack.md")
    return fm, pack_dir


def load_rules(pack_dir):
    """Import the domain's rules.py if present. Absent = None (normal). Import error = fail loud + isolated."""
    if not pack_dir:
        return None
    rp = os.path.join(pack_dir, "rules.py")
    if not os.path.isfile(rp):
        return None
    spec = importlib.util.spec_from_file_location("af_domain_rules", rp)
    mod = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(mod)
    except Exception as e:
        die(f"domain rules module failed to load ({os.path.relpath(rp, ROOT)}): {e}")
    return mod


def make_ctx():
    """The host helpers a domain rules module is handed (it never imports af.py)."""
    return types.SimpleNamespace(
        ROOT=ROOT, read=read, write=write, split_fm=split_fm, join_fm=join_fm,
        set_scalar=set_scalar, upsert_scalar=upsert_scalar, get_scalar=get_scalar, row_set=row_set, row_get=row_get,
        row_span=row_span, all_rows=all_rows, fm_list=fm_list, versions_in=versions_in, today=today,
        now_iso=now_iso, append_activity=append_activity, touch_lifecycle=touch_lifecycle, die=die)


# ---------------------------------------------------------------- ready

def pack_exportables(cfm):
    """Deliverable basenames the owning pack gates behind filed exports (pack.md `exportable:` list)."""
    desc, _ = load_pack(project_domain(cfm))
    return tuple(fm_list(desc, "exportable")) if desc else ()


def exportable_ingredient(path, extra=()):
    m = re.fullmatch(r"(.+)-v\d+\.md", os.path.basename(path))
    return m.group(1) if m and m.group(1) in EXPORTABLE_INGREDIENTS + tuple(extra) else None


def export_gate_issues(cdir, rel, dfm, extra=()):
    """Blocking exports[] problems for an exportable deliverable; [] when clear."""
    if not exportable_ingredient(rel, extra):
        return []
    values = fm_list_values(dfm, "exports")
    if not values:
        return ["exports[] is empty"]
    return [f"exports path {status}: {v}"
            for v in values for status in [manifest_path_status(cdir, rel, v)] if status]


def cmd_ready(args):
    cdir = project_dir(args.project)
    sdoc = state_doc(cdir)
    cfm, cbody = split_fm(read(os.path.join(cdir, sdoc)), sdoc)

    slug, rel = resolve_deliverable_target(cfm, args.deliverable)

    dpath = os.path.join(cdir, rel)
    os.path.isfile(dpath) or die(f"deliverable file not found: {rel}")
    head_of(dpath)

    dfm, dbody = split_fm(read(dpath), rel)
    gate = export_gate_issues(cdir, rel, dfm, pack_exportables(cfm))
    override_note = ""
    if gate:
        if not args.allow_missing_exports:
            folder = os.path.dirname(rel).replace("\\", "/") or "."
            die(f"{rel}: exportable deliverable failed the exports gate ({'; '.join(gate)}). "
                f"Land the approved finals under {folder}/media/ and record each path in exports[] "
                f"frontmatter, then rerun. Override (rare): --allow-missing-exports - doctor flags "
                f"the row until exports land.")
        override_note = f"READY WITHOUT EXPORTS (override): {'; '.join(gate)}"
    dfm = set_scalar(dfm, "status", "ready", rel)
    dfm = set_scalar(dfm, "last_updated", today(), rel)

    notes = []
    rules = load_rules(load_pack(project_domain(cfm))[1])
    if rules and hasattr(rules, "on_ready"):
        cfm, notes = rules.on_ready(make_ctx(), cdir, dpath, rel, cfm)
    if override_note:
        notes.append(override_note)

    write(dpath, join_fm(dfm, dbody))
    if slug:
        cfm = row_set(cfm, slug, "status", "ready")
        cfm = row_set(cfm, slug, "last_updated", today())
    cfm = touch_lifecycle(cfm)
    write(os.path.join(cdir, sdoc), join_fm(cfm, cbody))
    append_activity(cdir, f"ready: {slug or os.path.basename(rel)} ready; artifact={rel}"
                    + (f"; {'; '.join(notes)}" if notes else ""))

    print(f"af ready: {rel} -> ready" + (f" ({'; '.join(notes)})" if notes else ""))
    print("\nJudgment checklist (agent + operator):")
    print("  [ ] Template readiness criteria verified (the deliverable's template)")
    print("  [ ] Humanizer pass run, when the template declares it (public-facing prose)")
    print("  [ ] Voice was loaded for this deliverable's drafting (confirm if session resumed)")
    print("  [ ] Voice mini-retro eligibility checked (library/process/voice-mini-retro.md)")
    print("  [ ] Remaining follow-ups surfaced (feedback, export, publish)")


# ---------------------------------------------------------------- publish

def cmd_publish(args):
    cdir = project_dir(args.project)
    sdoc = state_doc(cdir)
    cpath = os.path.join(cdir, sdoc)
    cfm, cbody = split_fm(read(cpath), sdoc)
    domain = project_domain(cfm)
    _, pack_dir = load_pack(domain)
    rules = load_rules(pack_dir)
    if (rules and hasattr(rules, "publish") and hasattr(rules, "handles_publish")
            and rules.handles_publish(make_ctx(), cdir, args)):
        rules.publish(make_ctx(), cdir, args)
        return

    slug, rel = resolve_deliverable_target(cfm, args.deliverable)
    dpath = os.path.join(cdir, rel)
    os.path.isfile(dpath) or die(f"deliverable file not found: {rel}")
    dfm, dbody = split_fm(read(dpath), rel)
    status = get_scalar(dfm, "status")
    if status != "ready":
        die(f"{rel}: publish requires status ready (found {status or 'missing'}); run af ready first")
    if args.url:
        dfm = upsert_scalar(dfm, "published_url", yaml_quote(args.url), before=None)
    dfm = set_scalar(dfm, "status", "published", rel)
    dfm = set_scalar(dfm, "last_updated", today(), rel)
    write(dpath, join_fm(dfm, dbody))
    if slug:
        cfm = row_set(cfm, slug, "status", "published")
        cfm = row_set(cfm, slug, "last_updated", today())
    cfm = touch_lifecycle(cfm)
    write(cpath, join_fm(cfm, cbody))
    label = slug or os.path.basename(rel)
    detail = f"; url={args.url}" if args.url else ""
    append_activity(cdir, f"publish: {label} published; artifact={rel}{detail}")
    print(f"af publish: {rel} -> published" + (f" ({args.url})" if args.url else ""))


# ---------------------------------------------------------------- version

def version_target(cdir, cfm, row, artifact=None):
    """Resolve a row-owned head or an exact nested artifact head.

    Returns (source_rel, move_tracker_pointer). A nested artifact is addressed
    relative to the folder owned by the parent row and never moves that row's
    assembly-record pointer.
    """
    rel = row_get(cfm, row, "file") or die(f"tracker row '{row}' not found or has no file")
    if not artifact:
        dpath = os.path.join(cdir, rel)
        os.path.isfile(dpath) or die(f"deliverable file not found: {rel}")
        head_of(dpath) or die(f"{rel} is not a versioned -v{{N}}.md file")
        return rel.replace("\\", "/"), True

    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", artifact):
        die("artifact name must contain only letters, numbers, underscores, and hyphens")

    parent_path = os.path.join(cdir, rel)
    if os.path.isdir(parent_path):
        folder = parent_path
    elif os.path.isfile(parent_path):
        folder = os.path.dirname(parent_path)
    else:
        die(f"parent tracker path not found: {rel}")

    versions = versions_in(folder, artifact)
    if not versions:
        die(f"artifact '{artifact}' has no versioned head under "
            f"{os.path.relpath(folder, cdir).replace(os.sep, '/')}; start it with af draft")
    n = max(versions)
    source_path = os.path.join(folder, f"{artifact}-v{n}.md")
    head_of(source_path)
    return os.path.relpath(source_path, cdir).replace(os.sep, "/"), False


def cmd_version(args):
    cdir = project_dir(args.project)
    sdoc = state_doc(cdir)
    cpath = os.path.join(cdir, sdoc)
    cfm, cbody = split_fm(read(cpath), sdoc)
    rel, move_pointer = version_target(
        cdir, cfm, args.deliverable, getattr(args, "artifact", None)
    )
    dpath = os.path.join(cdir, rel)
    name, n = head_of(dpath)

    if not move_pointer and row_get(cfm, args.deliverable, "status") == "published":
        die(f"tracker row '{args.deliverable}' points at a published unversioned assembly. "
            "Published records are immutable; create a new tracked edition before revising its ingredients.")

    new_rel = os.path.join(os.path.dirname(rel), f"{name}-v{n + 1}.md").replace("\\", "/")
    new_path = os.path.join(cdir, new_rel)
    if os.path.exists(new_path):
        die(f"destination already exists: {new_rel}")

    dfm, dbody = split_fm(read(dpath), rel)
    source_status = get_scalar(dfm, "status")
    dfm = set_scalar(dfm, "status", "drafting", new_rel)
    dfm = set_scalar(dfm, "last_updated", today(), new_rel)
    write(new_path, join_fm(dfm, dbody))

    if move_pointer:
        cfm = row_set(cfm, args.deliverable, "file", new_rel)
    else:
        parent_rel = row_get(cfm, args.deliverable, "file")
        parent_path = os.path.join(cdir, parent_rel)
        if os.path.isfile(parent_path):
            pfm, pbody = split_fm(read(parent_path), parent_rel)
            pfm = set_scalar(pfm, "status", "drafting", parent_rel)
            pfm = set_scalar(pfm, "last_updated", today(), parent_rel)
            write(parent_path, join_fm(pfm, pbody))
    cfm = row_set(cfm, args.deliverable, "status", "drafting")
    cfm = row_set(cfm, args.deliverable, "last_updated", today())
    cfm = touch_lifecycle(cfm)
    write(cpath, join_fm(cfm, cbody))

    label = getattr(args, "artifact", None) or args.deliverable
    source_note = f"; source_status={source_status}" if source_status in {"ready", "published"} else ""
    append_activity(cdir, f"artifact_versioned: {label} v{n} -> v{n + 1}; {new_rel}{source_note}")

    pointer_note = "tracker pointer moved" if move_pointer else "parent tracker pointer unchanged"
    print(f"af version: {rel} -> {new_rel} (head; {pointer_note}; prior version untouched as the snapshot)")
    print("\nJudgment (stays with the agent):")
    print("  - Use this for REPLACEMENT-shaped changes (deliverable-versioning.md). Surgical")
    print("    edits (typos, frontmatter, small wording) go directly into the current head.")
    print(f"  - {new_rel} already contains v{n}'s full content — apply the replacement as surgical")
    print("    edits to that copy; a full-file rewrite is right only when the replacement is genuinely whole-body.")
    print("  - If the operator feedback criticized SHAPE or process, append one feedback-log.md line this turn.")


# ---------------------------------------------------------------- draft

def safe_project_rel(cdir, value):
    """Return a normalized project-relative path and its absolute target."""
    value = (value or "").replace("\\", "/").strip()
    if not value or os.path.isabs(value) or value == ".." or value.startswith("../") or "/../" in value:
        die("file path must stay inside the project and be project-relative")
    target = os.path.abspath(os.path.join(cdir, value))
    if os.path.commonpath((os.path.abspath(cdir), target)) != os.path.abspath(cdir):
        die("file path resolves outside the project")
    return value, target


def cmd_draft(args):
    cdir = project_dir(args.project)
    sdoc = state_doc(cdir)
    cpath = os.path.join(cdir, sdoc)
    cfm, cbody = split_fm(read(cpath), sdoc)
    row_span(cfm, args.deliverable) or die(f"tracker row '{args.deliverable}' not found")

    notes = []
    if args.artifact:
        if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", args.artifact):
            die("artifact name must contain only letters, numbers, underscores, and hyphens")
        parent_rel = row_get(cfm, args.deliverable, "file") or die(
            f"tracker row '{args.deliverable}' has no parent file/folder pointer"
        )
        parent_rel = parent_rel.replace("\\", "/")
        folder_rel = os.path.dirname(parent_rel) if parent_rel.lower().endswith(".md") else parent_rel.rstrip("/")
        folder_rel = folder_rel or "."
        new_rel, new_path = safe_project_rel(cdir, f"{folder_rel}/{args.artifact}-v1.md")
        if versions_in(os.path.dirname(new_path), args.artifact):
            die(f"artifact '{args.artifact}' already has a version chain under {folder_rel}")
        move_pointer = False
    else:
        new_rel, new_path = safe_project_rel(cdir, args.file)
        m = re.fullmatch(r"(.+)-v1\.md", os.path.basename(new_rel))
        if not m:
            die("--file must name a canonical -v1.md first-draft path")
        current_rel = row_get(cfm, args.deliverable, "file")
        if current_rel and os.path.exists(os.path.join(cdir, current_rel)):
            die(f"tracker row '{args.deliverable}' already has an existing artifact: {current_rel}")
        if versions_in(os.path.dirname(new_path), m.group(1)):
            die(f"deliverable '{m.group(1)}' already has a version chain under {os.path.dirname(new_rel) or '.'}")
        move_pointer = True

    if os.path.exists(new_path):
        die(f"destination already exists: {new_rel}")

    os.makedirs(os.path.dirname(new_path), exist_ok=True)
    write(new_path, f"---\nstatus: drafting\nlast_updated: {today()}\n---\n\n")

    rules = load_rules(load_pack(project_domain(cfm))[1])
    if rules and hasattr(rules, "on_draft"):
        cfm, notes = rules.on_draft(
            make_ctx(), cdir, new_path, new_rel, cfm, args.deliverable
        )

    if move_pointer:
        cfm = row_set(cfm, args.deliverable, "file", new_rel)
    cfm = row_set(cfm, args.deliverable, "status", "drafting")
    cfm = row_set(cfm, args.deliverable, "last_updated", today())
    cfm = touch_lifecycle(cfm)
    write(cpath, join_fm(cfm, cbody))

    label = args.artifact or args.deliverable
    append_activity(cdir, f"artifact_drafted: {label} created; {new_rel}")

    pointer_note = "tracker pointer moved" if move_pointer else "parent tracker pointer preserved"
    print(f"af draft: created {new_rel} ({pointer_note}" +
          (f"; {'; '.join(notes)}" if notes else "") + ")")
    print("\nJudgment (stays with the agent):")
    print("  - Load the resolved deliverable template before writing content.")
    print("  - Add any template-specific frontmatter fields before drafting; this command")
    print("    creates only the shared status/last_updated container.")


# ---------------------------------------------------------------- adopt

def cmd_adopt(args):
    """Register an existing drafting artifact without overwriting it."""
    cdir = project_dir(args.project)
    sdoc = state_doc(cdir)
    cpath = os.path.join(cdir, sdoc)
    cfm, cbody = split_fm(read(cpath), sdoc)
    rel, target = safe_project_rel(cdir, args.file)
    if not rel.lower().endswith(".md") or not os.path.isfile(target):
        die("--file must point at an existing project-relative Markdown artifact")
    parsed = split_fm_optional(read(target))
    if parsed is None:
        die(f"{rel} has no frontmatter block")
    afm, _ = parsed
    if get_scalar(afm, "status") != "drafting":
        die(f"{rel}: status must be drafting before adoption")

    fields = [
        ("status", "drafting"),
        ("file", rel),
        ("last_updated", today()),
    ]
    for key in ("workstream", "export", "notes"):
        value = getattr(args, key, None)
        if value:
            fields.append((key, yaml_quote(value)))

    if row_span(cfm, args.deliverable):
        current = row_get(cfm, args.deliverable, "file")
        if current not in (None, "", "null", rel) and os.path.exists(os.path.join(cdir, current)):
            die(f"tracker row '{args.deliverable}' already has an existing artifact: {current}")
        for key, value in fields:
            cfm = row_set(cfm, args.deliverable, key, value)
        action = "updated"
    else:
        cfm = row_add(cfm, args.deliverable, fields)
        action = "created"

    cfm = touch_lifecycle(cfm)
    write(cpath, join_fm(cfm, cbody))
    append_activity(cdir, f"deliverable_adopted: {args.deliverable} -> {rel}")
    print(f"af adopt: {action} row '{args.deliverable}' -> {rel}")


# ---------------------------------------------------------------- new-project

def cmd_new_project(args):
    slug = args.slug
    re.match(r"^[a-z0-9][a-z0-9-]*$", slug) or die("slug must be folder-safe lowercase kebab-case")
    desc, pack_dir = load_pack(args.domain)
    if not desc:
        die(f"no domain pack at library/domains/{args.domain}/ (pack.md missing) — author the pack first")
    if get_scalar(desc, "topology") == "pipeline":
        die(f"domain '{args.domain}' is pipeline-topology — its work lives under workspace/pipeline/ "
            f"(use 'af pipe save' / 'af pipe start'), not workspace/projects/")
    skel_path = os.path.join(pack_dir, "skeleton.md")
    os.path.isfile(skel_path) or die(f"domain '{args.domain}' ships no skeleton.md")
    cdir = os.path.join(PROJECTS, slug)
    if os.path.exists(cdir):
        die(f"{cdir} already exists")
    os.makedirs(cdir)
    os.makedirs(os.path.join(cdir, "sources"))
    os.makedirs(os.path.join(cdir, "knowledge"))
    os.makedirs(os.path.join(cdir, "knowledge", "people"))
    os.makedirs(os.path.join(cdir, "knowledge", "meetings"))
    os.makedirs(os.path.join(cdir, "knowledge", "_archive"))

    write(os.path.join(cdir, "sources", "INDEX.md"), "# Source Index\n\n| ID | File | Date | Description |\n|---|---|---|---|\n")

    name = args.name or slug.replace("-", " ").title()
    write(os.path.join(cdir, "project.md"), read(skel_path).format(
        name=name, slug=slug, date=today(), domain=args.domain, phase=FLOWS[args.flow], flow=args.flow, ts=now_iso()))
    write(os.path.join(cdir, "feedback-log.md"), "")
    append_activity(cdir, f"project_started: {name} scaffolded ({args.domain}, {args.flow})")

    print(f"af new-project: workspace/projects/{slug}/ scaffolded ({args.domain}, {args.flow}, phase {FLOWS[args.flow]})")
    print("\nJudgment (stays with the agent):")
    print(f"  - Load library/process/flows/{args.flow}.md and run its kickoff")
    print("    (research offer / plan proposal / pack-owned kickoff steps — flow-owned, not script-owned).")


# ---------------------------------------------------------------- project automation

def automation_id(value):
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", value or ""):
        die(f"automation id '{value}' is not folder-safe (use lowercase letters, numbers, hyphens)")
    return value


def automation_ref(project, automation, must_exist=True):
    cdir = project_dir(project)
    if state_doc(cdir) != "project.md":
        die("project automations belong to workspace projects, not pipeline applications")
    automation = automation_id(automation)
    rel = f"automations/{automation}/automation.md"
    path = os.path.join(cdir, *rel.split("/"))
    if must_exist and not os.path.isfile(path):
        die(f"automation '{automation}' not found at {rel}")
    return cdir, rel, path


def automation_issues(cdir, cfm=None):
    cpath = os.path.join(cdir, "project.md")
    if cfm is None:
        cfm, _ = split_fm(read(cpath), "project.md")
    project = get_scalar(cfm, "slug")
    issues = []
    tracked = set(mapping_rows(cfm, "automations"))
    for aid in sorted(tracked):
        rel_project = os.path.relpath(cdir, ROOT).replace("\\", "/")
        label = f"{rel_project}: automation '{aid}'"
        status = mapping_row_get(cfm, "automations", aid, "status")
        rel = mapping_row_get(cfm, "automations", aid, "file")
        deployment = mapping_row_get(cfm, "automations", aid, "deployment_id")
        job = mapping_row_get(cfm, "automations", aid, "job")
        if status not in AUTOMATION_STATUSES:
            issues.append(f"{label} status '{status}' invalid")
        if not rel:
            issues.append(f"{label} has no file pointer")
            continue
        expected = f"automations/{aid}/automation.md"
        if rel != expected:
            issues.append(f"{label} file must be {expected}")
        path = os.path.join(cdir, *rel.replace("\\", "/").split("/"))
        if not os.path.isfile(path):
            issues.append(f"{label} file missing: {rel}")
            continue
        try:
            afm, body = split_fm(read(path), rel)
        except SystemExit:
            issues.append(f"{label} contract has invalid frontmatter")
            continue
        required = {
            "schema_version": AUTOMATION_SCHEMA_VERSION,
            "automation_id": aid,
            "project": project,
        }
        for key, expected_value in required.items():
            if get_scalar(afm, key) != expected_value:
                issues.append(f"{label} contract {key} must be '{expected_value}'")
        for heading in ("## Job", "## Trigger And Inputs", "## Project Route",
                        "## Human Boundaries", "## Result", "## Verification", "## Deployment"):
            if heading not in body:
                issues.append(f"{label} contract missing '{heading}'")
        if not job:
            issues.append(f"{label} job is empty")
        if status == "active" and deployment in (None, "", "null"):
            issues.append(f"{label} active status requires deployment_id")

    root = os.path.join(cdir, "automations")
    if os.path.isdir(root):
        for path in sorted(glob.glob(os.path.join(root, "*", "automation.md"))):
            aid = os.path.basename(os.path.dirname(path))
            if aid not in tracked:
                rel = os.path.relpath(path, cdir).replace("\\", "/")
                issues.append(f"{rel}: automation contract has no project.md tracker row")
    return issues


def automation_write_project(cdir, cfm, cbody):
    write(os.path.join(cdir, "project.md"), join_fm(touch_lifecycle(cfm), cbody))


def cmd_automation_init(args):
    cdir, rel, path = automation_ref(args.project, args.automation_id, must_exist=False)
    cpath = os.path.join(cdir, "project.md")
    cfm, cbody = split_fm(read(cpath), "project.md")
    if get_scalar(cfm, "status") != "active":
        die("project automation requires an active project")
    if mapping_row_span(cfm, "automations", args.automation_id) or os.path.exists(path):
        die(f"automation '{args.automation_id}' already exists")
    project = get_scalar(cfm, "slug")
    os.makedirs(os.path.dirname(path), exist_ok=False)
    afm = f"""schema_version: {AUTOMATION_SCHEMA_VERSION}
automation_id: {args.automation_id}
project: {project}
created_at: {now_iso()}"""
    body = f"""
# Project Automation — {args.automation_id}

## Job

{args.job}

## Trigger And Inputs

(Define the standing trigger and the minimum inputs each run receives.)

## Project Route

(Name the project files, processes, templates, or deliverables a fresh managed run loads.)

## Human Boundaries

(Narrow the daemon charter for this automation. Never widen it.)

## Result

(Define the useful done, blocked, and failed receipt summaries.)

## Verification

(Name the checks one run can perform before reporting done.)

## Deployment

Not deployed. Runtime paths, credentials, queues, logs, and heartbeat state stay outside this project bundle.
"""
    write(path, join_fm(afm, body))
    cfm = mapping_row_add(cfm, "automations", args.automation_id, (
        ("status", "proposed"),
        ("file", rel),
        ("deployment_id", "null"),
        ("last_updated", now_iso()),
        ("job", yaml_quote(args.job)),
    ))
    automation_write_project(cdir, cfm, cbody)
    append_activity(cdir, f"automation_proposed: {args.automation_id} contract created; {rel}")
    print(f"af automation init: {args.automation_id} -> proposed ({rel})")


def automation_transition(args, target):
    cdir, rel, path = automation_ref(args.project, args.automation_id)
    cpath = os.path.join(cdir, "project.md")
    cfm, cbody = split_fm(read(cpath), "project.md")
    current = mapping_row_get(cfm, "automations", args.automation_id, "status")
    if current is None:
        die(f"project.md has no automation row '{args.automation_id}'")
    if target not in AUTOMATION_TRANSITIONS.get(current, set()):
        die(f"automation transition {current} -> {target} is not allowed")
    if target == "ready":
        issues = automation_issues(cdir, cfm)
        if issues:
            die("automation contract invalid:\n  - " + "\n  - ".join(issues))
    if target == "active":
        deployment = getattr(args, "deployment", None) or mapping_row_get(
            cfm, "automations", args.automation_id, "deployment_id")
        if deployment in (None, "", "null"):
            die("activating an automation requires --deployment <registry-id>")
        cfm = mapping_row_set(cfm, "automations", args.automation_id,
                              "deployment_id", yaml_quote(deployment))
    cfm = mapping_row_set(cfm, "automations", args.automation_id, "status", target)
    cfm = mapping_row_set(cfm, "automations", args.automation_id, "last_updated", now_iso())
    automation_write_project(cdir, cfm, cbody)
    event = {"ready": "automation_ready", "active": "automation_activated",
             "paused": "automation_paused", "retired": "automation_retired"}[target]
    append_activity(cdir, f"{event}: {args.automation_id} -> {target}; {rel}")
    print(f"af automation {target}: {args.automation_id} -> {target}")


def cmd_automation_ready(args):
    automation_transition(args, "ready")


def cmd_automation_activate(args):
    automation_transition(args, "active")


def cmd_automation_pause(args):
    automation_transition(args, "paused")


def cmd_automation_retire(args):
    automation_transition(args, "retired")


# ---------------------------------------------------------------- bounded autonomy

def autonomy_run_id(value):
    if not re.fullmatch(r"[a-z0-9][a-z0-9-]*", value or ""):
        die(f"autonomy run id '{value}' is not folder-safe (use lowercase letters, numbers, hyphens)")
    return value


def autonomy_ref(project, run_id, must_exist=True):
    cdir = project_dir(project)
    if state_doc(cdir) != "project.md":
        die("bounded autonomy runs belong to workspace projects, not pipeline applications")
    run_id = autonomy_run_id(run_id)
    path = os.path.join(cdir, "knowledge", "autonomy", f"{run_id}.md")
    if must_exist and not os.path.isfile(path):
        die(f"autonomy run '{run_id}' not found at {os.path.relpath(path, ROOT)}")
    return cdir, path


def autonomy_int(fm, key, issues, label, minimum=0):
    raw = get_scalar(fm, key)
    try:
        value = int(raw)
    except (TypeError, ValueError):
        issues.append(f"{label}: field '{key}' must be an integer")
        return None
    if value < minimum:
        issues.append(f"{label}: field '{key}' must be >= {minimum}")
    return value


def autonomy_issues(path, expected_project=None, require_ready=True):
    label = os.path.relpath(path, ROOT).replace("\\", "/")
    if not os.path.isfile(path):
        return [f"{label}: file missing"]
    try:
        fm, body = split_fm(read(path), label)
    except SystemExit:
        return [f"{label}: missing or invalid frontmatter"]

    issues = []
    required = (
        "schema_version", "run_id", "project", "status", "autonomy_level", "goal",
        "done_when", "context_sources", "allowed_paths", "verification", "max_iterations",
        "max_subagents", "subagents_used", "iteration", "planner_tier", "executor_tier", "reviewer_tier",
        "reviewer_mode", "completion_gate", "started_at", "last_checkpoint", "completed_at",
        "blocked_reason", "completion_evidence", "approved_by",
    )
    for field in required:
        if not has_field(fm, field):
            issues.append(f"{label}: required field '{field}' missing")

    if get_scalar(fm, "schema_version") != AUTONOMY_SCHEMA_VERSION:
        issues.append(f"{label}: schema_version must be {AUTONOMY_SCHEMA_VERSION}")
    if get_scalar(fm, "run_id") != os.path.splitext(os.path.basename(path))[0]:
        issues.append(f"{label}: run_id must match filename")
    if expected_project and get_scalar(fm, "project") != expected_project:
        issues.append(f"{label}: project must be '{expected_project}'")

    status = get_scalar(fm, "status")
    level = get_scalar(fm, "autonomy_level")
    reviewer_mode = get_scalar(fm, "reviewer_mode")
    completion_gate = get_scalar(fm, "completion_gate")
    if status not in AUTONOMY_STATUSES:
        issues.append(f"{label}: status '{status}' invalid")
    if level not in AUTONOMY_LEVELS:
        issues.append(f"{label}: autonomy_level '{level}' invalid")
    if reviewer_mode not in AUTONOMY_REVIEWER_MODES:
        issues.append(f"{label}: reviewer_mode '{reviewer_mode}' invalid")
    if completion_gate not in AUTONOMY_COMPLETION_GATES:
        issues.append(f"{label}: completion_gate '{completion_gate}' invalid")
    for field in ("planner_tier", "executor_tier", "reviewer_tier"):
        if get_scalar(fm, field) not in AUTONOMY_MODEL_TIERS:
            issues.append(f"{label}: {field} '{get_scalar(fm, field)}' invalid")

    max_iterations = autonomy_int(fm, "max_iterations", issues, label, minimum=1)
    max_subagents = autonomy_int(fm, "max_subagents", issues, label, minimum=0)
    subagents_used = autonomy_int(fm, "subagents_used", issues, label, minimum=0)
    iteration = autonomy_int(fm, "iteration", issues, label, minimum=0)
    if max_iterations is not None and iteration is not None and iteration > max_iterations:
        issues.append(f"{label}: iteration {iteration} exceeds max_iterations {max_iterations}")
    if max_subagents is not None and subagents_used is not None and subagents_used > max_subagents:
        issues.append(f"{label}: subagents_used {subagents_used} exceeds max_subagents {max_subagents}")

    if require_ready or status != "proposed":
        for field in ("goal", "done_when"):
            if get_scalar(fm, field) in (None, "", "null"):
                issues.append(f"{label}: readiness field '{field}' is unresolved")
        for field in ("context_sources", "allowed_paths", "verification"):
            if not fm_list(fm, field):
                issues.append(f"{label}: readiness list '{field}' is empty")

    if level == "unattended":
        if reviewer_mode != "independent":
            issues.append(f"{label}: unattended runs require reviewer_mode: independent")
        if max_subagents is not None and max_subagents < 1:
            issues.append(f"{label}: unattended runs require max_subagents >= 1")

    started = get_scalar(fm, "started_at")
    completed = get_scalar(fm, "completed_at")
    evidence = get_scalar(fm, "completion_evidence")
    approved_by = get_scalar(fm, "approved_by")
    blocked_reason = get_scalar(fm, "blocked_reason")
    if status in {"running", "blocked", "review", "complete"} and started in (None, "", "null"):
        issues.append(f"{label}: status '{status}' requires started_at")
    if status == "blocked" and blocked_reason in (None, "", "null"):
        issues.append(f"{label}: blocked status requires blocked_reason")
    if status in {"review", "complete"} and evidence in (None, "", "null"):
        issues.append(f"{label}: status '{status}' requires completion_evidence")
    if status == "complete":
        if completed in (None, "", "null"):
            issues.append(f"{label}: complete status requires completed_at")
        if approved_by not in {"operator", "reviewer"}:
            issues.append(f"{label}: complete status requires approved_by: operator|reviewer")

    for heading in ("## Context", "## Plan", "## Model Routing", "## Checkpoints"):
        if heading not in body:
            issues.append(f"{label}: body missing '{heading}'")
    return issues


def autonomy_project_slug(cdir, require_active=True):
    fm, _ = split_fm(read(os.path.join(cdir, "project.md")), "project.md")
    if require_active and get_scalar(fm, "status") != "active":
        die("bounded autonomy requires an active project")
    return get_scalar(fm, "slug")


def autonomy_save(path, fm, body):
    write(path, join_fm(fm, body))


def autonomy_touch(cdir):
    path = os.path.join(cdir, "project.md")
    fm, body = split_fm(read(path), "project.md")
    write(path, join_fm(touch_lifecycle(fm), body))


def autonomy_checkpoint_body(body, iteration, outcome, summary, evidence=None):
    line = f"- {now_iso()} | iteration {iteration} | {outcome} | {' '.join(summary.split())}"
    if evidence:
        line += f" | evidence: {' '.join(evidence.split())}"
    return body.rstrip() + "\n" + line + "\n"


def cmd_autonomy_init(args):
    cdir, path = autonomy_ref(args.project, args.run_id, must_exist=False)
    project = autonomy_project_slug(cdir)
    if os.path.exists(path):
        die(f"autonomy run already exists: {os.path.relpath(path, ROOT)}")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    fm = f"""schema_version: {AUTONOMY_SCHEMA_VERSION}
run_id: {args.run_id}
project: {project}
status: proposed
autonomy_level: {args.level}
goal: null
done_when: null
context_sources: []
allowed_paths: []
verification: []
max_iterations: 6
max_subagents: 6
subagents_used: 0
iteration: 0
planner_tier: premium
executor_tier: workhorse
reviewer_tier: premium
reviewer_mode: independent
completion_gate: human
started_at: null
last_checkpoint: null
completed_at: null
blocked_reason: null
completion_evidence: null
approved_by: null"""
    body = f"""
# Bounded Autonomy Run — {args.run_id}

## Context

Describe the source material, constraints, and decisions the run must preserve.

## Plan

Write independently checkable work units before starting.

## Model Routing

Record requested roles and the actual/inherited models the harness provided.

## Checkpoints

"""
    autonomy_save(path, fm, body)
    rel = os.path.relpath(path, ROOT).replace("\\", "/")
    print(f"af autonomy init: {rel} -> proposed")
    print("  complete the run contract, then run: af autonomy check <project> <run-id>")


def cmd_autonomy_check(args):
    cdir, path = autonomy_ref(args.project, args.run_id)
    project = autonomy_project_slug(cdir, require_active=False)
    issues = autonomy_issues(path, expected_project=project, require_ready=True)
    if issues:
        print(f"af autonomy check: {len(issues)} issue(s)", file=sys.stderr)
        for issue in issues:
            print(f"  - {issue}", file=sys.stderr)
        sys.exit(1)
    fm, _ = split_fm(read(path), path)
    print(f"af autonomy check: {args.run_id} is valid ({get_scalar(fm, 'status')})")


def cmd_autonomy_start(args):
    cdir, path = autonomy_ref(args.project, args.run_id)
    project = autonomy_project_slug(cdir)
    issues = autonomy_issues(path, expected_project=project, require_ready=True)
    if issues:
        die("autonomy readiness failed:\n  - " + "\n  - ".join(issues))
    fm, body = split_fm(read(path), path)
    prior = get_scalar(fm, "status")
    if prior not in {"proposed", "blocked"}:
        die(f"autonomy start requires proposed|blocked, found '{prior}'")
    if prior == "blocked" and not args.resume_reason:
        die("resuming a blocked run requires --resume-reason")
    iteration = int(get_scalar(fm, "iteration"))
    max_iterations = int(get_scalar(fm, "max_iterations"))
    if iteration >= max_iterations:
        die(f"iteration budget exhausted ({iteration}/{max_iterations}); raise max_iterations deliberately before resuming")

    when = now_iso()
    fm = set_scalar(fm, "status", "running", path)
    if get_scalar(fm, "started_at") in (None, "", "null"):
        fm = set_scalar(fm, "started_at", when, path)
    fm = set_scalar(fm, "last_checkpoint", when, path)
    fm = set_scalar(fm, "blocked_reason", "null", path)
    if prior == "blocked":
        body = autonomy_checkpoint_body(body, iteration, "resumed", args.resume_reason)
    autonomy_save(path, fm, body)
    autonomy_touch(cdir)
    event = "autonomy_started" if prior == "proposed" else "autonomy_resumed"
    detail = f"{args.run_id} running; level={get_scalar(fm, 'autonomy_level')}, budget={iteration}/{max_iterations}"
    if args.resume_reason:
        detail += f". Reason: \"{' '.join(args.resume_reason.split())}\""
    append_activity(cdir, f"{event}: {detail}")
    print(f"af autonomy start: {args.run_id} -> running ({iteration}/{max_iterations})")


def cmd_autonomy_checkpoint(args):
    cdir, path = autonomy_ref(args.project, args.run_id)
    project = autonomy_project_slug(cdir)
    issues = autonomy_issues(path, expected_project=project, require_ready=True)
    if issues:
        die("autonomy state invalid:\n  - " + "\n  - ".join(issues))
    fm, body = split_fm(read(path), path)
    if get_scalar(fm, "status") != "running":
        die(f"autonomy checkpoint requires running, found '{get_scalar(fm, 'status')}'")
    if args.outcome == "review" and not args.evidence:
        die("review checkpoint requires --evidence")
    if args.subagents_spawned < 0:
        die("--subagents-spawned must be >= 0")

    iteration = int(get_scalar(fm, "iteration")) + 1
    max_iterations = int(get_scalar(fm, "max_iterations"))
    subagents_used = int(get_scalar(fm, "subagents_used")) + args.subagents_spawned
    max_subagents = int(get_scalar(fm, "max_subagents"))
    when = now_iso()
    outcome = args.outcome
    fm = set_scalar(fm, "iteration", str(iteration), path)
    fm = set_scalar(fm, "subagents_used", str(subagents_used), path)
    fm = set_scalar(fm, "last_checkpoint", when, path)

    if subagents_used > max_subagents:
        outcome = "blocked"
        reason = f"subagent budget exceeded ({subagents_used}/{max_subagents}): {args.summary}"
        fm = set_scalar(fm, "status", "blocked", path)
        fm = set_scalar(fm, "blocked_reason", yaml_quote(reason), path)
    elif outcome == "review":
        fm = set_scalar(fm, "status", "review", path)
        fm = set_scalar(fm, "completion_evidence", yaml_quote(args.evidence), path)
    elif outcome == "blocked":
        fm = set_scalar(fm, "status", "blocked", path)
        fm = set_scalar(fm, "blocked_reason", yaml_quote(args.summary), path)
    elif iteration >= max_iterations:
        outcome = "blocked"
        reason = f"iteration budget exhausted ({iteration}/{max_iterations}): {args.summary}"
        fm = set_scalar(fm, "status", "blocked", path)
        fm = set_scalar(fm, "blocked_reason", yaml_quote(reason), path)

    body = autonomy_checkpoint_body(body, iteration, outcome, args.summary, args.evidence)
    autonomy_save(path, fm, body)
    autonomy_touch(cdir)
    if outcome == "blocked":
        append_activity(cdir, f"autonomy_blocked: {args.run_id} blocked at iteration {iteration}; {get_scalar(fm, 'blocked_reason')}")
    elif outcome == "review":
        append_activity(cdir, f"autonomy_review_ready: {args.run_id} reached review at iteration {iteration}; evidence recorded in knowledge/autonomy/{args.run_id}.md")
    print(f"af autonomy checkpoint: {args.run_id} -> {get_scalar(fm, 'status')} "
          f"({iteration}/{max_iterations} iterations; {subagents_used}/{max_subagents} subagents)")


def cmd_autonomy_finish(args):
    cdir, path = autonomy_ref(args.project, args.run_id)
    project = autonomy_project_slug(cdir)
    issues = autonomy_issues(path, expected_project=project, require_ready=True)
    if issues:
        die("autonomy state invalid:\n  - " + "\n  - ".join(issues))
    fm, body = split_fm(read(path), path)
    if get_scalar(fm, "status") != "review":
        die(f"autonomy finish requires review, found '{get_scalar(fm, 'status')}'")
    gate = get_scalar(fm, "completion_gate")
    if gate == "human" and args.approved_by != "operator":
        die("completion_gate: human requires --approved-by operator")
    if args.approved_by == "reviewer" and get_scalar(fm, "reviewer_mode") != "independent":
        die("reviewer approval requires reviewer_mode: independent")

    when = now_iso()
    fm = set_scalar(fm, "status", "complete", path)
    fm = set_scalar(fm, "completed_at", when, path)
    fm = set_scalar(fm, "last_checkpoint", when, path)
    fm = set_scalar(fm, "approved_by", args.approved_by, path)
    iteration = int(get_scalar(fm, "iteration"))
    body = autonomy_checkpoint_body(body, iteration, "complete", f"approved by {args.approved_by}")
    autonomy_save(path, fm, body)
    autonomy_touch(cdir)
    append_activity(cdir, f"autonomy_completed: {args.run_id} complete after {iteration} iteration(s); approved_by={args.approved_by}")
    print(f"af autonomy finish: {args.run_id} -> complete (approved by {args.approved_by})")


# ---------------------------------------------------------------- pipe (pipeline topology)

def pipe_pack():
    """The single pack declaring `topology: pipeline` → (domain, desc_fm, pack_dir)."""
    if not os.path.isdir(DOMAINS):
        die("library/domains/ missing")
    hits = []
    for name in sorted(os.listdir(DOMAINS)):
        desc, pack_dir = load_pack(name)
        if desc and get_scalar(desc, "topology") == "pipeline":
            hits.append((name, desc, pack_dir))
    if not hits:
        die("no domain pack declares topology: pipeline — author one before using 'af pipe'")
    if len(hits) > 1:
        die(f"multiple pipeline-topology packs ({', '.join(h[0] for h in hits)}) — the singleton board can serve only one")
    return hits[0]


def board_path():
    return os.path.join(PIPELINE, "pipeline.md")


def ensure_board(pack_dir):
    if os.path.isfile(board_path()):
        return
    skel = os.path.join(pack_dir, "pipeline-skeleton.md")
    os.path.isfile(skel) or die(f"{os.path.relpath(pack_dir, ROOT)} ships no pipeline-skeleton.md")
    os.makedirs(PIPELINE, exist_ok=True)
    write(board_path(), read(skel).format(date=today(), ts=now_iso()))


def load_board():
    os.path.isfile(board_path()) or die("no board at workspace/pipeline/pipeline.md — 'af pipe save' creates it")
    return split_fm(read(board_path()), "pipeline.md")


def write_board(fm, body):
    fm = set_scalar(fm, "last_activity", now_iso(), "pipeline.md")
    write(board_path(), join_fm(fm, body))


def pipe_rows(fm):
    """Row slugs in the board's `applications:` block."""
    m = re.search(r"^applications:\s*(\{\})?\s*$", fm, re.M)
    if not m:
        return []
    rest = fm[m.end():]
    nxt = re.search(r"^\S", rest, re.M)
    block = rest[: nxt.start() if nxt else len(rest)]
    return re.findall(r"^  ([A-Za-z0-9_-]+):\s*$", block, re.M)


def mapping_span(fm, key):
    """Span of a top-level mapping block, excluding the next top-level key/comment."""
    m = re.search(rf"^{re.escape(key)}:\s*(?:\{{\}})?\s*$", fm, re.M)
    if not m:
        return None
    rest = fm[m.end():]
    nxt = re.search(r"^(?:\S[^:]*:|# )", rest, re.M)
    return m.start(), m.end() + (nxt.start() if nxt else len(rest))


def mapping_rows(fm, key):
    span = mapping_span(fm, key)
    if not span:
        return []
    return re.findall(r"^  ([A-Za-z0-9_-]+):\s*$", fm[span[0]:span[1]], re.M)


def mapping_row_span(fm, key, slug):
    span = mapping_span(fm, key)
    if not span:
        return None
    block = fm[span[0]:span[1]]
    m = re.search(rf"^  {re.escape(slug)}:\s*$", block, re.M)
    if not m:
        return None
    start = span[0] + m.start()
    rest = fm[span[0] + m.end():span[1]]
    nxt = re.search(r"^  \S", rest, re.M)
    end = span[0] + m.end() + (nxt.start() if nxt else len(rest))
    return start, end


def mapping_row_get(fm, mapping, slug, key):
    span = mapping_row_span(fm, mapping, slug)
    if not span:
        return None
    m = re.search(rf"^    {re.escape(key)}:[ \t]*(.*?)\s*$", fm[span[0]:span[1]], re.M)
    return clean_value(m.group(1)) if m else None


def mapping_row_set(fm, mapping, slug, key, value):
    span = mapping_row_span(fm, mapping, slug)
    if not span:
        die(f"project.md: '{slug}' not found in {mapping} block")
    s, e = span
    block = fm[s:e]
    pat = re.compile(rf"^(    {re.escape(key)}:)[ \t]*.*$", re.M)
    if pat.search(block):
        block = pat.sub(rf"\g<1> {value}", block, count=1)
    else:
        block = block.rstrip("\n") + f"\n    {key}: {value}\n"
    return fm[:s] + block + fm[e:]


def mapping_row_add(fm, mapping, slug, fields):
    if mapping_row_span(fm, mapping, slug):
        die(f"project.md: '{slug}' already exists in {mapping} block")
    row = f"  {slug}:\n" + "".join(f"    {key}: {value}\n" for key, value in fields)
    span = mapping_span(fm, mapping)
    if span:
        s, e = span
        block = fm[s:e]
        if re.search(rf"^{re.escape(mapping)}:\s*\{{\}}\s*$", block, re.M):
            block = re.sub(rf"^{re.escape(mapping)}:\s*\{{\}}\s*$", f"{mapping}:\n", block, count=1, flags=re.M)
        block = block.rstrip("\n") + "\n" + row
        return fm[:s] + block + fm[e:]
    section = f"\n# AUTOMATIONS\n{mapping}:\n{row}"
    marker = re.search(r"^# COUNTERS\s*$", fm, re.M)
    if marker:
        return fm[:marker.start()].rstrip() + "\n" + section + "\n" + fm[marker.start():]
    return fm.rstrip() + "\n" + section.rstrip() + "\n"


def app_materials(afm):
    """Deliverable rows that constitute the submission; resume-only when `materials` is absent."""
    return fm_list(afm, "materials") or ["resume"]


def pipe_row_add(fm, slug, fields):
    fm = re.sub(r"^applications:\s*\{\}\s*$", "applications:", fm, count=1, flags=re.M)
    m = re.search(r"^applications:\s*$", fm, re.M)
    if not m:
        die("pipeline.md has no applications block")
    rest = fm[m.end():]
    nxt = re.search(r"^\S", rest, re.M)
    end = m.end() + (nxt.start() if nxt else len(rest))
    row = f"  {slug}:\n" + "".join(f"    {k}: {v}\n" for k, v in fields.items() if v not in (None, ""))
    head = fm[:end]
    if not head.endswith("\n"):
        head += "\n"
    return head + row + fm[end:]


def yaml_str(v):
    return f'"{v}"' if v else None


def app_dir(slug):
    return os.path.join(PIPELINE, "applications", slug)


def jd_cache_path(slug):
    return os.path.join(PIPELINE, "scout", "jd-cache", f"{slug}.jd.md")


def cmd_pipe_save(args):
    domain, desc, pack_dir = pipe_pack()
    ensure_board(pack_dir)
    slug = args.slug or re.sub(r"[^a-z0-9]+", "-", f"{args.company}-{args.role}".lower()).strip("-")
    re.match(r"^[a-z0-9][a-z0-9-]*$", slug) or die(f"derived slug '{slug}' is not folder-safe — pass --slug")
    fm, body = load_board()
    if row_span(fm, slug):
        die(f"board row '{slug}' already exists")
    fm = pipe_row_add(fm, slug, {
        "stage": "saved", "company": yaml_str(args.company), "role": yaml_str(args.role),
        "url": args.url, "ats": args.ats, "source": args.source, "posted": args.posted,
        "deadline": args.deadline, "salary": yaml_str(args.salary), "saved": today()})
    write_board(fm, body)
    print(f"af pipe save: {slug} -> saved (board row; no folder until start)")
    print("\nJudgment (stays with the agent):")
    print(f"  - Cache the verbatim JD at workspace/pipeline/scout/jd-cache/{slug}.jd.md now — postings vanish.")
    print(f"  - Committing to it? 'af pipe start {slug}' scaffolds the sprint folder.")


def cmd_pipe_start(args):
    domain, desc, pack_dir = pipe_pack()
    fm, body = load_board()
    slug = args.slug
    row_span(fm, slug) or die(f"no board row '{slug}' — 'af pipe save' it first")
    cur = row_get(fm, slug, "stage")
    if cur != "saved":
        die(f"'{slug}' is at stage '{cur}' — start applies to 'saved' rows only")
    adir = app_dir(slug)
    if os.path.exists(adir):
        die(f"{adir} already exists")
    skel = os.path.join(pack_dir, "skeleton.md")
    os.path.isfile(skel) or die(f"domain '{domain}' ships no skeleton.md")

    company = row_get(fm, slug, "company") or "?"
    role = row_get(fm, slug, "role") or "?"
    name = f"{company} - {role}"
    os.makedirs(adir)
    write(os.path.join(adir, "application.md"), read(skel).format(
        name=name, slug=slug, date=today(), ts=now_iso(), company=company, role=role,
        url=row_get(fm, slug, "url") or "null", source=row_get(fm, slug, "source") or "manual",
        ats=row_get(fm, slug, "ats") or "unknown", posted=row_get(fm, slug, "posted") or "null",
        salary=row_get(fm, slug, "salary") or ""))

    jd_note = "no cached JD found — capture jd.md verbatim before mapping"
    if os.path.isfile(jd_cache_path(slug)):
        shutil.move(jd_cache_path(slug), os.path.join(adir, "jd.md"))
        jd_note = "cached JD moved in as jd.md"
    fm = row_set(fm, slug, "stage", "preparing")
    write_board(fm, body)
    append_activity(adir, f"application_started: {name} scaffolded ({domain}); {jd_note}")

    print(f"af pipe start: workspace/pipeline/applications/{slug}/ scaffolded ({jd_note})")
    print("\nJudgment (stays with the agent):")
    print(f"  - Load the runbook: library/domains/{domain}/production.md (brief -> map -> tailor -> verify -> export).")
    print("  - The jd-map's gap stop and coverage choice are operator conversations, not drafting problems.")


def cmd_pipe_stage(args):
    fm, body = load_board()
    slug, new = args.slug, args.stage
    row_span(fm, slug) or die(f"no board row '{slug}'")
    new in PIPE_STAGES or die(f"unknown stage '{new}' (stages: {', '.join(PIPE_STAGES)})")
    cur = row_get(fm, slug, "stage")
    legal = PIPE_TRANSITIONS.get(cur, set())
    if new not in legal:
        die(f"illegal transition {cur} -> {new}" + (f" (legal from {cur}: {', '.join(sorted(legal))})" if legal else f" ({cur} is terminal)"))

    notes = []
    fm = row_set(fm, slug, "stage", new)
    if new == "applied":
        fm = row_set(fm, slug, "applied", today())
        fm = row_set(fm, slug, "next_nudge", (datetime.date.today() + datetime.timedelta(days=PIPE_NUDGE_DAYS)).isoformat())
        adir = app_dir(slug)
        ap = os.path.join(adir, "application.md")
        if os.path.isfile(ap):
            afm, _ = split_fm(read(ap), "application.md")
            mats = app_materials(afm)
            rel, st = row_get(afm, mats[0], "file"), row_get(afm, mats[0], "status")
            m = re.search(r"-v(\d+)\.md$", rel or "")
            if m and st in ("ready", "published"):
                fm = row_set(fm, slug, "shipped", f"v{m.group(1)}")
            else:
                notes.append(f"primary material '{mats[0]}' is '{st}' — shipped left unset (ready + export before submitting next time)")
            for mat in mats[1:]:
                mst = row_get(afm, mat, "status")
                if mst not in ("ready", "published"):
                    notes.append(f"material '{mat}' is '{mst}'")
    elif new == "interviewing":
        fm = row_set(fm, slug, "next_nudge", (datetime.date.today() + datetime.timedelta(days=PIPE_NUDGE_DAYS)).isoformat())
    else:
        fm = row_set(fm, slug, "next_nudge", "null")
    write_board(fm, body)
    if os.path.isdir(app_dir(slug)):
        append_activity(app_dir(slug), f"stage: {cur} -> {new}" + (f"; {'; '.join(notes)}" if notes else ""))

    print(f"af pipe stage: {slug} {cur} -> {new}" + (f" ({'; '.join(notes)})" if notes else ""))
    print("\nJudgment (stays with the agent):")
    if new == "applied":
        print("  - Note the submission channel in application.md (direct career site beats boards for ranking).")
    if new in ("offer", "rejected"):
        print("  - Anything worth banking? Run career-harvest while the evidence is fresh (library/process/career-harvest.md).")
    if new == "interviewing":
        print("  - Prep from the jd-map + stories, not the resume; refresh company-brief '## Now' if it is >30 days old.")


def cmd_pipe_board(args):
    if not os.path.isfile(board_path()):
        print("af pipe board: no pipeline yet — 'af pipe save' opens the first row")
        return
    fm, _ = load_board()
    rows = pipe_rows(fm)
    if not rows:
        print("af pipe board: board is empty")
        return
    order = {s: i for i, s in enumerate(PIPE_STAGES)}
    cols = ("stage", "company", "role", "deadline", "applied", "next_nudge", "shipped")
    table = [[s] + [row_get(fm, s, c) or "-" for c in cols] for s in sorted(rows, key=lambda r: (order.get(row_get(fm, r, "stage"), 99), r))]
    widths = [max(len(r[i]) for r in table + [["slug"] + list(cols)]) for i in range(len(cols) + 1)]
    header = ["slug"] + list(cols)
    print("  ".join(h.ljust(w) for h, w in zip(header, widths)))
    for r in table:
        print("  ".join(v.ljust(w) for v, w in zip(r, widths)))


def check_pipeline():
    """Board/application invariants (issues) + follow-up/staleness alarms (notes)."""
    issues, notes = [], []
    if not os.path.isfile(board_path()):
        return issues, notes
    fm, _ = split_fm(read(board_path()), "pipeline.md")
    today_d = datetime.date.today()
    rows = pipe_rows(fm)

    apps_root = os.path.join(PIPELINE, "applications")
    folders = sorted(d for d in (os.listdir(apps_root) if os.path.isdir(apps_root) else [])
                     if os.path.isfile(os.path.join(apps_root, d, "application.md")))
    for d in folders:
        if d not in rows:
            issues.append(f"workspace/pipeline/applications/{d}: folder has no board row")

    rules = None
    if os.path.isdir(DOMAINS):
        for name in sorted(os.listdir(DOMAINS)):
            desc, pack_dir = load_pack(name)
            if desc and get_scalar(desc, "topology") == "pipeline":
                rules = load_rules(pack_dir)
                break

    for slug in rows:
        stage = row_get(fm, slug, "stage")
        if stage not in PIPE_STAGES:
            issues.append(f"pipeline.md: row '{slug}' stage '{stage}' invalid")
            continue
        adir = app_dir(slug)
        started = os.path.isfile(os.path.join(adir, "application.md"))
        if stage not in ("saved", "dropped") and not started:
            issues.append(f"pipeline.md: row '{slug}' is '{stage}' but has no application folder")
        if stage == "saved":
            saved = parse_iso_date(row_get(fm, slug, "saved"))
            if saved and (today_d - saved).days > PIPE_STALE_SAVED_DAYS:
                notes.append(f"pipeline.md: '{slug}' saved {(today_d - saved).days}d ago — start it or drop it (ghost-job window passed)")
        if stage in ("applied", "interviewing"):
            nudge = parse_iso_date(row_get(fm, slug, "next_nudge"))
            if nudge and nudge <= today_d:
                notes.append(f"pipeline.md: '{slug}' follow-up due since {nudge.isoformat()} ({stage})")
        if not started:
            continue

        rel = f"workspace/pipeline/applications/{slug}"
        afm, _ = split_fm(read(os.path.join(adir, "application.md")), "application.md")
        for field in ("name", "slug", "schema_version", "created_at", "domain", "company", "role", "job_url", "last_activity"):
            if get_scalar(afm, field) in (None, ""):
                issues.append(f"{rel}: required field '{field}' missing")
        for mat in app_materials(afm):
            if mat not in all_rows(afm):
                issues.append(f"{rel}: materials names '{mat}' but no such deliverable row exists")
        if get_scalar(afm, "slug") != slug:
            issues.append(f"{rel}: slug '{get_scalar(afm, 'slug')}' != folder name")
        if stage != "saved" and not os.path.isfile(os.path.join(adir, "jd.md")):
            issues.append(f"{rel}: jd.md missing — tailoring without the verbatim posting is guesswork")
        for dslug in all_rows(afm):
            st, f = row_get(afm, dslug, "status"), row_get(afm, dslug, "file")
            if st not in STATUS_ENUM:
                issues.append(f"{rel}: row '{dslug}' status '{st}' invalid")
            if not f:
                issues.append(f"{rel}: row '{dslug}' has no file pointer")
                continue
            p = os.path.join(adir, f)
            if st != "not_started" and not os.path.isfile(p):
                issues.append(f"{rel}: row '{dslug}' file missing: {f}")
            elif os.path.isfile(p):
                m = re.fullmatch(r"(.+)-v(\d+)\.md", os.path.basename(p))
                if m and int(m.group(2)) != max(versions_in(os.path.dirname(p), m.group(1))):
                    issues.append(f"{rel}: row '{dslug}' points at v{m.group(2)} but head is v{max(versions_in(os.path.dirname(p), m.group(1)))}")
        issues += media_manifest_issues_for_fm(adir, afm, "application.md")
        if rules and hasattr(rules, "check_application"):
            r_issues, r_notes = rules.check_application(make_ctx(), adir, afm)
            issues += r_issues
            notes += r_notes
    return issues, notes


# ---------------------------------------------------------------- doctor

# Dream-pass nudge thresholds. Doctor owns WHEN to nudge (deterministic
# mechanics); the project-consolidate skill owns what a dream pass does.
DREAM_AGE_DAYS = 30            # once consolidated, active project this far past the stamp → nudge
DREAM_ACTIVE_WINDOW_DAYS = 14  # ...but only if it saw activity this recently
ACTIVITY_LINE_CAP = 200        # chars; longer reads as narration, not an event line
ACTIVITY_LINE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}( \d{2}:\d{2})?\s*[—-]\s*[a-z][a-z0-9_]*:\s")
DREAM_LINE_CAPS = (("knowledge/decision-log.md", 300),
                   ("knowledge/raid-log.md", 300),
                   ("activity.md", 500),
                   ("project.md", 250))
MEDIA_MANIFEST_FIELDS = ("shipped_media", "exports")
MEDIA_PREVIEW_EXTS = {".html", ".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg",
                      ".pdf", ".mp4", ".mov", ".webm", ".pptx", ".docx"}


def parse_iso_date(value):
    if not value or value == "null":
        return None
    try:
        return datetime.date.fromisoformat(value[:10])
    except ValueError:
        return None


def fm_list_values(fm, key):
    """Parse inline or block-list frontmatter values for a key."""
    inline = fm_list(fm, key)
    if inline:
        return inline
    m = re.search(rf"^\s*{re.escape(key)}:\s*$", fm, re.M)
    if not m:
        return []
    rest = fm[m.end():]
    nxt = re.search(r"^\S", rest, re.M)
    block = rest[: nxt.start() if nxt else len(rest)]
    values = []
    for line in block.splitlines():
        item = re.match(r"^\s*-\s*(.*?)\s*$", line)
        if item:
            values.append(clean_value(item.group(1)))
    return [v for v in values if v]


def list_from_value(value):
    value = clean_value(value or "")
    if not value:
        return []
    if value.startswith("[") and value.endswith("]"):
        return [clean_value(i) for i in value[1:-1].split(",") if clean_value(i)]
    return [value]


def manifest_path_status(cdir, owner_rel, raw):
    """None when valid/skipped; otherwise a short error label."""
    value = clean_value(raw)
    if not value or value.startswith(("http://", "https://")):
        return None
    croot = os.path.abspath(cdir)

    def inside(path):
        candidate = os.path.abspath(path)
        try:
            if os.path.commonpath([croot, candidate]) != croot:
                return None
        except ValueError:
            return None
        return candidate

    if os.path.isabs(value):
        candidates = [inside(value)]
        outside = candidates[0] is None
    else:
        owner_dir = os.path.dirname(owner_rel)
        candidates = [inside(os.path.join(cdir, value)),
                      inside(os.path.join(cdir, owner_dir, value))]
        outside = not any(candidates)
    candidates = [c for c in candidates if c]
    if outside:
        return "resolves outside project"
    if not any(os.path.isfile(c) for c in candidates):
        return "missing"
    return None


def media_manifest_issues_for_fm(cdir, cfm, source_label="project.md"):
    issues = []
    rel = os.path.relpath(cdir, ROOT).replace("\\", "/")
    extra = pack_exportables(cfm)
    for slug in all_rows(cfm):
        st, f = row_get(cfm, slug, "status"), row_get(cfm, slug, "file")
        if st not in ("ready", "published") or not f or not os.path.isfile(os.path.join(cdir, f)):
            continue
        if not f.lower().endswith(".md"):
            continue
        try:
            parsed = split_fm_optional(read(os.path.join(cdir, f)))
        except (UnicodeDecodeError, OSError):
            parsed = None
        if parsed is None:
            issues.append(f"{rel}: {source_label} row '{slug}' file has no/invalid frontmatter or is unreadable: {f}")
            continue
        dfm, _ = parsed
        if exportable_ingredient(f, extra) and not fm_list_values(dfm, "exports"):
            issues.append(f"{rel}: {source_label} row '{slug}' is a {st} exportable deliverable with empty exports[] - land the finals in the deliverable's media/ folder and record them")
        for field in MEDIA_MANIFEST_FIELDS:
            for value in fm_list_values(dfm, field):
                status = manifest_path_status(cdir, f, value)
                if status:
                    issues.append(f"{rel}: {source_label} row '{slug}' {field} path {status}: {value}")
    return issues


def media_manifest_issues(cdir, cfm):
    issues = media_manifest_issues_for_fm(cdir, cfm)
    archive = os.path.join(cdir, "knowledge", "_archive", "deliverables-archive.md")
    if os.path.isfile(archive):
        afm, _ = split_fm(read(archive), "deliverables-archive.md")
        issues += media_manifest_issues_for_fm(cdir, afm, "deliverables-archive.md")
    return issues


def project_manifest_ingredients(cfm):
    m = re.search(r"^\s*ingredients:\s*\[(.*?)\]\s*$", cfm, re.M)
    if not m:
        return []
    return [clean_value(i) for i in m.group(1).split(",") if clean_value(i)]


def has_previewable_file(folder):
    if not os.path.isdir(folder):
        return False
    for _, dirnames, filenames in os.walk(folder):
        dirnames[:] = [d for d in dirnames if not d.startswith(".") and d not in ("history", "raw")]
        for name in filenames:
            if os.path.splitext(name)[1].lower() in MEDIA_PREVIEW_EXTS:
                return True
    return False


def media_manifest_notes(cdir):
    notes = []
    rel = os.path.relpath(cdir, ROOT).replace("\\", "/")
    try:
        cfm, _ = split_fm(read(os.path.join(cdir, "project.md")), "project.md")
    except SystemExit:
        return notes
    project_ingredients = project_manifest_ingredients(cfm)
    for slug in all_rows(cfm):
        if not re.match(r"post-\d+$", slug) or row_get(cfm, slug, "status") != "published":
            continue
        f = row_get(cfm, slug, "file")
        if not f or not os.path.isfile(os.path.join(cdir, f)):
            continue
        dfm, _ = split_fm(read(os.path.join(cdir, f)), f)
        if fm_list_values(dfm, "shipped_media"):
            continue
        row_ingredients = list_from_value(row_get(cfm, slug, "ingredients"))
        ingredients = row_ingredients or project_ingredients
        post_dir = os.path.dirname(os.path.join(cdir, f))
        image_bearing = (
            "image-prompts" in ingredients
            or has_previewable_file(os.path.join(post_dir, "visuals"))
            or has_previewable_file(os.path.join(post_dir, "media"))
        )
        if image_bearing:
            notes.append(f"{rel}: published {slug} has imagery signals but empty shipped_media[] - land shipped media in the post folder and record it on post-FINAL.md")
    return notes


def dream_note(cdir):
    """One informational line when a project looks due for a dream pass.

    Never an issue and never affects the exit code: bloated-but-valid books
    are still valid. Fires on an active project when an append-only file
    exceeds its cap, or when consolidation is >DREAM_AGE_DAYS old and the
    project is still being worked (last_activity within the window).
    """
    rel = os.path.relpath(cdir, ROOT).replace("\\", "/")
    try:
        cfm, _ = split_fm(read(os.path.join(cdir, "project.md")), "project.md")
    except SystemExit:
        return None
    if get_scalar(cfm, "status") != "active":
        return None

    today_d = datetime.date.today()
    signals = []
    for relpath, cap in DREAM_LINE_CAPS:
        p = os.path.join(cdir, relpath)
        if os.path.isfile(p):
            lines = read(p).count("\n") + 1
            if lines > cap:
                signals.append(f"{relpath} {lines} lines (cap {cap})")

    base = parse_iso_date(get_scalar(cfm, "last_consolidated"))
    age = (today_d - base).days if base else None
    last_act = parse_iso_date(get_scalar(cfm, "last_activity"))
    steadily_worked = last_act is not None and (today_d - last_act).days <= DREAM_ACTIVE_WINDOW_DAYS
    if age is not None and age >= DREAM_AGE_DAYS and steadily_worked:
        signals.insert(0, f"{age}d since last consolidation")

    if not signals:
        return None
    return f"{rel}: dream pass recommended — " + "; ".join(signals) + " (offer system/skills/project-consolidate)"


def activity_notes(cdir):
    """Shape lint for activity.md — drift alarms, never issues. Checks shape
    only (timestamp/event-type prefix, line length), never event vocabulary,
    so any domain can introduce event types freely. Active projects only —
    completed history won't change, and a permanent alarm is noise (same
    contract as dream_note)."""
    path = os.path.join(cdir, "activity.md")
    if not os.path.isfile(path):
        return []
    try:
        cfm, _ = split_fm(read(os.path.join(cdir, "project.md")), "project.md")
    except SystemExit:
        return []
    if get_scalar(cfm, "status") != "active":
        return []
    rel = os.path.relpath(cdir, ROOT).replace("\\", "/")
    unshaped, overlong = 0, 0
    for raw in read(path).splitlines():
        s = raw.strip()
        if not s or s.startswith("#") or s.startswith("- ["):
            continue  # blank, heading, or Attention bullet
        if not ACTIVITY_LINE_RE.match(s):
            unshaped += 1
        if len(s) > ACTIVITY_LINE_CAP:
            overlong += 1
    notes = []
    if unshaped:
        notes.append(f"{rel}: activity.md {unshaped} line(s) not shaped 'YYYY-MM-DD [HH:MM] — event_type: ...' — one line per material event")
    if overlong:
        notes.append(f"{rel}: activity.md {overlong} line(s) over {ACTIVITY_LINE_CAP} chars — iteration narration belongs in the version files' changes_from_vN, not here")
    return notes


def check_project(cdir):
    issues = []
    rel = os.path.relpath(cdir, ROOT).replace("\\", "/")
    try:
        cfm, _ = split_fm(read(os.path.join(cdir, "project.md")), "project.md")
    except SystemExit:
        return [f"{rel}: project.md missing or has no frontmatter"]

    required = (
        "name", "slug", "schema_version", "created_at", "domain",
        "status", "current_phase", "flow", "last_activity",
    )
    for field in required:
        if get_scalar(cfm, field) in (None, ""):
            issues.append(f"{rel}: required field '{field}' missing")
    if not has_field(cfm, "deliverables"):
        issues.append(f"{rel}: required field 'deliverables' missing")
    if get_scalar(cfm, "schema_version") != PROJECT_SCHEMA_VERSION:
        issues.append(f"{rel}: schema_version must be {PROJECT_SCHEMA_VERSION}; migrate the project forward")
    if get_scalar(cfm, "slug") not in (None, os.path.basename(cdir)):
        issues.append(f"{rel}: slug '{get_scalar(cfm, 'slug')}' != folder name")
    lifecycle = get_scalar(cfm, "status")
    if lifecycle not in LIFECYCLE_ENUM:
        issues.append(f"{rel}: lifecycle status '{lifecycle}' not in {sorted(LIFECYCLE_ENUM)}")
    completed_at = get_scalar(cfm, "completed_at")
    cancelled_at = get_scalar(cfm, "cancelled_at")
    if lifecycle == "complete" and completed_at in (None, "null", ""):
        issues.append(f"{rel}: status complete requires completed_at")
    if lifecycle == "cancelled" and cancelled_at in (None, "null", ""):
        issues.append(f"{rel}: status cancelled requires cancelled_at")
    if lifecycle != "complete" and completed_at not in (None, "null", ""):
        issues.append(f"{rel}: completed_at is present but status is {lifecycle}")
    if lifecycle != "cancelled" and cancelled_at not in (None, "null", ""):
        issues.append(f"{rel}: cancelled_at is present but status is {lifecycle}")

    # Domain pack: core checks always; the pack's frontmatter extension + rules apply for its domain.
    domain = get_scalar(cfm, "domain")
    desc, pack_dir = load_pack(domain)
    if domain and not desc:
        issues.append(f"{rel}: domain '{domain}' has no pack at library/domains/{domain}/")
    if desc:
        for f in fm_list(desc, "extension_fields"):
            if not has_field(cfm, f):
                issues.append(f"{rel}: domain '{domain}' requires field '{f}' (missing)")
    flow = get_scalar(cfm, "flow")
    if flow and flow not in FLOWS:
        issues.append(f"{rel}: flow '{flow}' is not registered in system/af.py")

    for slug in all_rows(cfm):
        st, f = row_get(cfm, slug, "status"), row_get(cfm, slug, "file")
        if st not in STATUS_ENUM:
            issues.append(f"{rel}: row '{slug}' status '{st}' invalid")
        if not f or f == "null":
            issues.append(f"{rel}: row '{slug}' has no file pointer")
            continue
        assembly = domain == "marketing" and re.fullmatch(r"post-\d+", slug) and f.endswith("post-FINAL.md")
        if not assembly and not re.fullmatch(r".+-v\d+\.md", os.path.basename(f)):
            issues.append(f"{rel}: row '{slug}' file is not a numeric version head: {f}")
        p = os.path.join(cdir, f)
        if st == "not_started":
            pass
        elif not os.path.isfile(p):
            issues.append(f"{rel}: row '{slug}' file missing: {f}")
        else:
            try:
                parsed = split_fm_optional(read(p))
            except (UnicodeDecodeError, OSError):
                parsed = None
            if parsed is None:
                issues.append(f"{rel}: row '{slug}' file has no/invalid frontmatter or is unreadable: {f}")
                continue
            artifact_status = get_scalar(parsed[0], "status")
            if artifact_status != st:
                issues.append(f"{rel}: row '{slug}' status {st} != artifact status {artifact_status}: {f}")
            m = re.fullmatch(r"(.+)-v(\d+)\.md", os.path.basename(p))
            if m:
                highest = max(versions_in(os.path.dirname(p), m.group(1)))
                if int(m.group(2)) != highest:
                    issues.append(f"{rel}: row '{slug}' points at v{m.group(2)} but head is v{highest}")

    if get_scalar(cfm, "status") == "complete":
        unfinished = [s for s in all_rows(cfm) if row_get(cfm, s, "status") in ("not_started", "drafting")]
        if unfinished:
            issues.append(f"{rel}: completed project has unfinished rows: {', '.join(unfinished)}")

    # Channels and stakeholders validation
    channels = fm_list(cfm, "channels")
    for c_slug in channels:
        c_file = os.path.join(ROOT, "library", "context", "channels", c_slug, "profile.md")
        if not os.path.isfile(c_file):
            issues.append(f"{rel}: channel '{c_slug}' referenced in frontmatter but library/context/channels/{c_slug}/profile.md does not exist")

    stakeholders = fm_list(cfm, "stakeholders")
    for s_slug in stakeholders:
        s_file = os.path.join(ROOT, "library", "context", "people", s_slug, "profile.md")
        if not os.path.isfile(s_file):
            issues.append(f"{rel}: stakeholder '{s_slug}' referenced in frontmatter but library/context/people/{s_slug}/profile.md does not exist")
        overlay_path = os.path.join(cdir, "knowledge", "people", f"{s_slug}.md")
        if not os.path.isfile(overlay_path):
            issues.append(f"{rel}: stakeholder '{s_slug}' per-project overlay missing at knowledge/people/{s_slug}.md")

    rules = load_rules(pack_dir)
    if rules and hasattr(rules, "check"):
        issues += rules.check(make_ctx(), cdir, cfm)
    issues += media_manifest_issues(cdir, cfm)
    autonomy_dir = os.path.join(cdir, "knowledge", "autonomy")
    if os.path.isdir(autonomy_dir):
        for path in sorted(glob.glob(os.path.join(autonomy_dir, "*.md"))):
            issues += autonomy_issues(path, expected_project=get_scalar(cfm, "slug"), require_ready=False)
    issues += automation_issues(cdir, cfm)
    return issues


# System-surface checks (full doctor runs only). Same contract as dream notes:
# issues are broken invariants (dead links), notes are drift alarms the
# operator judges (size budgets, voice mirror staleness) — never auto-fixed.
PERSONA_WORD_BUDGET = 1700    # AGENTS.builder.md / AGENTS.operator.md
PROCESS_WORD_BUDGET = 1500    # library/process/**/*.md
TEMPLATE_WORD_BUDGET = 900    # */deliverables/*/template.md

VOICE_LIVE = os.path.join(ROOT, "library", "context", "operator", "voice")
VOICE_SCHEMA = os.path.join(ROOT, "library", "context", "operator-schema", "voice")

LINK_SCAN_GLOBS = ("AGENTS.md", "AGENTS.builder.md", "AGENTS.operator.md", "AGENTS.daemon.md",
                   "library/process/**/*.md", "library/deliverables/**/*.md",
                   "library/domains/**/*.md", "library/assets/README.md",
                   "system/audit/README.md", "system/skills/README.md",
                   "system/skills/*/SKILL.md")

LINK_PATTERN = re.compile(r"\[[^\]]*\]\(([^)\s]+)\)")


def link_scan_files():
    seen = []
    for pat in LINK_SCAN_GLOBS:
        for p in sorted(glob.glob(os.path.join(ROOT, pat), recursive=True)):
            if os.path.isfile(p) and p not in seen:
                seen.append(p)
    return seen


def dead_link_issues():
    issues = []
    for path in link_scan_files():
        rel = os.path.relpath(path, ROOT).replace("\\", "/")
        # Code spans and fenced blocks hold template strings, not navigable links.
        text = re.sub(r"```.*?```", "", read(path), flags=re.S)
        text = re.sub(r"`[^`\n]*`", "", text)
        for target in LINK_PATTERN.findall(text):
            if target.startswith(("http://", "https://", "mailto:", "#", "data:")) or "{" in target:
                continue
            bare = target.split("#")[0]
            if not bare:
                continue
            local = os.path.normpath(os.path.join(os.path.dirname(path), bare))
            rooted = os.path.normpath(os.path.join(ROOT, bare))
            rel_target = os.path.relpath(local, ROOT).replace("\\", "/")
            personal_context = rel_target.startswith("library/context/") and not rel_target.startswith(
                ("library/context/_meta/", "library/context/operator-schema/")
            )
            if personal_context or rel_target.startswith("system/builder-backlog"):
                continue
            if not (os.path.exists(local) or os.path.exists(rooted)):
                issues.append(f"{rel}: dead link -> {target}")
    return issues


def budget_notes():
    notes = []
    targets = [(os.path.join(ROOT, f), PERSONA_WORD_BUDGET, "persona")
               for f in ("AGENTS.builder.md", "AGENTS.operator.md", "AGENTS.daemon.md")]
    targets += [(p, PROCESS_WORD_BUDGET, "process file")
                for p in sorted(glob.glob(os.path.join(ROOT, "library", "process", "**", "*.md"), recursive=True))]
    targets += [(p, TEMPLATE_WORD_BUDGET, "template")
                for p in sorted(glob.glob(os.path.join(ROOT, "library", "deliverables", "*", "template.md")))
                + sorted(glob.glob(os.path.join(ROOT, "library", "domains", "*", "deliverables", "*", "template.md")))]
    for path, cap, label in targets:
        if not os.path.isfile(path):
            continue
        words = len(read(path).split())
        if words > cap:
            rel = os.path.relpath(path, ROOT).replace("\\", "/")
            notes.append(f"size budget: {rel} {words} words (cap {cap}, {label}) — diet before adding")
    return notes


def voice_mirror_notes():
    """Staleness alarm for the agnostic voice method surface.

    Method surface = root-level *.md plus any */README.md under the live
    voice tree; deeper files are personal content and never mirrored. A live
    file newer than its schema mirror means an unmirrored method change OR a
    personal-only edit — mirroring or touching the schema file clears it.
    """
    notes = []
    if not (os.path.isdir(VOICE_LIVE) and os.path.isdir(VOICE_SCHEMA)):
        return notes
    method = sorted(os.path.basename(p) for p in glob.glob(os.path.join(VOICE_LIVE, "*.md")))
    method += sorted(os.path.relpath(p, VOICE_LIVE).replace("\\", "/")
                     for p in glob.glob(os.path.join(VOICE_LIVE, "*", "README.md")))
    shared = sorted(os.path.relpath(p, VOICE_LIVE).replace("\\", "/")
                    for p in glob.glob(os.path.join(VOICE_LIVE, "**", "*.md"), recursive=True)
                    if os.path.isfile(os.path.join(VOICE_SCHEMA, os.path.relpath(p, VOICE_LIVE))))
    for rel in method:
        if not os.path.isfile(os.path.join(VOICE_SCHEMA, rel)):
            notes.append(f"voice mirror: {rel} has no mirror under library/context/operator-schema/voice/ "
                         "— mirror the agnostic method surface or move personal content out of the method slot")
    for rel in shared:
        live, schema = os.path.join(VOICE_LIVE, rel), os.path.join(VOICE_SCHEMA, rel)
        if os.path.getmtime(live) > os.path.getmtime(schema):
            notes.append(f"voice mirror: {rel} modified after its schema mirror — mirror the agnostic "
                         "change or touch the schema file to confirm personal-only")
    return notes


def ppt_master_stray_issues(base=None):
    """Deck artifacts inside the vendored ppt-master tree are strays: the
    vendor refresh procedure wipes the tree and .gitignore hides them.
    system/hooks/ppt_master_guard.py blocks creating them; this is the
    backstop for anything that slipped past the hooks."""
    root = base or ROOT
    skill = os.path.join(root, "system", "skills", "ppt-master")
    issues = []
    if not os.path.isdir(skill):
        return issues
    for dirpath, dirnames, filenames in os.walk(skill):
        rel = os.path.relpath(dirpath, root).replace("\\", "/")
        if "projects" in dirnames:
            issues.append(f"{rel}/projects: deck project staged inside the vendored skill tree — "
                          "move it under the calling campaign (see system/skills/ppt-master/AGENTS.md)")
            dirnames.remove("projects")
        for f in sorted(filenames):
            if f.lower().endswith(".pptx"):
                issues.append(f"{rel}/{f}: exported deck inside the vendored skill tree — "
                              "promote it to the calling deliverable folder, then clear the stray")
    return issues


HARNESS_MANIFEST = os.path.join("system", "harnesses", "manifest.json")
PROJECTION_MARKER = ".agentframe-projection.json"
PROJECTION_MANIFEST = ".agentframe-manifest.json"
PROJECTION_GENERATOR = "AgentFrame af sync-harnesses"


def _sha256(data):
    return hashlib.sha256(data).hexdigest()


def _projection_config(root=ROOT):
    path = Path(root) / HARNESS_MANIFEST
    try:
        config = json.loads(path.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(f"cannot load {HARNESS_MANIFEST}: {exc}") from exc
    if config.get("schema_version") != 1:
        raise ValueError(f"{HARNESS_MANIFEST}: unsupported schema_version")
    if not config.get("skills") or not config.get("targets"):
        raise ValueError(f"{HARNESS_MANIFEST}: skills and targets are required")
    return path, config


def _tree_hashes(folder):
    folder = Path(folder)
    hashes = {}
    for path in sorted(p for p in folder.rglob("*") if p.is_file()):
        rel = path.relative_to(folder).as_posix()
        hashes[rel] = _sha256(_projection_bytes(path))
    return hashes


def _bundle_hash(file_hashes):
    payload = "".join(f"{name}\0{digest}\n" for name, digest in sorted(file_hashes.items()))
    return _sha256(payload.encode("utf-8"))


def _projection_bytes(path):
    """Normalize UTF-8 text EOLs while leaving binary bytes exact."""
    data = Path(path).read_bytes()
    try:
        text = data.decode("utf-8")
    except UnicodeDecodeError:
        return data
    return text.replace("\r\n", "\n").encode("utf-8")


def _build_harness_projections(root, build_root):
    root, build_root = Path(root), Path(build_root)
    config_path, config = _projection_config(root)
    canonical_root = root / config["canonical_root"]
    config_hash = _sha256(_projection_bytes(config_path))
    overlays = config.get("overlays", {})
    built = {}

    for harness, target_rel in sorted(config["targets"].items()):
        target_build = build_root / harness
        target_build.mkdir(parents=True)
        skill_records = {}
        for skill in config["skills"]:
            source = canonical_root / skill
            if not (source / "SKILL.md").is_file():
                raise ValueError(f"canonical skill is missing SKILL.md: {source}")
            destination = target_build / skill
            shutil.copytree(source, destination)

            overlay_rel = overlays.get(harness, {}).get(skill)
            if overlay_rel:
                overlay = root / overlay_rel
                if not overlay.is_dir():
                    raise ValueError(f"projection overlay does not exist: {overlay_rel}")
                shutil.copytree(overlay, destination, dirs_exist_ok=True)

            file_hashes = _tree_hashes(destination)
            record = {
                "source": f"{config['canonical_root']}/{skill}",
                "overlay": overlay_rel,
                "bundle_sha256": _bundle_hash(file_hashes),
                "files": file_hashes,
            }
            marker = {
                "generated_by": PROJECTION_GENERATOR,
                "regenerate": "python system/af.py sync-harnesses --write",
                "do_not_edit": True,
                **record,
            }
            (destination / PROJECTION_MARKER).write_text(
                json.dumps(marker, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
            )
            skill_records[skill] = record

        root_manifest = {
            "schema_version": 1,
            "generated_by": PROJECTION_GENERATOR,
            "regenerate": "python system/af.py sync-harnesses --write",
            "source_manifest": HARNESS_MANIFEST.replace("\\", "/"),
            "source_manifest_sha256": config_hash,
            "harness": harness,
            "skills": skill_records,
        }
        (target_build / PROJECTION_MANIFEST).write_text(
            json.dumps(root_manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
        )
        built[harness] = (target_build, root / target_rel)
    return built


def _projection_drift(expected, actual):
    expected, actual = Path(expected), Path(actual)
    if not actual.is_dir():
        return [f"missing projection directory: {actual}"]
    expected_files = {
        p.relative_to(expected).as_posix(): p for p in expected.rglob("*") if p.is_file()
    }
    actual_files = {
        p.relative_to(actual).as_posix(): p for p in actual.rglob("*") if p.is_file()
    }
    issues = []
    for rel in sorted(expected_files.keys() - actual_files.keys()):
        issues.append(f"missing generated file: {actual / rel}")
    for rel in sorted(actual_files.keys() - expected_files.keys()):
        issues.append(f"unexpected generated file: {actual / rel}")
    for rel in sorted(expected_files.keys() & actual_files.keys()):
        if _projection_bytes(expected_files[rel]) != _projection_bytes(actual_files[rel]):
            issues.append(f"drifted generated file: {actual / rel}")
    return issues


def _remove_projection_path(path):
    path = Path(path)
    if path.is_dir():
        shutil.rmtree(path)
    elif path.exists():
        path.unlink()


def _validate_projection_destination(path):
    path = Path(path)
    if not path.exists():
        return
    marker_path = path / PROJECTION_MARKER
    try:
        marker = json.loads(marker_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError(
            f"refusing to replace non-generated skill directory: {path}"
        ) from exc
    if marker.get("generated_by") != PROJECTION_GENERATOR:
        raise ValueError(f"refusing to replace foreign projection: {path}")


def sync_harnesses(*, root=ROOT, write=False):
    root = Path(root)
    with tempfile.TemporaryDirectory(prefix=".agentframe-sync-", dir=root) as tmp:
        temp_root = Path(tmp)
        built = _build_harness_projections(root, temp_root / "build")
        if not write:
            issues = []
            for _harness, (expected_root, actual_root) in built.items():
                for skill in json.loads((root / HARNESS_MANIFEST).read_text(encoding="utf-8-sig"))["skills"]:
                    issues.extend(_projection_drift(expected_root / skill, actual_root / skill))
                expected_manifest = expected_root / PROJECTION_MANIFEST
                actual_manifest = actual_root / PROJECTION_MANIFEST
                if not actual_manifest.is_file():
                    issues.append(f"missing projection manifest: {actual_manifest}")
                elif expected_manifest.read_bytes() != actual_manifest.read_bytes():
                    issues.append(f"drifted projection manifest: {actual_manifest}")
            return issues

        operations = []
        backup_root = temp_root / "backup"
        for harness, (expected_root, actual_root) in built.items():
            actual_root.mkdir(parents=True, exist_ok=True)
            for skill_dir in sorted(p for p in expected_root.iterdir() if p.is_dir()):
                destination = actual_root / skill_dir.name
                _validate_projection_destination(destination)
                operations.append((skill_dir, destination, backup_root / harness / skill_dir.name))
            manifest_source = expected_root / PROJECTION_MANIFEST
            manifest_destination = actual_root / PROJECTION_MANIFEST
            if manifest_destination.exists():
                try:
                    current = json.loads(manifest_destination.read_text(encoding="utf-8"))
                except (OSError, json.JSONDecodeError) as exc:
                    raise ValueError(
                        f"refusing to replace foreign manifest: {manifest_destination}"
                    ) from exc
                if current.get("generated_by") != PROJECTION_GENERATOR:
                    raise ValueError(f"refusing to replace foreign manifest: {manifest_destination}")
            operations.append((manifest_source, manifest_destination,
                               backup_root / harness / PROJECTION_MANIFEST))

        installed = []
        backups = []
        try:
            for source, destination, backup in operations:
                if destination.exists():
                    backup.parent.mkdir(parents=True, exist_ok=True)
                    os.replace(destination, backup)
                    backups.append((backup, destination))
                os.replace(source, destination)
                installed.append(destination)
        except Exception:
            for destination in reversed(installed):
                _remove_projection_path(destination)
            for backup, destination in reversed(backups):
                os.replace(backup, destination)
            raise
    return []


def cmd_sync_harnesses(args):
    try:
        issues = sync_harnesses(write=args.write)
    except ValueError as exc:
        die(str(exc))
    if issues:
        for issue in issues:
            print(f"af sync-harnesses: DRIFT: {issue}")
        raise SystemExit(1)
    action = "wrote" if args.write else "checked"
    print(f"af sync-harnesses: {action} native skill projections; clean")


def check_system():
    return dead_link_issues() + ppt_master_stray_issues(), budget_notes() + voice_mirror_notes()


def cmd_doctor(args):
    dirs, pipeline_scope = [], ""
    if args.project == "pipeline":
        pass  # pipeline checks only
    elif args.project:
        dirs = [project_dir(args.project)]
    else:
        if os.path.isdir(PROJECTS):
            dirs = [os.path.join(PROJECTS, d) for d in sorted(os.listdir(PROJECTS))
                    if d != "completed" and os.path.isfile(os.path.join(PROJECTS, d, "project.md"))]
    all_issues, notes = [], []
    for d in dirs:
        all_issues += check_project(d)
        note = dream_note(d)
        if note:
            notes.append(note)
        notes += media_manifest_notes(d)
        notes += activity_notes(d)
    if args.project in (None, "pipeline"):
        pipe_issues, pipe_notes = check_pipeline()
        all_issues += pipe_issues
        notes += pipe_notes
        if os.path.isfile(board_path()):
            pipeline_scope = " + pipeline"
    system_scope = ""
    if not args.project:
        sys_issues, sys_notes = check_system()
        all_issues += sys_issues
        notes += sys_notes
        system_scope = " + system surfaces"
    system_scope += pipeline_scope
    for n in notes:
        print(f"af doctor: note — {n}")
    if all_issues:
        print(f"af doctor: {len(all_issues)} issue(s) — surfaced, never auto-fixed (operator decides):")
        for i in all_issues:
            print(f"  - {i}")
        sys.exit(1)
    print(f"af doctor: {len(dirs)} project(s){system_scope} checked, books clean")


# ---------------------------------------------------------------- main

# Verbs that mutate project state are Operator actions. `doctor` is read-only
# and allowed in any mode; so is `pipe board`.
OPERATOR_VERBS = {"ready", "publish", "version", "draft", "adopt", "new-project"}
OPERATOR_PIPE_VERBS = {"save", "start", "stage"}
OPERATOR_AUTONOMY_VERBS = {"init", "start", "checkpoint", "finish"}
OPERATOR_AUTOMATION_VERBS = {"init", "ready", "activate", "pause", "retire"}


def check_mode_gate(cmd, args=None):
    """Enforce the unattended-run charter for state-changing commands.

    Interactive task ownership is routed by the stable root AGENTS.md and its
    task-local routers, not by mutable repository mode state.
    """
    if os.environ.get("AGENTFRAME_MANAGED_RUN") == "1":
        if cmd in {"ready", "publish", "automation", "sync-harnesses"}:
            die(f"'af {cmd}' is outside the managed-run charter; report blocked and exit")
        return
    return


def main():
    for stream in (sys.stdout, sys.stderr):
        try:
            stream.reconfigure(encoding="utf-8")
        except (AttributeError, ValueError):
            pass
    p = argparse.ArgumentParser(prog="af", description="AgentFrame state-transition CLI")
    sub = p.add_subparsers(dest="cmd", required=True)

    s = sub.add_parser("ready");           s.add_argument("project"); s.add_argument("deliverable")
    s.add_argument("--allow-missing-exports", action="store_true"); s.set_defaults(fn=cmd_ready)
    s = sub.add_parser("publish");         s.add_argument("project"); s.add_argument("deliverable")
    s.add_argument("--url"); s.add_argument("--posted-at"); s.add_argument("--platform")
    s.add_argument("--media", nargs="*", default=[]); s.set_defaults(fn=cmd_publish)
    s = sub.add_parser("version");         s.add_argument("project"); s.add_argument("deliverable")
    s.add_argument("--artifact"); s.set_defaults(fn=cmd_version)
    s = sub.add_parser("draft");           s.add_argument("project"); s.add_argument("deliverable")
    target = s.add_mutually_exclusive_group(required=True)
    target.add_argument("--artifact"); target.add_argument("--file")
    s.set_defaults(fn=cmd_draft)
    s = sub.add_parser("adopt");           s.add_argument("project"); s.add_argument("deliverable")
    s.add_argument("--file", required=True); s.add_argument("--workstream")
    s.add_argument("--export"); s.add_argument("--notes"); s.set_defaults(fn=cmd_adopt)
    s = sub.add_parser("new-project");     s.add_argument("slug")
    s.add_argument("--flow", default=DEFAULT_FLOW, choices=sorted(FLOWS)); s.add_argument("--domain", default=DEFAULT_DOMAIN)
    s.add_argument("--name"); s.set_defaults(fn=cmd_new_project)
    s = sub.add_parser("doctor");          s.add_argument("project", nargs="?"); s.set_defaults(fn=cmd_doctor)
    s = sub.add_parser("sync-harnesses")
    action = s.add_mutually_exclusive_group(required=True)
    action.add_argument("--check", action="store_true")
    action.add_argument("--write", action="store_true")
    s.set_defaults(fn=cmd_sync_harnesses)

    s = sub.add_parser("automation")
    msub = s.add_subparsers(dest="automation_cmd", required=True)
    ma = msub.add_parser("init"); ma.add_argument("project"); ma.add_argument("automation_id")
    ma.add_argument("--job", required=True); ma.set_defaults(fn=cmd_automation_init)
    ma = msub.add_parser("ready"); ma.add_argument("project"); ma.add_argument("automation_id")
    ma.set_defaults(fn=cmd_automation_ready)
    ma = msub.add_parser("activate"); ma.add_argument("project"); ma.add_argument("automation_id")
    ma.add_argument("--deployment"); ma.set_defaults(fn=cmd_automation_activate)
    ma = msub.add_parser("pause"); ma.add_argument("project"); ma.add_argument("automation_id")
    ma.set_defaults(fn=cmd_automation_pause)
    ma = msub.add_parser("retire"); ma.add_argument("project"); ma.add_argument("automation_id")
    ma.set_defaults(fn=cmd_automation_retire)

    s = sub.add_parser("autonomy")
    asub = s.add_subparsers(dest="autonomy_cmd", required=True)
    aa = asub.add_parser("init"); aa.add_argument("project"); aa.add_argument("run_id")
    aa.add_argument("--level", choices=sorted(AUTONOMY_LEVELS), default="assisted"); aa.set_defaults(fn=cmd_autonomy_init)
    aa = asub.add_parser("check"); aa.add_argument("project"); aa.add_argument("run_id"); aa.set_defaults(fn=cmd_autonomy_check)
    aa = asub.add_parser("start"); aa.add_argument("project"); aa.add_argument("run_id")
    aa.add_argument("--resume-reason"); aa.set_defaults(fn=cmd_autonomy_start)
    aa = asub.add_parser("checkpoint"); aa.add_argument("project"); aa.add_argument("run_id")
    aa.add_argument("--outcome", choices=("continue", "blocked", "review"), required=True)
    aa.add_argument("--summary", required=True); aa.add_argument("--evidence")
    aa.add_argument("--subagents-spawned", type=int, required=True); aa.set_defaults(fn=cmd_autonomy_checkpoint)
    aa = asub.add_parser("finish"); aa.add_argument("project"); aa.add_argument("run_id")
    aa.add_argument("--approved-by", choices=("operator", "reviewer"), required=True); aa.set_defaults(fn=cmd_autonomy_finish)

    s = sub.add_parser("pipe")
    psub = s.add_subparsers(dest="pipe_cmd", required=True)
    ps = psub.add_parser("save")
    ps.add_argument("--company", required=True); ps.add_argument("--role", required=True)
    ps.add_argument("--url", required=True); ps.add_argument("--ats", default="unknown")
    ps.add_argument("--source", default="manual"); ps.add_argument("--posted")
    ps.add_argument("--deadline"); ps.add_argument("--salary"); ps.add_argument("--slug")
    ps.set_defaults(fn=cmd_pipe_save)
    ps = psub.add_parser("start");  ps.add_argument("slug"); ps.set_defaults(fn=cmd_pipe_start)
    ps = psub.add_parser("stage");  ps.add_argument("slug"); ps.add_argument("stage"); ps.set_defaults(fn=cmd_pipe_stage)
    ps = psub.add_parser("board");  ps.set_defaults(fn=cmd_pipe_board)

    args = p.parse_args()
    check_mode_gate(args.cmd, args)
    args.fn(args)


if __name__ == "__main__":
    main()
