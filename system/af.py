#!/usr/bin/env python3
"""AgentFrame state-transition CLI — the buttons.

Owns the MECHANICS of campaign state changes. Binds to the
campaign-frontmatter schema only — never to flows, templates, or content.
Each command performs its bookkeeping atomically, writes the campaign
paper trail (activity.md) as a side effect, and prints back the JUDGMENT
checklist the agent must still run. Stdlib only.

Commands:
  python system/af.py lock <campaign> <deliverable-slug-or-path>
  python system/af.py publish <campaign> <post-slug> --url URL [--posted-at ISO] [--platform P] [--media PATH ...]
  python system/af.py version <campaign> <deliverable-slug>
  python system/af.py new-campaign <slug> --flow {solo-flow,standard-flow,open-flow} [--name NAME]
  python system/af.py doctor [campaign]
"""

import argparse
import datetime
import glob
import os
import re
import shutil
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CAMPAIGNS = os.path.join(ROOT, "workspace", "campaigns")

STATUS_ENUM = {"not_started", "drafting", "locked", "shipped", "deferred"}
LIFECYCLE_ENUM = {"active", "complete", "cancelled"}
FLOWS = {"solo-flow": "1-research-and-architecture",
         "standard-flow": "1-research",
         "open-flow": "active"}


def die(msg):
    print(f"af: ERROR: {msg}", file=sys.stderr)
    sys.exit(1)


def today():
    return datetime.date.today().isoformat()


def stamp():
    return datetime.datetime.now().strftime("%Y-%m-%d %H:%M")


def read(path):
    with open(path, "r", encoding="utf-8-sig") as fh:  # -sig: tolerate BOMs from Windows editors
        return fh.read()


