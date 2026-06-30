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
  python system/af.py lock <project> <deliverable-slug-or-path>
  python system/af.py publish <project> <target> --url URL [--posted-at ISO] [--platform P] [--media PATH ...]
  python system/af.py version <project> <deliverable-slug>
  python system/af.py new-project <slug> [--domain marketing] [--flow open-flow] [--name NAME]
  python system/af.py doctor [project]
"""

import argparse
import datetime
import glob
import importlib.util
import os
import re
import shutil
import sys
import types

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROJECTS = os.path.join(ROOT, "workspace", "projects")
DOMAINS = os.path.join(ROOT, "library", "domains")

STATUS_ENUM = {"not_started", "drafting", "locked", "delivered", "deferred"}
LIFECYCLE_ENUM = {"active", "complete", "cancelled"}
FLOWS = {"marketing-solo-flow": "1-research-and-architecture",
         "marketing-standard-flow": "1-research",
         "open-flow": "active",
         "project-mgmt-open-flow": "active"}


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
    """Find a project by folder name, else by its `slug` frontmatter field."""
    for base in (PROJECTS, os.path.join(PROJECTS, "completed")):
        d = os.path.join(base, arg)
        if os.path.isfile(os.path.join(d, "project.md")):
            return d
    for base in (PROJECTS, os.path.join(PROJECTS, "completed")):
        if not os.path.isdir(base):
            continue
        for name in sorted(os.listdir(base)):
            sp = os.path.join(base, name, "project.md")
            if os.path.isfile(sp) and get_scalar(split_fm(read(sp), sp)[0], "slug") == arg:
                return os.path.join(base, name)
    die(f"project '{arg}' not found under workspace/projects/")


def split_fm(text, path="file"):
    m = re.match(r"\A---\r?\n(.*?)\r?\n---\r?\n?", text, re.S)
    if not m:
        die(f"{path} has no frontmatter block")
    return m.group(1), text[m.end():]


def join_fm(fm, body):
    return f"---\n{fm}\n---\n{body}"


def set_scalar(fm, key, value, path="frontmatter"):
    """Replace `key: ...` at any indent; error if absent (no silent schema invention)."""
    pat = re.compile(rf"^(\s*{re.escape(key)}:)[ \t]*.*$", re.M)
    if not pat.search(fm):
        die(f"{path}: field '{key}' not found — fix the file or the schema first")
    return pat.sub(rf"\g<1> {value}", fm, count=1)


def clean_value(v):
    return re.sub(r"\s+#.*$", "", v).strip().strip('"')


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


def row_span(fm, slug):
    """Span of a 2-space-indented tracker row `  {slug}:` and its 4-space fields."""
    m = re.search(rf"^  {re.escape(slug)}:\s*$", fm, re.M)
    if not m:
        return None
    start = m.start()
    rest = fm[m.end():]
    nxt = re.search(r"^(  \S|\S)", rest, re.M)
    end = m.end() + (nxt.start() if nxt else len(rest))
    return start, end


def row_set(fm, slug, key, value):
    span = row_span(fm, slug)
    if not span:
        die(f"project.md: tracker row '{slug}' not found in deliverables block")
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


def all_rows(fm):
    m = re.search(r"^deliverables:\s*$", fm, re.M)
    if not m:
        return []
    rest = fm[m.end():]
    nxt = re.search(r"^\S", rest, re.M)
    block = rest[: nxt.start() if nxt else len(rest)]
    return re.findall(r"^  ([A-Za-z0-9_-]+):\s*$", block, re.M)


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
        set_scalar=set_scalar, get_scalar=get_scalar, row_set=row_set, row_get=row_get,
        row_span=row_span, all_rows=all_rows, versions_in=versions_in, today=today,
        now_iso=now_iso, append_activity=append_activity, touch_lifecycle=touch_lifecycle, die=die)


# ---------------------------------------------------------------- lock

def cmd_lock(args):
    cdir = project_dir(args.project)
    cfm, cbody = split_fm(read(os.path.join(cdir, "project.md")), "project.md")

    slug, rel = args.deliverable, None
    if "/" in slug.replace("\\", "/"):
        rel, slug = slug.replace("\\", "/"), None
    else:
        rel = row_get(cfm, slug, "file") or die(f"row '{slug}' has no file pointer")

    dpath = os.path.join(cdir, rel)
    os.path.isfile(dpath) or die(f"deliverable file not found: {rel}")
    head_of(dpath)

    dfm, dbody = split_fm(read(dpath), rel)
    dfm = set_scalar(dfm, "status", "locked", rel)
    dfm = set_scalar(dfm, "last_updated", today(), rel)
    write(dpath, join_fm(dfm, dbody))

    notes = []
    rules = load_rules(load_pack(project_domain(cfm))[1])
    if rules and hasattr(rules, "on_lock"):
        cfm, notes = rules.on_lock(make_ctx(), cdir, dpath, rel, cfm)

    if slug:
        cfm = row_set(cfm, slug, "status", "locked")
        cfm = row_set(cfm, slug, "last_updated", today())
    cfm = touch_lifecycle(cfm)
    write(os.path.join(cdir, "project.md"), join_fm(cfm, cbody))
    append_activity(cdir, f"lock: {slug or os.path.basename(rel)} locked; artifact={rel}"
                    + (f"; {'; '.join(notes)}" if notes else ""))

    print(f"af lock: {rel} -> locked" + (f" ({'; '.join(notes)})" if notes else ""))
    print("\nJudgment checklist (agent + operator):")
    print("  [ ] Template lock criteria verified (the deliverable's template)")
    print("  [ ] Humanizer pass run, when the template declares it (public-facing prose)")
    print("  [ ] Voice was loaded for this deliverable's drafting (confirm if session resumed)")
    print("  [ ] Voice mini-retro eligibility checked (library/process/voice-mini-retro.md)")
    print("  [ ] Remaining follow-ups surfaced (review, export, deliver)")


# ---------------------------------------------------------------- publish

def cmd_publish(args):
    cdir = project_dir(args.project)
    cfm, _ = split_fm(read(os.path.join(cdir, "project.md")), "project.md")
    domain = project_domain(cfm)
    desc, pack_dir = load_pack(domain)
    if not desc or "publish" not in fm_list(desc, "verbs"):
        die(f"publish is not a verb the '{domain}' domain declares — see library/domains/{domain}/pack.md")
    rules = load_rules(pack_dir)
    if not rules or not hasattr(rules, "publish"):
        die(f"the '{domain}' domain declares publish but ships no rules.publish")
    rules.publish(make_ctx(), cdir, args)


# ---------------------------------------------------------------- version

def cmd_version(args):
    cdir = project_dir(args.project)
    cpath = os.path.join(cdir, "project.md")
    cfm, cbody = split_fm(read(cpath), "project.md")
    rel = row_get(cfm, args.deliverable, "file") or die(f"tracker row '{args.deliverable}' not found or has no file")
    dpath = os.path.join(cdir, rel)
    os.path.isfile(dpath) or die(f"deliverable file not found: {rel}")
    name, n = head_of(dpath) or die(f"{rel} is not a versioned -v{{N}}.md file")

    new_rel = os.path.join(os.path.dirname(rel), f"{name}-v{n + 1}.md").replace("\\", "/")
    new_path = os.path.join(cdir, new_rel)
    shutil.copyfile(dpath, new_path)
    dfm, dbody = split_fm(read(new_path), new_rel)
    dfm = set_scalar(dfm, "status", "drafting", new_rel)
    dfm = set_scalar(dfm, "last_updated", today(), new_rel)
    write(new_path, join_fm(dfm, dbody))

    cfm = row_set(cfm, args.deliverable, "file", new_rel)
    cfm = row_set(cfm, args.deliverable, "status", "drafting")
    cfm = row_set(cfm, args.deliverable, "last_updated", today())
    cfm = touch_lifecycle(cfm)
    write(cpath, join_fm(cfm, cbody))

    print(f"af version: {rel} -> {new_rel} (head; prior version untouched as the snapshot)")
    print("\nJudgment (stays with the agent):")
    print("  - Use this for REPLACEMENT-shaped changes (deliverable-versioning.md). Surgical")
    print("    edits (typos, frontmatter, small wording) go directly into the current head.")
    print("  - If the operator feedback criticized SHAPE or process, append one feedback-log.md line this turn.")


# ---------------------------------------------------------------- new-project

def cmd_new_project(args):
    slug = args.slug
    re.match(r"^[a-z0-9][a-z0-9-]*$", slug) or die("slug must be folder-safe lowercase kebab-case")
    desc, pack_dir = load_pack(args.domain)
    if not desc:
        die(f"no domain pack at library/domains/{args.domain}/ (pack.md missing) — author the pack first")
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


# ---------------------------------------------------------------- doctor

def check_project(cdir):
    issues = []
    rel = os.path.relpath(cdir, ROOT).replace("\\", "/")
    try:
        cfm, _ = split_fm(read(os.path.join(cdir, "project.md")), "project.md")
    except SystemExit:
        return [f"{rel}: project.md missing or has no frontmatter"]

    for field in ("name", "slug", "schema_version", "created_at", "domain", "status", "current_phase", "flow", "last_activity"):
        if get_scalar(cfm, field) in (None, ""):
            issues.append(f"{rel}: required field '{field}' missing")
    if get_scalar(cfm, "slug") not in (None, os.path.basename(cdir)):
        issues.append(f"{rel}: slug '{get_scalar(cfm, 'slug')}' != folder name")
    if get_scalar(cfm, "status") not in LIFECYCLE_ENUM:
        issues.append(f"{rel}: lifecycle status '{get_scalar(cfm, 'status')}' not in {sorted(LIFECYCLE_ENUM)}")

    # Domain pack: core checks always; the pack's frontmatter extension + rules apply for its domain.
    domain = get_scalar(cfm, "domain")
    desc, pack_dir = load_pack(domain)
    if domain and not desc:
        issues.append(f"{rel}: domain '{domain}' has no pack at library/domains/{domain}/")
    if desc:
        for f in fm_list(desc, "extension_fields"):
            if not has_field(cfm, f):
                issues.append(f"{rel}: domain '{domain}' requires field '{f}' (missing)")

    for slug in all_rows(cfm):
        st, f = row_get(cfm, slug, "status"), row_get(cfm, slug, "file")
        if st not in STATUS_ENUM:
            issues.append(f"{rel}: row '{slug}' status '{st}' invalid")
        if not f:
            issues.append(f"{rel}: row '{slug}' has no file pointer")
            continue
        p = os.path.join(cdir, f)
        if st == "not_started":
            pass
        elif not os.path.isfile(p):
            issues.append(f"{rel}: row '{slug}' file missing: {f}")
        else:
            m = re.fullmatch(r"(.+)-v(\d+)\.md", os.path.basename(p))
            if m:
                highest = max(versions_in(os.path.dirname(p), m.group(1)))
                if int(m.group(2)) != highest:
                    issues.append(f"{rel}: row '{slug}' points at v{m.group(2)} but head is v{highest}")

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
    return issues


def cmd_doctor(args):
    dirs = []
    if args.project:
        dirs = [project_dir(args.project)]
    else:
        for base in (PROJECTS, os.path.join(PROJECTS, "completed")):
            if os.path.isdir(base):
                dirs += [os.path.join(base, d) for d in sorted(os.listdir(base))
                         if os.path.isfile(os.path.join(base, d, "project.md"))]
    all_issues = []
    for d in dirs:
        all_issues += check_project(d)
    if all_issues:
        print(f"af doctor: {len(all_issues)} issue(s) — surfaced, never auto-fixed (operator decides):")
        for i in all_issues:
            print(f"  - {i}")
        sys.exit(1)
    print(f"af doctor: {len(dirs)} project(s) checked, books clean")


# ---------------------------------------------------------------- main

def main():
    p = argparse.ArgumentParser(prog="af", description="AgentFrame state-transition CLI")
    sub = p.add_subparsers(dest="cmd", required=True)

    s = sub.add_parser("lock");            s.add_argument("project"); s.add_argument("deliverable"); s.set_defaults(fn=cmd_lock)
    s = sub.add_parser("publish");         s.add_argument("project"); s.add_argument("post")
    s.add_argument("--url", required=True); s.add_argument("--posted-at"); s.add_argument("--platform")
    s.add_argument("--media", nargs="*", default=[]); s.set_defaults(fn=cmd_publish)
    s = sub.add_parser("version");         s.add_argument("project"); s.add_argument("deliverable"); s.set_defaults(fn=cmd_version)
    s = sub.add_parser("new-project");     s.add_argument("slug")
    s.add_argument("--flow", default="open-flow", choices=sorted(FLOWS)); s.add_argument("--domain", default="marketing")
    s.add_argument("--name"); s.set_defaults(fn=cmd_new_project)
    s = sub.add_parser("doctor");          s.add_argument("project", nargs="?"); s.set_defaults(fn=cmd_doctor)

    args = p.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