def write(path, text):
    with open(path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write(text)


def campaign_dir(slug):
    for base in (CAMPAIGNS, os.path.join(CAMPAIGNS, "completed")):
        d = os.path.join(base, slug)
        if os.path.isfile(os.path.join(d, "campaign.md")):
            return d
    die(f"campaign '{slug}' not found under workspace/campaigns/")


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


def get_scalar(fm, key):
    m = re.search(rf"^\s*{re.escape(key)}:[ \t]*(.*?)\s*$", fm, re.M)
    return m.group(1).strip('"') if m else None


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
        die(f"campaign.md: tracker row '{slug}' not found in deliverables block")
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
    return m.group(1).strip('"') if m else None


def all_rows(fm):
    m = re.search(r"^deliverables:\s*$", fm, re.M)
    if not m:
        return []
    rest = fm[m.end():]
    nxt = re.search(r"^\S", rest, re.M)
    block = rest[: nxt.start() if nxt else len(rest)]
    return re.findall(r"^  ([A-Za-z0-9_-]+):\s*$", block, re.M)


def head_of(path):
    """Verify a versioned file is the highest v{N} in its folder; return (name, N)."""
    m = re.match(r"(.+)-v(\d+)\.md$", os.path.basename(path))
    if not m:
        return None
    name, n = m.group(1), int(m.group(2))
    pattern = os.path.join(os.path.dirname(path), f"{name}-v*.md")
    highest = max(int(re.search(r"-v(\d+)\.md$", p).group(1)) for p in glob.glob(pattern))
    if n != highest:
        die(f"{os.path.basename(path)} is not the head ({name}-v{highest}.md exists) — point at the head or run doctor")
    return name, n


def touch_lifecycle(fm):
    return set_scalar(fm, "last_activity", datetime.datetime.now().astimezone().isoformat(timespec="seconds"), "campaign.md")


def append_activity(cdir, line):
    path = os.path.join(cdir, "activity.md")
    text = read(path) if os.path.isfile(path) else ""
    if text and not text.endswith("\n"):
        text += "\n"
    write(path, text + f"{stamp()} — {line}\n")


def manifest_ingredients(fm):
    m = re.search(r"^\s*ingredients:\s*\[(.*?)\]\s*$", fm, re.M)
    if not m:
        return []
    return [i.strip() for i in m.group(1).split(",") if i.strip()]


# ---------------------------------------------------------------- lock

POST_FINAL_SKELETON = """---
status: drafting
last_updated: {date}
---

# {title} — FINAL

Assembled from the post's locked ingredients per the campaign manifest.
"""


def assemble_post_final(post_dir, locked_path, ingredient):
    pf = os.path.join(post_dir, "post-FINAL.md")
    title = os.path.basename(post_dir)
    if not os.path.isfile(pf):
        write(pf, POST_FINAL_SKELETON.format(date=today(), title=title))
    fm, body = split_fm(read(pf), "post-FINAL.md")
    _, src_body = split_fm(read(locked_path), locked_path)
    fname = os.path.basename(locked_path)
    heading = f"## {ingredient.replace('-', ' ').title()} (locked from {fname})"
    section = f"{heading}\n{src_body.strip()}\n"
    old = re.search(rf"^## .*\(locked from {re.escape(ingredient)}-v\d+\.md\)\n.*?(?=^## |\Z)", body, re.M | re.S)
    if old:
        body = body[:old.start()] + section + "\n" + body[old.end():]
    else:
        body = body.rstrip("\n") + "\n\n" + section
    fm = set_scalar(fm, "last_updated", today(), "post-FINAL.md")
    write(pf, join_fm(fm, body))
    return pf


def post_complete(post_dir, ingredients):
    for ing in ingredients:
        heads = glob.glob(os.path.join(post_dir, f"{ing}-v*.md"))
        if not heads:
            return False
        best = max(heads, key=lambda p: int(re.search(r"-v(\d+)\.md$", p).group(1)))
        if get_scalar(split_fm(read(best), best)[0], "status") != "locked":
            return False
    return True


def cmd_lock(args):
    cdir = campaign_dir(args.campaign)
    cfm, cbody = split_fm(read(os.path.join(cdir, "campaign.md")), "campaign.md")

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
    norm = rel.replace("\\", "/")
    if re.search(r"(^|/)posts/", norm):
        post_dir = os.path.dirname(dpath)
        ing = re.match(r"(.+)-v\d+\.md$", os.path.basename(dpath)).group(1)
        assemble_post_final(post_dir, dpath, ing)
        notes.append("post-FINAL.md updated")
        ings = manifest_ingredients(cfm)
        if ings and post_complete(post_dir, ings):
            pf = os.path.join(post_dir, "post-FINAL.md")
            pfm, pbody = split_fm(read(pf), "post-FINAL.md")
            pfm = set_scalar(pfm, "status", "locked", "post-FINAL.md")
            pfm = set_scalar(pfm, "last_updated", today(), "post-FINAL.md")
            write(pf, join_fm(pfm, pbody))
            post_slug = os.path.basename(post_dir)
            if row_span(cfm, post_slug):
                cfm = row_set(cfm, post_slug, "status", "locked")
                cfm = row_set(cfm, post_slug, "last_updated", today())
            notes.append("all manifest ingredients locked — post-FINAL locked")

    if slug:
        cfm = row_set(cfm, slug, "status", "locked")
        cfm = row_set(cfm, slug, "last_updated", today())
    cfm = touch_lifecycle(cfm)
    write(os.path.join(cdir, "campaign.md"), join_fm(cfm, cbody))
    append_activity(cdir, f"lock: {slug or os.path.basename(rel)} locked; artifact={rel}"
                    + (f"; {'; '.join(notes)}" if notes else ""))

    print(f"af lock: {rel} -> locked" + (f" ({'; '.join(notes)})" if notes else ""))
    print("\nJudgment checklist (agent + operator):")
    print("  [ ] Template lock criteria verified (library/deliverables/{type}/template.md)")
    print("  [ ] Humanizer pass run, when the template declares it (public-facing prose)")
    print("  [ ] Voice was loaded for this deliverable's drafting (confirm if session resumed)")
    print("  [ ] Voice mini-retro eligibility checked (library/process/voice-mini-retro.md)")
    print("  [ ] Remaining follow-ups surfaced (review, export, publish)")


# ---------------------------------------------------------------- publish

def cmd_publish(args):
    cdir = campaign_dir(args.campaign)
    cfm, cbody = split_fm(read(os.path.join(cdir, "campaign.md")), "campaign.md")
    rel = row_get(cfm, args.post, "file") or die(f"tracker row '{args.post}' not found or has no file")
    if not rel.endswith("post-FINAL.md"):
        die(f"row '{args.post}' points at {rel}, not a post-FINAL.md — publish operates on the assembly record")
    pf = os.path.join(cdir, rel)
    pfm, pbody = split_fm(read(pf), rel)

    posted = args.posted_at or datetime.datetime.now().astimezone().isoformat(timespec="seconds")
    pfm = set_scalar(pfm, "status", "shipped", rel)
    pfm = set_scalar(pfm, "last_updated", today(), rel)
    pfm = re.sub(r"\n(shipped_at:.*|published:(\n  .*)*|shipped_media:(\n  - .*)*)", "", pfm)
    block = [f"shipped_at: {today()}", "published:", f"  platform: {args.platform}",
             f"  url: {args.url}", f"  posted_at: {posted}"]
    if args.media:
        block.append("shipped_media:")
        block += [f"  - {m}" for m in args.media]
    pfm = pfm.rstrip("\n") + "\n" + "\n".join(block)
    write(pf, join_fm(pfm, pbody))

    cfm = row_set(cfm, args.post, "status", "shipped")
    cfm = row_set(cfm, args.post, "last_updated", today())
    shipped = sum(1 for s in all_rows(cfm)
                  if re.match(r"post-\d+$", s) and row_get(cfm, s, "status") == "shipped")
    cfm = set_scalar(cfm, "posts_published", str(shipped), "campaign.md")
    if get_scalar(cfm, "shipped_at") in (None, "null", ""):
        cfm = set_scalar(cfm, "shipped_at", today(), "campaign.md")
    cfm = touch_lifecycle(cfm)
    write(os.path.join(cdir, "campaign.md"), join_fm(cfm, cbody))
    append_activity(cdir, f"post_published: {args.post} → {args.url}")

    print(f"af publish: {args.post} -> shipped ({args.url}); posts_published={shipped}")
    print("\nJudgment checklist (agent + operator):")
    print("  [ ] Shipped copy reconciled: if the live post differs materially from the locked")
    print("      ingredient, write the next ingredient version with the as-shipped text, re-lock,")
    print("      refresh its post-FINAL section — and run the voice publish/back-fill fallback")
    print("  [ ] shipped_media recorded for every asset that actually shipped (--media)")
    print("  [ ] Performance capture scheduled (~14 days after posted_at, per composio-notes.md)")


# ---------------------------------------------------------------- version

def cmd_version(args):
    cdir = campaign_dir(args.campaign)
    cpath = os.path.join(cdir, "campaign.md")
    cfm, cbody = split_fm(read(cpath), "campaign.md")
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


# ---------------------------------------------------------------- new-campaign

CAMPAIGN_SKELETON = """---
# IDENTITY
name: "{name}"
slug: {slug}
schema_version: 2026-04-23
created_at: {date}
supersedes: null

# LIFECYCLE
status: active
current_phase: {phase}
campaign_flow: {flow}
last_activity: {ts}
shipped_at: null
completed_at: null
cancelled_at: null
cancelled_reason: null
quarterly_goals_advanced: []

# MANIFEST
post_manifest:
  ingredients: []

# DELIVERABLES
deliverables: {{}}

# COUNTERS
post_count: 0
posts_published: 0
system_retro_completed: null
campaign_retro_completed: null
---

# {name}

## Thesis

(one paragraph once the direction locks)
"""


def cmd_new_campaign(args):
    slug = args.slug
    re.match(r"^[a-z0-9][a-z0-9-]*$", slug) or die("slug must be folder-safe lowercase kebab-case")
    cdir = os.path.join(CAMPAIGNS, slug)
    if os.path.exists(cdir):
        die(f"{cdir} already exists")
    os.makedirs(cdir)
    name = args.name or slug.replace("-", " ").title()
    write(os.path.join(cdir, "campaign.md"), CAMPAIGN_SKELETON.format(
        name=name, slug=slug, date=today(), phase=FLOWS[args.flow], flow=args.flow,
        ts=datetime.datetime.now().astimezone().isoformat(timespec="seconds")))
    write(os.path.join(cdir, "feedback-log.md"), "")
    append_activity(cdir, f"campaign_started: {name} scaffolded ({args.flow})")

    print(f"af new-campaign: workspace/campaigns/{slug}/ scaffolded ({args.flow}, phase {FLOWS[args.flow]})")
    print("\nJudgment (stays with the agent):")
    print(f"  - Load library/process/campaign-flows/{args.flow}.md and run its kickoff")
    print("    (research offer / plan proposal / manifest moment — flow-owned, not script-owned).")


# ---------------------------------------------------------------- doctor

def check_campaign(cdir):
    issues = []
    rel = os.path.relpath(cdir, ROOT).replace("\\", "/")
    try:
        cfm, _ = split_fm(read(os.path.join(cdir, "campaign.md")), "campaign.md")
    except SystemExit:
        return [f"{rel}: campaign.md missing or has no frontmatter"]

    for field in ("name", "slug", "schema_version", "created_at", "status", "current_phase", "campaign_flow", "last_activity"):
        if get_scalar(cfm, field) in (None, ""):
            issues.append(f"{rel}: required field '{field}' missing")
    if get_scalar(cfm, "slug") not in (None, os.path.basename(cdir)):
        issues.append(f"{rel}: slug '{get_scalar(cfm, 'slug')}' != folder name")
    if get_scalar(cfm, "status") not in LIFECYCLE_ENUM:
        issues.append(f"{rel}: lifecycle status '{get_scalar(cfm, 'status')}' not in {sorted(LIFECYCLE_ENUM)}")

    shipped_posts = 0
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
            m = re.match(r"(.+)-v(\d+)\.md$", os.path.basename(p))
            if m:
                pattern = os.path.join(os.path.dirname(p), f"{m.group(1)}-v*.md")
                highest = max(int(re.search(r"-v(\d+)\.md$", q).group(1)) for q in glob.glob(pattern))
                if int(m.group(2)) != highest:
                    issues.append(f"{rel}: row '{slug}' points at v{m.group(2)} but head is v{highest}")
        if re.match(r"post-\d+$", slug) and st == "shipped":
            shipped_posts += 1
    declared = get_scalar(cfm, "posts_published")
    if declared is not None and declared.isdigit() and int(declared) != shipped_posts:
        issues.append(f"{rel}: posts_published={declared} but {shipped_posts} post rows are shipped")
    return issues


def cmd_doctor(args):
    dirs = []
    if args.campaign:
        dirs = [campaign_dir(args.campaign)]
    else:
        for base in (CAMPAIGNS, os.path.join(CAMPAIGNS, "completed")):
            if os.path.isdir(base):
                dirs += [os.path.join(base, d) for d in sorted(os.listdir(base))
                         if os.path.isfile(os.path.join(base, d, "campaign.md"))]
    all_issues = []
    for d in dirs:
        all_issues += check_campaign(d)
    if all_issues:
        print(f"af doctor: {len(all_issues)} issue(s) — surfaced, never auto-fixed (operator decides):")
        for i in all_issues:
            print(f"  - {i}")
        sys.exit(1)
    print(f"af doctor: {len(dirs)} campaign(s) checked, books clean")


# ---------------------------------------------------------------- main

def main():
    p = argparse.ArgumentParser(prog="af", description="AgentFrame state-transition CLI")
    sub = p.add_subparsers(dest="cmd", required=True)

    s = sub.add_parser("lock");            s.add_argument("campaign"); s.add_argument("deliverable"); s.set_defaults(fn=cmd_lock)
    s = sub.add_parser("publish");         s.add_argument("campaign"); s.add_argument("post")
    s.add_argument("--url", required=True); s.add_argument("--posted-at"); s.add_argument("--platform", default="linkedin")
    s.add_argument("--media", nargs="*", default=[]); s.set_defaults(fn=cmd_publish)
    s = sub.add_parser("version");         s.add_argument("campaign"); s.add_argument("deliverable"); s.set_defaults(fn=cmd_version)
    s = sub.add_parser("new-campaign");    s.add_argument("slug")
    s.add_argument("--flow", required=True, choices=sorted(FLOWS)); s.add_argument("--name"); s.set_defaults(fn=cmd_new_campaign)
    s = sub.add_parser("doctor");          s.add_argument("campaign", nargs="?"); s.set_defaults(fn=cmd_doctor)

    args = p.parse_args()
    args.fn(args)


if __name__ == "__main__":
    main()
