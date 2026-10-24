"""Parity harness: python system/af.py <verb> against target/release/af <verb>.

Each case builds one fixture root, copies it (copy2, so mtimes survive), runs the same
steps through the Python on one copy and the Rust binary on the other, then compares:

  - exit code and stdout of every step (root paths masked),
  - the `af: ERROR:` lines on stderr (the refusal text),
  - the file tree after the last step: path list and bytes of every file,
  - the run log, line for line, with only `ts` masked.

Masked, because they are nondeterministic: clock readings the run itself writes
(`now_iso()`, `stamp()`, the run-log `ts`) — any timestamp not already present in the
fixture — and the two temp root paths. Nothing else is masked.

Usage:  python system/af/tests/parity.py [-k substring] [--keep]
Prints one line per case, `PASS name` or `DIFF name: <first differing path>`; exits 1 on
any DIFF. Build first: cargo build --release (in system/af/).
"""
import argparse
import datetime
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
CRATE = os.path.dirname(HERE)
REPO = os.path.dirname(os.path.dirname(CRATE))
BIN = os.path.join(CRATE, "target", "release", "af.exe" if os.name == "nt" else "af")
TODAY = datetime.date.today()


def ago(days):
    return (TODAY - datetime.timedelta(days=days)).isoformat()


def ahead(days):
    return (TODAY + datetime.timedelta(days=days)).isoformat()


def utc_ago(days):
    t = datetime.datetime.now(datetime.timezone.utc) - datetime.timedelta(days=days)
    return t.replace(microsecond=0).isoformat()


# ------------------------------------------------------------------ fixture helpers

def w(root, rel, text):
    path = os.path.join(root, *rel.split("/"))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    return path


def wb(root, rel, data):
    path = os.path.join(root, *rel.split("/"))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)
    return path


def mkdir(root, rel):
    os.makedirs(os.path.join(root, *rel.split("/")), exist_ok=True)


def base(root):
    """What every fixture root carries: the Python under test, its media helper, the packs."""
    shutil.copy2(os.path.join(REPO, "system", "af.py"), _mk(root, "system/af.py"))
    shutil.copy2(os.path.join(REPO, "system", "tools", "media_intake.py"), _mk(root, "system/tools/media_intake.py"))
    shutil.copytree(os.path.join(REPO, "library", "domains"), os.path.join(root, "library", "domains"),
                    ignore=shutil.ignore_patterns("__pycache__"))
    w(root, "AGENTS.md", "# Fixture root\n")


def _mk(root, rel):
    path = os.path.join(root, *rel.split("/"))
    os.makedirs(os.path.dirname(path), exist_ok=True)
    return path


def project(root, slug, *, domain="project-mgmt", status="active", rows="", extra="", name=None,
            schema="2026-07-19-v2", body="\n# Project\n", folder=None):
    fm = (f"# IDENTITY\nname: {name or slug}\nslug: {slug}\nschema_version: {schema}\n"
          f"created_at: 2026-07-19\ndomain: {domain}\n\n# LIFECYCLE\nstatus: {status}\n"
          f"current_phase: active\nlast_activity: 2026-07-19T10:00:00-07:00\n{extra}")
    fm += "deliverables:\n" + rows if rows else "deliverables: {}\n"
    return w(root, f"workspace/projects/{folder or slug}/project.md", f"---\n{fm}---\n{body}")


def row(slug, status, file, **extra):
    s = f"  {slug}:\n    status: {status}\n    file: {file}\n    last_updated: 2026-07-19\n"
    for k, v in extra.items():
        s += f"    {k}: {v}\n"
    return s


def head(root, rel, status="drafting", body="\nbody\n", extra=""):
    return w(root, rel, f"---\nstatus: {status}\nlast_updated: 2026-07-19\n{extra}---\n{body}")


# ------------------------------------------------------------------ cases

def case_doctor_empty(root):
    return [["doctor"], ["doctor", "pipeline"], ["doctor", "studio"]]


def case_doctor_projects(root):
    p = "workspace/projects"
    # clean project-mgmt project
    project(root, "good", rows=row("brief", "drafting", "brief/brief-v1.md"))
    head(root, f"{p}/good/brief/brief-v1.md")
    w(root, f"{p}/good/activity.md", "# Activity\n\n2026-07-19 10:00 — project_started: good\n- [ ] attention bullet\n")
    # broken frontmatter fields and lifecycle
    w(root, f"{p}/bad-fields/project.md",
      "---\nname: \nslug: other-slug\nschema_version: 2026-01-01\ndomain: project-mgmt\nstatus: paused\n"
      "completed_at: 2026-07-01\ncancelled_at: null\nlast_activity: 2026-07-19T10:00:00-07:00\n---\n")
    project(root, "done-early", status="complete",
            rows=row("a", "drafting", "a/a-v1.md") + row("b", "not_started", "b/b-v1.md"))
    head(root, f"{p}/done-early/a/a-v1.md")
    project(root, "cancelled", status="cancelled")
    project(root, "nopack", domain="no-such-pack")
    project(root, "slugged", folder="folder-name")
    # rows of every shape, marketing domain
    rows = (row("bad-status", "shipping", "x/x-v1.md")
            + "  no-file:\n    status: drafting\n    last_updated: 2026-07-19\n"
            + row("nullfile", "drafting", "null")
            + row("unversioned", "drafting", "notes/notes.md")
            + row("missing", "drafting", "gone/gone-v1.md")
            + row("planned", "not_started", "later/later-v1.md")
            + row("mismatch", "ready", "m/m-v1.md")
            + row("stale-head", "drafting", "s/s-v1.md")
            + row("nofm", "drafting", "n/n-v1.md")
            + row("binary", "drafting", "deck/session-deck.pptx")
            + row("undecodable", "drafting", "u/u-v1.md")
            + row("empty", "drafting", "e/e-v1.md")
            + row("essay", "drafting", "essay/essay-v1.md", substack_url='"https://example.com/p/x"', mastodon_url="null",
                  substack_draft="210153401")
            + row("carousel", "ready", "post-1-carousel/image-prompts-v1.md")
            + row("post-1", "published", "posts/post-1/post-FINAL.md")
            + row("post-2", "published", "posts/post-2/post-FINAL.md", ingredients="[body-copy]")
            + row("post-3", "drafting", "posts/post-3/post-FINAL.md"))
    project(root, "rows", domain="marketing", rows=rows,
            extra="channels: [linkedin, substack]\nstakeholders: [jane-doe]\npost_manifest:\n  ingredients: [body-copy, image-prompts]\n")
    r = f"{p}/rows"
    head(root, f"{r}/x/x-v1.md")
    head(root, f"{r}/notes/notes.md")
    head(root, f"{r}/m/m-v1.md", status="drafting")
    head(root, f"{r}/s/s-v1.md")
    head(root, f"{r}/s/s-v2.md")
    w(root, f"{r}/n/n-v1.md", "# no frontmatter\n")
    wb(root, f"{r}/deck/session-deck.pptx", b"PK\x03\x04\xfe\xff\x00\x01binary")
    wb(root, f"{r}/u/u-v1.md", b"---\nstatus: drafting\n---\n\xff\xfe bad bytes\n")
    head(root, f"{r}/e/e-v1.md", body="\n  \n")
    head(root, f"{r}/essay/essay-v1.md")
    head(root, f"{r}/post-1-carousel/image-prompts-v1.md", status="ready")
    head(root, f"{r}/posts/post-1/post-FINAL.md", status="published", extra="shipped_media:\n  - media/missing.png\n  - https://cdn.example.com/x.png\n  - ../../../outside.png\n")
    head(root, f"{r}/posts/post-2/post-FINAL.md", status="published")
    w(root, f"{r}/posts/post-2/visuals/draft.png", "png")
    w(root, f"{r}/posts/post-2/visuals/history/old.png", "png")
    head(root, f"{r}/posts/post-3/post-FINAL.md")
    w(root, f"{r}/knowledge/people/jane-doe.md", "overlay\n")
    w(root, f"{r}/knowledge/_archive/deliverables-archive.md",
      "---\ndeliverables:\n  post-0:\n    status: published\n    file: posts/post-0/image-prompts-v1.md\n    last_updated: 2026-05-01\n---\n")
    head(root, f"{r}/posts/post-0/image-prompts-v1.md", status="published", extra="exports:\n  - media/final.png\n")
    w(root, "workspace/life/channels/linkedin/profile.md", "linkedin\n")
    # long, ragged activity log on an active project
    lines = ["# Activity", ""]
    lines += [f"2026-07-01 — note: event {i}" for i in range(198)]
    lines += ["2026-07-02 10:00 - decision: " + "x" * 220, "freeform narration with no shape", "  ",
              "2026-07-03 — Bad: capitalised event"]
    project(root, "chatty")
    w(root, f"{p}/chatty/activity.md", "\n".join(lines) + "\n")
    # completed/ and folders without a project.md are skipped
    project(root, "archived-one", folder="completed")
    mkdir(root, f"{p}/not-a-project")
    # a custom pack with extension fields and its own exportables
    w(root, "library/domains/custom/pack.md",
      "---\nname: custom\nextension_fields: [budget, sponsor]\nexportable: [deck]\n---\n\n# Custom pack\n")
    project(root, "ext", domain="custom", extra="budget: 100\n",
            rows=row("deck", "ready", "deck/deck-v1.md"))
    head(root, f"{p}/ext/deck/deck-v1.md", status="ready")
    return [["doctor"], ["doctor", "rows"], ["doctor", "ext"], ["doctor", "nope"], ["doctor", "folder-name"],
            ["doctor", "slugged"]]


def case_doctor_system(root):
    w(root, "AGENTS.md",
      "# Root\n\nSee [ops](AGENTS.operator.md), [gone](missing.md), [anchor](#top), [web](https://x.com),\n"
      "[life](workspace/life/voice/README.md), [backlog](system/backlog.md#bb), [tpl]({slug}.md),\n"
      "`[code](code-span.md)` and [rooted](library/process/README.md) and [frag](missing2.md#x).\n\n"
      "```\n[fenced](fenced.md)\n```\n")
    w(root, "AGENTS.operator.md", "# Operator\n" + "word " * 1001 + "\n")
    w(root, "AGENTS.builder.md", "# Builder\n[up](../outside.md)\n")
    w(root, "library/process/README.md", "# Process\n[sib](sibling.md) [deep](sub/deeper.md)\n")
    w(root, "library/process/sub/deeper.md", "# Deep\n" + "w " * 1600 + "\n[root](../../../AGENTS.md)\n")
    w(root, "library/deliverables/memo/template.md", "---\nkind: memo\n---\n" + "t " * 950 + "\n")
    w(root, "library/assets/README.md", "# Assets\n[lang](design-languages/demo/README.md)\n")
    w(root, "system/vendor/README.md", "# Vendor\n[skill](demo/SKILL.md)\n")
    w(root, "system/vendor/demo/SKILL.md", "# Demo\n[ref](references/missing.md)\n")
    w(root, ".claude/skills/README.md", "# Skills\n")
    w(root, ".claude/skills/thing/SKILL.md", "# Thing\n[x](../../../library/process/README.md) [y](nope.md)\n")
    # voice mirror: method surface + personal depth; mtimes fixed so the comparison is stable
    live = "workspace/life/voice"
    schema = "library/schema/operator/voice"
    for rel, t in ((f"{live}/README.md", 2000), (f"{live}/method.md", 2000), (f"{live}/unmirrored.md", 2000),
                   (f"{live}/registers/README.md", 1000), (f"{live}/registers/formal.md", 3000),
                   (f"{live}/exemplars/README.md", 1000),
                   (f"{schema}/README.md", 1000), (f"{schema}/method.md", 3000),
                   (f"{schema}/registers/README.md", 2000), (f"{schema}/registers/formal.md", 1000)):
        path = w(root, rel, f"# {rel}\n")
        os.utime(path, (1_700_000_000 + t, 1_700_000_000 + t))
    # design languages
    dl = "library/assets/design-languages"
    w(root, f"{dl}/clean/README.md", "notes")
    w(root, f"{dl}/clean/package/templates/design_spec.md", "kind: deck\n")
    w(root, f"{dl}/clean/package/templates/cover.svg", "<svg/>")
    w(root, f"{dl}/clean/imagery/orb.png", "x")
    w(root, f"{dl}/clean/imagery/manifest.yaml",
      "assets:\n  - path: orb.png\n    source: derived\n    licence: pexels\n    restriction: none\n")
    w(root, f"{dl}/messy/package/templates/notes.txt", "x")
    mkdir(root, f"{dl}/messy/package/exports")
    w(root, f"{dl}/messy/imagery/manifest.yaml",
      "assets:\n  - path: \"ghost.png\"\n    licence: unknown\n    restriction: none\n  - path: 'b.png'\n    licence: unknown\n    restriction: project-scoped\n")
    w(root, f"{dl}/messy/imagery/b.png", "x")
    w(root, f"{dl}/waypoint/README.md", "Just a look we like.")
    w(root, f"{dl}/declared/README.md", "Reference-grade until promoted. Tracked as BB-2026-08-12-04.")
    w(root, f"{dl}/declared/imagery/manifest.yaml", "assets: []\n")
    w(root, f"{dl}/stray-file.txt", "not a language")
    # media shelf
    shelf = "library/assets/media"
    w(root, f"{shelf}/media.yaml", "root: Z:/no/such/drive/media\n")
    w(root, f"{shelf}/cards/aaaaaaaaaaaa.md",
      "---\nsha256: aaaaaaaaaaaaffff\npath: clips/a.mov\nkind: video\ncodec: hevc\nreview: pending\n"
      f"ingested_at: '{ago(30)}T10:00:00-07:00'\npeople: others\nused_in: [post-1]\n---\n# a\n")
    w(root, f"{shelf}/cards/bbbbbbbbbbbb.md",
      "---\nsha256: bbbbbbbbbbbbffff\npath: Z:/abs/b.jpg\nkind: photo\nreview: done\npeople: others\nused_in: [x]\nconsent: signed\n---\n")
    w(root, f"{shelf}/cards/cccccccccccc.md", "no frontmatter here\n")
    w(root, f"{shelf}/cards/dddddddddddd.md", "---\npath: nosha.png\n---\n")
    w(root, f"{shelf}/cards/notes.txt", "ignored\n")
    # ppt-master strays
    mkdir(root, "system/vendor/ppt-master/projects/demo/exports")
    wb(root, "system/vendor/ppt-master/templates/decks/deck_20260709.PPTX", b"")
    wb(root, "system/vendor/ppt-master/a.pptx", b"")
    mkdir(root, "system/projects/demo")
    # run log + backlog for the buttons note
    rows = []
    for verb, code, refusal, crash, days in (("draft", 1, "already has a version chain", None, 1),
                                             ("draft", 1, "repo-root-relative", None, 2),
                                             ("adopt", 1, "status must be drafting", None, 3),
                                             ("draft", 2, None, "SyntaxError: mid-edit", 1),
                                             ("version", 2, "usage", None, 1),
                                             ("pipe board", 1, None, None, 1),
                                             ("doctor", 1, None, None, 1),
                                             ("ready", 1, "old", None, 20),
                                             ("publish", 0, None, None, 1)):
        import json
        r = {"ts": utc_ago(days), "verb": verb, "argv": verb.split(), "exit": code}
        if refusal:
            r["refusal"] = refusal
        if crash:
            r["crash"] = crash
        rows.append(json.dumps(r))
    rows.append("not json")
    rows.append('{"ts": "2026-01-01T00:00:00", "verb": "draft", "exit": 1}')
    w(root, "system/logs/af-runs.jsonl", "\n".join(rows) + "\n")
    w(root, "system/backlog.md",
      "# Backlog\n\nprose mentioning af.py that is not a row\n\n"
      "## BB-2026-09-01-01 — prose only\nnothing about buttons here\nStatus: open\n\n"
      "## BB-2026-09-01-02 — `af ready` refuses a shape\nthe button dies on it\nStatus: open\n\n"
      "## BB-2026-09-01-04 — `af draft` crashed once\nfixed in the parser\nStatus: done\n\n"
      "```yaml\n- id: BB-2026-09-01-03\n  one_line: af.py draft path bug\n```\n")
    return [["doctor"]]


def case_doctor_system_quiet(root):
    """Media shelf that parses clean, root mounted; voice tree present without schema mirror."""
    shelf = "library/assets/media"
    w(root, f"{shelf}/media.yaml", "root: .\n")
    w(root, f"{shelf}/cards/eeeeeeeeeeee.md",
      "---\nsha256: eeeeeeeeeeeeffff\npath: e.png\nkind: photo\nreview: pending\n"
      f"ingested_at: '{TODAY.isoformat()}T23:59:59+14:00'\n---\n")
    w(root, f"{shelf}/e.png", "x")
    w(root, "workspace/life/voice/README.md", "# voice\n")
    return [["doctor"]]


def case_media_shelf_broken(root):
    w(root, "library/assets/media/cards/ffffffffffff.md", "---\n- just\n- a list\n---\n")
    return [["doctor"]]


def pipeline_board(root, rows_text, extra=""):
    w(root, "workspace/pipeline/pipeline.md",
      f"---\nschema_version: 1\ncreated_at: 2026-10-01\nlast_activity: 2026-10-01T10:00:00-04:00\n{extra}"
      f"applications:\n{rows_text}\n---\n\n# Job Pipeline\n")


def app(root, slug, *, folder=None, materials="[resume]", rows=None, fields=None, jd=True):
    rows = rows if rows is not None else (
        "  resume:\n    file: resume/resume-v1.md\n    status: not_started\n"
        "  cover-letter:\n    file: cover-letter/cover-letter-v1.md\n    status: not_started\n")
    f = {"name": f'"{slug} app"', "slug": slug, "schema_version": "1", "created_at": "2026-10-01",
         "domain": "careers", "company": '"Acme"', "role": '"AI PM"', "job_url": "https://example.com/j/1",
         "source": "manual", "ats": "greenhouse"}
    f.update(fields or {})
    body = "".join(f"{k}: {v}\n" for k, v in f.items() if v is not None)
    d = f"workspace/pipeline/applications/{folder or slug}"
    w(root, f"{d}/application.md",
      f"---\n# IDENTITY\n{body}\n# LIFECYCLE\nlast_activity: 2026-10-01T10:00:00-04:00\n\n"
      f"materials: {materials}\n\n# DELIVERABLES\ndeliverables:\n{rows}---\n\n# App\n")
    if jd:
        w(root, f"{d}/jd.md", "verbatim JD\n")
    return d


def case_doctor_pipeline(root):
    board = ("  clean-one:\n    stage: preparing\n    company: \"Acme\"\n    saved: 2026-10-01\n"
             f"  nudge-due:\n    stage: applied\n    saved: 2026-09-01\n    next_nudge: {ago(2)}\n"
             f"  nudge-later:\n    stage: interviewing\n    next_nudge: {ahead(5)}\n"
             f"  stale-save:\n    stage: saved\n    saved: {ago(40)}\n"
             f"  fresh-save:\n    stage: saved\n    saved: {ago(3)}\n"
             "  no-folder:\n    stage: interviewing\n"
             "  dropped-one:\n    stage: dropped\n"
             "  bad-stage:\n    stage: wishlist\n"
             "  archived-one:\n    stage: rejected\n"
             "  linty:\n    stage: preparing\n"
             "  broken:\n    stage: applied\n"
             "  rounds:\n    stage: interviewing\n")
    pipeline_board(root, board)
    app(root, "clean-one")
    app(root, "nudge-due")
    app(root, "nudge-later")
    app(root, "archived-one", folder="completed/archived-one")
    app(root, "orphan")
    app(root, "old-orphan", folder="completed/old-orphan")
    d = app(root, "linty", materials="[resume, deck]",
            rows="  resume:\n    file: resume/resume-v1.md\n    status: drafting\n"
                 "  cover-letter:\n    file: cover-letter/cover-letter-v2.md\n    status: ready\n")
    w(root, f"{d}/resume/resume-v1.md",
      "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: []\n---\n"
      "## My Journey\n- spearheaded X — delivered Y • and – more\n- Acme 2022 - 2023, Beta 2019-2021\n"
      "## Skills\n- delve into synergies\n")
    w(root, f"{d}/cover-letter/cover-letter-v1.md", "---\nstatus: ready\n---\nold\n")
    w(root, f"{d}/cover-letter/cover-letter-v2.md",
      "---\nstatus: ready\nlast_updated: 2026-07-10\nexports: [media/cl.pdf]\n---\nDear team → hi ✓\n")
    app(root, "broken", fields={"company": None, "slug": "wrong-slug", "job_url": '""'}, jd=False,
        rows="  resume:\n    file: resume/resume-v1.md\n    status: weird\n  demo:\n    status: drafting\n")
    d = app(root, "rounds")
    w(root, f"{d}/round-1-jeff/README.md", f"---\nheld_at: {ago(10)}T10:00:00-07:00\n---\n\n# Round 1\n")
    w(root, f"{d}/round-2-panel/README.md", f"---\nheld_at: {ago(5)}\n---\n")
    w(root, f"{d}/round-2-panel/debrief.md",
      "---\nstatus: drafting\ncompleteness: Partial - operator has not reported\npromoted: false\n---\n\n# D\n")
    w(root, f"{d}/round-3-final/README.md", "---\nheld_at: 2099-01-05T10:00:00-07:00\n---\n")
    w(root, f"{d}/round-4-ceo/README.md", "no frontmatter\n")
    w(root, f"{d}/round-4-ceo/debrief.md", "---\ncompleteness: full\npromoted: TRUE\n---\n")
    w(root, f"{d}/round-5-x/README.md", "---\nheld_at: \n---\n")
    w(root, f"{d}/round-5-x/debrief.md", "plain notes, no frontmatter\n")
    w(root, f"{d}/round-6-y/debrief.md", "---\ncompleteness: full\n---\n")
    w(root, f"{d}/Round-7-Bad/README.md", "---\nheld_at: 2026-01-01\n---\n")
    return [["doctor", "pipeline"], ["doctor"], ["doctor", "linty"]]


def case_doctor_pipeline_clean(root):
    pipeline_board(root, "  clean-one:\n    stage: preparing\n    saved: 2026-10-01\n")
    app(root, "clean-one")
    return [["doctor", "pipeline"], ["doctor"]]


def calendar(root, rows_text):
    w(root, "workspace/studio/calendar.md",
      f"---\nschema_version: 1\ncreated_at: 2026-10-01\nlast_activity: 2026-10-01T10:00:00-04:00\nslots:\n{rows_text}\n---\n\n# Studio Calendar\n")


def post(root, slug, *, fields=None, rows=None):
    f = {"name": f'"{slug.title()}"', "slug": slug, "schema_version": "1", "created_at": "2026-10-01",
         "domain": "creator", "type": "video", "platform": "tiktok", "posted_at": "null", "url": "null",
         "metrics_captured_at": "null"}
    f.update(fields or {})
    body = "".join(f"{k}: {v}\n" for k, v in f.items() if v is not None)
    d = f"workspace/studio/posts/{slug}"
    w(root, f"{d}/post.md", f"---\n# IDENTITY\n{body}\n# LIFECYCLE\nlast_activity: 2026-10-01T10:00:00-04:00\n\n"
                            f"deliverables:{(chr(10) + rows) if rows else ' {}' + chr(10)}---\n\n# Post\n")
    return d


def case_doctor_studio(root):
    cal = ("  captured-one:\n    state: captured\n    platform: tiktok\n"
           "  planned-future:\n    state: planned\n    date: 2099-01-01\n"
           f"  planned-past:\n    state: planned\n    date: {ago(3)}\n"
           f"  sched-past:\n    state: scheduled\n    date: {ago(2)}\n"
           "  cut-no-render:\n    state: cut\n"
           "  posted-old:\n    state: posted\n"
           "  posted-captured:\n    state: posted\n"
           "  posted-fresh:\n    state: posted\n"
           "  dropped-gone:\n    state: dropped\n"
           "  planned-gone:\n    state: planned\n"
           "  weird:\n    state: editing\n"
           "  broken:\n    state: captured\n"
           "  scripted:\n    state: planned\n")
    calendar(root, cal)
    post(root, "captured-one")
    post(root, "planned-future")
    post(root, "planned-past")
    d = post(root, "sched-past")
    w(root, f"{d}/renders/final.mp4", "mp4")
    post(root, "cut-no-render")
    post(root, "posted-old", fields={"posted_at": f"{ago(30)}T09:00:00-07:00", "url": '"https://t.example/1"'})
    post(root, "posted-captured", fields={"posted_at": f"{ago(30)}T09:00:00-07:00", "metrics_captured_at": ago(1)})
    post(root, "posted-fresh", fields={"posted_at": f"{ago(3)}T09:00:00-07:00"})
    post(root, "orphan-post")
    post(root, "broken", fields={"platform": None, "slug": "nope", "name": '""'})
    d = post(root, "scripted", rows="  script:\n    status: drafting\n    file: script/script-v1.md\n")
    head(root, f"{d}/script/script-v1.md")
    return [["doctor", "studio"], ["doctor"], ["doctor", "scripted"]]


def case_new_project(root):
    return [["new-project", "alpha"],
            ["new-project", "beta", "--domain", "marketing", "--name", "Beta Launch"],
            ["new-project", "alpha"],
            ["new-project", "Bad_Slug"],
            ["new-project", "gamma", "--domain", "careers"],
            ["new-project", "delta", "--domain", "creator"],
            ["new-project", "eps", "--domain", "no-such-pack"],
            ["new-project", "zeta", "--dom", "marketing", "--name", "Zeta: a \"quoted\" name"],
            ["doctor"]]


def case_lifecycle(root):
    return [["new-project", "lifecycle-smoke", "--domain", "project-mgmt"],
            ["draft", "lifecycle-smoke", "brief", "--file", "brief/brief-v1.md"],
            ["version", "lifecycle-smoke", "brief"],
            ["ready", "lifecycle-smoke", "brief"],
            ["publish", "lifecycle-smoke", "brief", "--url", "https://example.com/brief"],
            ["doctor", "lifecycle-smoke"],
            ["doctor"]]


def case_draft(root):
    project(root, "campaign", domain="marketing",
            rows=row("brief", "not_started", "brief/placeholder.md")
            + row("spine", "drafting", "spine/spine-v1.md")
            + row("post-8", "not_started", "phase-3-production/posts/post-8/post-FINAL.md")
            + row("post-9", "drafting", "posts/post-9/post-FINAL.md"),
            extra="post_manifest:\n  ingredients: [body-copy, image-prompts]\n")
    head(root, "workspace/projects/campaign/spine/spine-v1.md")
    head(root, "workspace/projects/campaign/posts/post-9/post-FINAL.md")
    head(root, "workspace/projects/campaign/posts/post-9/body-copy-v1.md")
    head(root, "workspace/projects/campaign/loose/loose-v1.md")
    project(root, "plain", rows="")
    return [["draft", "campaign", "brief", "--file", "brief/brief.md"],
            ["draft", "campaign", "brief", "--file", "workspace/projects/campaign/brief/brief-v1.md"],
            ["draft", "campaign", "brief", "--file", "brief/_seed.md"],
            ["draft", "campaign", "brief", "--file", "brief/brief-v1.md"],
            ["draft", "campaign", "brief", "--file", "brief/brief-v1.md"],
            ["draft", "campaign", "spine", "--file", "spine/spine-v1.md"],
            ["draft", "campaign", "loose", "--file", "loose/loose-v1.md"],
            ["draft", "campaign", "post-8", "--artifact", "body-copy"],
            ["draft", "campaign", "post-9", "--artifact", "body-copy"],
            ["draft", "campaign", "post-9", "--artifact", "image-prompts"],
            ["draft", "campaign", "post-10", "--artifact", "body-copy"],
            ["draft", "campaign", "x", "--artifact", "a", "--file", "b-v1.md"],
            ["draft", "plain", "notes", "--file", "notes/notes-v1.md"],
            ["draft", "plain", "Bad Slug", "--file", "b/b-v1.md"],
            ["draft", "plain", "deck", "--file", "deck/deck-v1.pptx"],
            ["draft", "plain", "other", "--file", "../escape/escape-v1.md"],
            ["draft", "ghost", "x", "--file", "x/x-v1.md"],
            ["doctor"]]


def case_version(root):
    project(root, "vp", domain="marketing",
            rows=row("brief", "ready", "brief/brief-v1.md")
            + row("post-8", "ready", "posts/post-8/post-FINAL.md")
            + row("old", "drafting", "old/old-v1.md")
            + row("deck", "drafting", "deck/d.pptx")
            + row("ghost", "drafting", "ghost/ghost-v1.md"))
    v = "workspace/projects/vp"
    head(root, f"{v}/brief/brief-v1.md", status="ready", body="\nsource snapshot\n", extra="published_url: x\nchanges_from_v0: y\n")
    head(root, f"{v}/posts/post-8/post-FINAL.md", status="ready")
    head(root, f"{v}/posts/post-8/body-copy-v1.md", status="ready")
    head(root, f"{v}/posts/post-8/body-copy-notes-v1.md")
    head(root, f"{v}/old/old-v1.md")
    head(root, f"{v}/old/old-v2.md")
    wb(root, f"{v}/deck/d.pptx", b"PK\x03\x04")
    return [["version", "vp", "brief"],
            ["version", "vp", "brief"],
            ["version", "vp", "post-8", "--artifact", "body-copy"],
            ["version", "vp", "post-8", "--artifact", "nothing"],
            ["version", "vp", "post-8"],
            ["version", "vp", "old"],
            ["version", "vp", "deck"],
            ["version", "vp", "ghost"],
            ["version", "vp", "nosuchrow"],
            ["version", "vp", "brief/brief-v3.md"],
            ["doctor", "vp"]]


def case_adopt(root):
    project(root, "ad", rows="")
    project(root, "busy", rows=row("brief", "drafting", "brief/current-v1.md"))
    a = "workspace/projects/ad"
    head(root, f"{a}/brief/brief-v1.md")
    w(root, f"{a}/spine/spine-v1.md", "---\nstatus: drafting\n---\n# Spine\n\nbody\n")
    w(root, f"{a}/raw/raw.md", "---\nstatus: drafting\nlast_updated: 2026-01-01\n---\nraw\n")
    head(root, f"{a}/ready/ready-v1.md", status="ready")
    w(root, f"{a}/nofm/nofm-v1.md", "plain\n")
    head(root, "workspace/projects/busy/brief/current-v1.md", body="\nkeep\n")
    head(root, "workspace/projects/busy/brief/replacement-v1.md", body="\nreplacement\n")
    return [["adopt", "ad", "brief", "--file", "brief/brief-v1.md", "--notes", "existing renderer output"],
            ["adopt", "ad", "spine", "--file", "spine/spine-v1.md", "--workstream", "core", "--export", "media/x.pdf"],
            ["adopt", "ad", "raw", "--file", "raw/raw.md"],
            ["adopt", "ad", "ready", "--file", "ready/ready-v1.md"],
            ["adopt", "ad", "nofm", "--file", "nofm/nofm-v1.md"],
            ["adopt", "ad", "missing", "--file", "missing/missing-v1.md"],
            ["adopt", "ad", "brief", "--file", "brief/brief-v1.md"],
            ["adopt", "ad", "rr", "--file", "workspace/projects/ad/brief/brief-v1.md"],
            ["adopt", "busy", "brief", "--file", "brief/replacement-v1.md"],
            ["version", "ad", "spine"],
            ["doctor"]]


def case_ready(root):
    c = "workspace/projects/carousel"
    project(root, "carousel", domain="marketing",
            rows=row("post-1-carousel", "drafting", "post-1-carousel/image-prompts-v1.md")
            + row("post-2-carousel", "drafting", "post-2-carousel/image-prompts-v1.md")
            + row("post-3-carousel", "drafting", "post-3-carousel/image-prompts-v1.md")
            + row("post-1-body", "drafting", "post-1-body/body-copy-v1.md")
            + row("stale", "drafting", "stale/stale-v1.md")
            + row("deck", "drafting", "deck/deck.pptx")
            + row("done", "published", "done/done-v1.md"))
    head(root, f"{c}/post-1-carousel/image-prompts-v1.md")
    head(root, f"{c}/post-2-carousel/image-prompts-v1.md", extra="exports:\n  - media/missing.png\n")
    head(root, f"{c}/post-3-carousel/image-prompts-v1.md", extra="exports:\n  - media/final.pdf\n")
    w(root, f"{c}/post-3-carousel/media/final.pdf", "x")
    head(root, f"{c}/post-1-body/body-copy-v1.md")
    head(root, f"{c}/stale/stale-v1.md")
    head(root, f"{c}/stale/stale-v2.md")
    wb(root, f"{c}/deck/deck.pptx", b"PK")
    head(root, f"{c}/done/done-v1.md", status="published")
    # marketing assembly
    p = "workspace/projects/campaign"
    project(root, "campaign", domain="marketing",
            rows=row("post-1", "drafting", "posts/post-1/post-FINAL.md")
            + row("post-1-5", "drafting", "posts/post-1.5-vitamix/post-FINAL.md"),
            extra="post_manifest:\n  ingredients: [body-copy, image-prompts]\n")
    head(root, f"{p}/posts/post-1/post-FINAL.md", body="\n# Post\n\n## Body Copy (ready from body-copy-v0.md)\nold text\n\n## Tail\nkept\n")
    head(root, f"{p}/posts/post-1/body-copy-v1.md", status="ready", body="\nBody\n")
    w(root, f"{p}/posts/post-1/media/final.png", "png")
    head(root, f"{p}/posts/post-1/image-prompts-v1.md", extra="exports:\n  - media/final.png\n", body="\nPrompts\n")
    head(root, f"{p}/posts/post-1.5-vitamix/post-FINAL.md", body="\n# Post\n")
    head(root, f"{p}/posts/post-1.5-vitamix/body-copy-v2.md", body="\nBody text.\n")
    return [["ready", "carousel", "post-1-carousel"],
            ["ready", "carousel", "post-2-carousel"],
            ["ready", "carousel", "post-1-carousel", "--allow-missing-exports"],
            ["ready", "carousel", "post-3-carousel"],
            ["ready", "carousel", "post-1-carousel/../post-1-body/body-copy-v1.md"],
            ["ready", "carousel", "post-1-body/body-copy-v1.md"],
            ["ready", "carousel", "stale/stale-v1.md"],
            ["ready", "carousel", "stale"],
            ["ready", "carousel", "deck", "--allow"],
            ["ready", "carousel", "done"],
            ["ready", "carousel", "nope"],
            ["ready", "campaign", "post-1"],
            ["ready", "campaign", "post-1-5"],
            ["ready", "campaign", "post-1-5", "--artifact", "body-copy"],
            ["ready", "campaign", "posts/post-1/image-prompts-v1.md"],
            ["ready", "campaign", "post-1", "--artifact", "nothing"],
            ["doctor"]]


def case_ready_careers(root):
    pipeline_board(root, "  acme:\n    stage: preparing\n    saved: 2026-10-01\n")
    d = app(root, "acme", materials="[resume, deck]",
            rows="  resume:\n    file: resume/resume-v1.md\n    status: drafting\n"
                 "  deck:\n    file: deck/deck-v1.md\n    status: drafting\n"
                 "  cover-letter:\n    file: cover-letter/cover-letter-v1.md\n    status: drafting\n")
    w(root, f"{d}/resume/resume-v1.md",
      "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: [media/resume-v1.pdf]\n---\n"
      "## Work Experience\n- Cut latency 38% (Jan 2022 - Dec 2023)\n## Skills\n- Python\n")
    w(root, f"{d}/resume/media/resume-v1.pdf", "pdf")
    w(root, f"{d}/deck/deck-v1.md",
      "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: [media/deck-v1.pptx]\n---\n# Deck — arc\n")
    w(root, f"{d}/deck/media/deck-v1.pptx", "pptx")
    w(root, f"{d}/cover-letter/cover-letter-v1.md",
      "---\nstatus: drafting\nlast_updated: 2026-07-10\nexports: [media/cl.pdf]\n---\nDear team — hello 2019-2021\n")
    w(root, f"{d}/cover-letter/media/cl.pdf", "pdf")
    w(root, f"{d}/jd-map.md", "---\nstatus: drafting\n---\n## Requirements\n- Python\n## Verification\n   \n## After\nx\n")

    def add_verification(r):
        p = os.path.join(r, *f"{d}/jd-map.md".split("/"))
        with open(p, encoding="utf-8") as f:
            t = f.read()
        with open(p, "w", encoding="utf-8", newline="\n") as f:
            f.write(t.replace("## Verification\n   \n", "## Verification\n- criteria mapped; deck reviewed\n"))

    return [["ready", "acme", "resume"],
            ["ready", "acme", "deck"],
            add_verification,
            ["ready", "acme", "resume"],
            ["ready", "acme", "deck"],
            ["ready", "acme", "cover-letter"],
            ["version", "acme", "resume"],
            ["doctor", "pipeline"]]


def case_publish(root):
    project(root, "generic", rows=row("brief", "ready", "brief/brief-v1.md")
            + row("memo", "ready", "memo/memo-v1.md")
            + row("draft", "drafting", "draft/draft-v1.md")
            + row("deck", "ready", "deck/deck.pptx"))
    g = "workspace/projects/generic"
    head(root, f"{g}/brief/brief-v1.md", status="ready")
    head(root, f"{g}/memo/memo-v1.md", status="ready")
    head(root, f"{g}/draft/draft-v1.md")
    wb(root, f"{g}/deck/deck.pptx", b"PK")
    project(root, "campaign", domain="marketing",
            rows=row("post-1", "ready", "posts/post-1/post-FINAL.md")
            + row("post-2", "ready", "posts/post-2/post-FINAL.md")
            + row("essay", "ready", "essay/substack-essay-v1.md"),
            extra="post_manifest:\n  ingredients: [body-copy]\n")
    m = "workspace/projects/campaign"
    head(root, f"{m}/posts/post-1/post-FINAL.md", status="ready")
    head(root, f"{m}/posts/post-2/post-FINAL.md", status="ready")
    w(root, f"{m}/posts/post-2/media/a.png", "png")
    head(root, f"{m}/essay/substack-essay-v1.md", status="ready")
    w(root, f"{m}/knowledge/_archive/deliverables-archive.md",
      "---\ndeliverables:\n  post-0:\n    status: published\n    file: posts/post-0/post-FINAL.md\n    last_updated: 2026-05-01\n---\n")
    return [["publish", "generic", "brief"],
            ["publish", "generic", "memo", "--url", "https://example.com/memo"],
            ["publish", "generic", "draft"],
            ["publish", "generic", "deck"],
            ["publish", "generic", "brief"],
            ["publish", "campaign", "post-1", "--url", "https://example.com/post-1"],
            ["publish", "campaign", "posts/post-2/post-FINAL.md", "--url", "https://example.com/p2",
             "--posted-at", "2026-07-27T18:40:00-07:00", "--platform", "linkedin", "--media", "media/a.png", "media/b.png"],
            ["publish", "campaign", "essay"],
            ["doctor"]]


def case_usage(root):
    project(root, "p", rows=row("d", "drafting", "d/d-v1.md"))
    head(root, "workspace/projects/p/d/d-v1.md")
    return [["draft", "p", "d"],
            ["lock", "p", "b"],
            ["ready", "p"],
            ["doctor", "a", "b"],
            ["publish", "p", "d", "--bogus"],
            ["ready", "p", "d", "--a"],
            []]


def case_crash(root):
    w(root, "workspace/projects/good/project.md", "---\nname: g\n---\n")
    wb(root, "workspace/projects/zz-broken/project.md", b"---\nname: \xff\xfe\n---\n")
    return [["version", "zz-broken", "brief"], ["doctor"]]


def case_bom_crlf(root):
    p = "workspace/projects/crlf"
    wb(root, f"{p}/project.md",
       ("\ufeff---\r\nname: crlf\r\nslug: crlf\r\nschema_version: 2026-07-19-v2\r\ncreated_at: 2026-07-19\r\n"
        "domain: project-mgmt\r\nstatus: active\r\ncurrent_phase: active\r\nlast_activity: 2026-07-19T10:00:00-07:00\r\n"
        "deliverables:\r\n  brief:\r\n    status: drafting   # inline comment\r\n    file: \"brief/brief-v1.md\"\r\n"
        "    last_updated: 2026-07-19\r\n---\r\n\r\n# CRLF\r\n").encode("utf-8"))
    wb(root, f"{p}/brief/brief-v1.md", "---\r\nstatus: drafting\r\nlast_updated: 2026-07-19\r\n---\r\nbody\r".encode("utf-8"))
    wb(root, f"{p}/activity.md", b"2026-07-19 10:00 \xe2\x80\x94 project_started: crlf")
    return [["doctor"], ["ready", "crlf", "brief"], ["version", "crlf", "brief"], ["doctor"]]


CASES = [
    ("doctor-empty", case_doctor_empty),
    ("doctor-projects", case_doctor_projects),
    ("doctor-system", case_doctor_system),
    ("doctor-system-quiet", case_doctor_system_quiet),
    ("doctor-media-shelf-broken", case_media_shelf_broken),
    ("doctor-pipeline", case_doctor_pipeline),
    ("doctor-pipeline-clean", case_doctor_pipeline_clean),
    ("doctor-studio", case_doctor_studio),
    ("new-project", case_new_project),
    ("lifecycle", case_lifecycle),
    ("draft", case_draft),
    ("version", case_version),
    ("adopt", case_adopt),
    ("ready", case_ready),
    ("ready-careers", case_ready_careers),
    ("publish", case_publish),
    ("usage", case_usage),
    ("crash", case_crash),
    ("bom-crlf", case_bom_crlf),
]


def media_case(cards, config=None):
    """One media-shelf fixture: each card is (name, text); a broken card fails the whole check."""
    def build(root):
        if config is not None:
            w(root, "library/assets/media/media.yaml", config)
        for name, text in cards:
            (wb if isinstance(text, bytes) else w)(root, f"library/assets/media/cards/{name}", text)
        return [["doctor"]]
    return build


CARD = "---\nsha256: {sha}\npath: {path}\nkind: {kind}\n{extra}---\n# card\n"
for _name, _cards, _config in (
    ("sha-list", [("a.md", CARD.format(sha="[ab, cd]", path="x.png", kind="photo", extra=""))], None),
    ("sha-int", [("a.md", CARD.format(sha="12345", path="x.png", kind="photo", extra=""))], None),
    ("sha-map", [("a.md", CARD.format(sha="{k: v}", path="x.png", kind="photo", extra=""))], None),
    ("path-int", [("a.md", CARD.format(sha="abc", path="5", kind="photo", extra=""))], None),
    ("codec-list", [("a.md", CARD.format(sha="abc", path="x.mov", kind="video", extra="codec: [hevc]\n"))], None),
    ("codec-h264", [("a.md", CARD.format(sha="abc", path="x.mov", kind="video", extra="codec: h264\n"))], None),
    ("codec-proxy", [("a.md", CARD.format(sha="abc", path="x.mov", kind="video", extra="codec: prores\nproxy: p.mp4\n"))], None),
    ("ingested-int", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra="review: pending\ningested_at: 5\n"))], None),
    ("ingested-missing", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra="review: pending\n"))], None),
    ("empty-docs", [("a.md", "---\n[]\n---\n"), ("b.md", "---\n\n---\n"), ("c.md", "---\n0\n---\n")], None),
    ("doc-str", [("a.md", "---\njust text\n---\n")], None),
    ("config-list", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra=""))], "- a\n- b\n"),
    ("config-empty-list", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra=""))], "[]\n"),
    ("config-int-root", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra=""))], "root: 5\n"),
    ("card-undecodable", [("a.md", b"---\nsha256: abc\npath: \xff\n---\n")], None),
    ("used-in-map", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra="people: others\nused_in: {post: 1}\n"))], None),
    # YAML 1.1 typing, as PyYAML resolves hand-edited scalars
    ("consent-no", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra="people: others\nused_in: [p]\nconsent: no\n"))], None),
    ("ingested-unquoted", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo",
                                                extra="review: pending\ningested_at: 2026-01-01T10:00:00-07:00\n"))], None),
    ("used-in-yes", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra="people: others\nused_in: yes\n"))], None),
    ("used-in-datetime", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo",
                                               extra="people: others\nused_in: 2026-01-01 10:00:00.5 +05:30\n"))], None),
    ("sha-1e3", [("a.md", CARD.format(sha="1e3", path="x.png", kind="photo", extra=""))], None),
    ("sha-sexagesimal", [("a.md", CARD.format(sha="1:30", path="x.png", kind="photo", extra=""))], None),
    ("sha-int-list", [("a.md", CARD.format(sha="[017, 0x1F, 1_000, 0b11, .5, 1.0e+3, ~, Off]", path="x.png", kind="photo", extra=""))], None),
    ("path-date", [("a.md", CARD.format(sha="abc", path="2026-01-01", kind="photo", extra=""))], None),
    ("proxy-off", [("a.md", CARD.format(sha="abc", path="x.mov", kind="video", extra="codec: hevc\nproxy: off\n"))], None),
    ("codec-float", [("a.md", CARD.format(sha="abc", path="x.mov", kind="video", extra="codec: 1.5e+3\n"))], None),
    ("config-root-date", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra=""))], "root: 2026-01-01\n"),
    # PyYAML-only syntax rules and constructor errors
    ("tab-in-card", [("a.md", CARD.format(sha="abc", path="x.png\t", kind="photo", extra=""))], None),
    ("block-scalar-tail", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra="people: others\nused_in: |\n  campaign\n"))], None),
    ("quoted-multiline", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo",
                                               extra="people: others\nused_in: \"two\nlines\\_here\"\n"))], None),
    ("unknown-tag", [("a.md", CARD.format(sha="abc", path="x.png", kind="!foo photo", extra=""))], None),
    ("merge-key", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo",
                                        extra="base: &b {consent: ok}\n<<: *b\npeople: others\nused_in: [p]\n"))], None),
    ("duplicate-anchor", [("a.md", CARD.format(sha="abc", path="x.png", kind="photo", extra="x: &a 1\ny: &a 2\n"))], None),
    ("first-card-wins", [("a.md", CARD.format(sha="1:30", path="x.png", kind="photo", extra="")),
                         ("b.md", CARD.format(sha="abc", path="x.png", kind="!bar photo", extra=""))], None),
):
    CASES.append((f"media-{_name}", media_case(_cards, _config)))


# ------------------------------------------------------------------ comparison

NOW_T = re.compile(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:[+-]\d{2}:\d{2}|Z)?")
NOW_STAMP = re.compile(r"\d{4}-\d{2}-\d{2} \d{2}:\d{2}")
TS_FIELD = re.compile(r'"ts": "[^"]*"')
SKIP_PARTS = {"__pycache__"}


def tree(root):
    out = {}
    for dp, dns, fns in os.walk(root):
        dns[:] = sorted(d for d in dns if d not in SKIP_PARTS)
        rel_dir = os.path.relpath(dp, root).replace("\\", "/")
        if rel_dir == ".":
            rel_dir = ""
        for d in dns:
            p = os.path.join(dp, d)
            if not os.listdir(p):
                out[(rel_dir + "/" + d).lstrip("/") + "/"] = b""
        for fn in sorted(fns):
            with open(os.path.join(dp, fn), "rb") as f:
                out[(rel_dir + "/" + fn).lstrip("/")] = f.read()
    return out


def stamps_in(blobs):
    seen = set()
    for data in blobs:
        text = data.decode("utf-8", "replace")
        seen.update(NOW_T.findall(text))
        seen.update(NOW_STAMP.findall(text))
    return seen


def mask(data, fixture_stamps, roots):
    text = data.decode("utf-8", "replace") if isinstance(data, bytes) else data
    for r in roots:
        for form in (r.replace("\\", "\\" * 4), r.replace("\\", "\\" * 2), r, r.replace("\\", "/")):
            text = text.replace(form, "<ROOT>")
    text = TS_FIELD.sub('"ts": "<TS>"', text)
    text = NOW_T.sub(lambda m: m.group(0) if m.group(0) in fixture_stamps else "<NOW>", text)
    text = NOW_STAMP.sub(lambda m: m.group(0) if m.group(0) in fixture_stamps else "<STAMP>", text)
    return text


def error_lines(stderr):
    return [ln for ln in stderr.decode("utf-8", "replace").splitlines() if ln.startswith("af: ERROR:")]


def run_side(root, steps, rust):
    env = dict(os.environ)
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    env.pop("AF_ROOT", None)
    if rust:
        env["AF_ROOT"] = root
    results = []
    for step in steps:
        if callable(step):
            step(root)
            results.append(None)
            continue
        cmd = [BIN] + step if rust else [sys.executable, os.path.join(root, "system", "af.py")] + step
        p = subprocess.run(cmd, cwd=root, env=env, capture_output=True)
        results.append(p)
    return results


def first_line_diff(a, b):
    al, bl = a.splitlines(), b.splitlines()
    for i, (x, y) in enumerate(zip(al, bl)):
        if x != y:
            return f"line {i + 1}: py={x[:160]!r} rs={y[:160]!r}"
    if len(al) != len(bl):
        i = min(len(al), len(bl))
        extra = (al[i] if len(al) > i else bl[i])[:160]
        return f"line {i + 1}: {'py' if len(al) > len(bl) else 'rs'} has extra {extra!r}"
    return "line endings or trailing bytes differ"


def compare(name, build, keep=False, verbose=False):
    work = tempfile.mkdtemp(prefix=f"afparity-{name}-")
    py_root, rs_root = os.path.join(work, "py"), os.path.join(work, "rs")
    try:
        os.makedirs(py_root)
        base(py_root)
        steps = build(py_root)
        shutil.copytree(py_root, rs_root, copy_function=shutil.copy2)
        fixture = tree(py_root)
        fstamps = stamps_in(fixture.values())
        roots = [py_root, rs_root]
        py_res = run_side(py_root, steps, rust=False)
        rs_res = run_side(rs_root, steps, rust=True)
        for i, (step, a, b) in enumerate(zip(steps, py_res, rs_res)):
            if a is None:
                continue
            label = f"step {i + 1} `af {' '.join(step)}`"
            if verbose:
                print(f"--- {label}: exit {a.returncode}")
                print(mask(a.stdout, fstamps, roots).rstrip())
                for ln in error_lines(a.stderr):
                    print("    stderr:", mask(ln, fstamps, roots))
            if a.returncode != b.returncode:
                return f"{label}: exit py={a.returncode} rs={b.returncode}"
            sa, sb = mask(a.stdout, fstamps, roots), mask(b.stdout, fstamps, roots)
            if sa != sb:
                return f"{label} stdout {first_line_diff(sa, sb)}"
            ea = [mask(x, fstamps, roots) for x in error_lines(a.stderr)]
            eb = [mask(x, fstamps, roots) for x in error_lines(b.stderr)]
            if ea != eb:
                return f"{label} stderr refusal py={ea[:2]} rs={eb[:2]}"
        ta, tb = tree(py_root), tree(rs_root)
        if sorted(ta) != sorted(tb):
            only_a = sorted(set(ta) - set(tb))
            only_b = sorted(set(tb) - set(ta))
            return f"tree: only-py={only_a[:3]} only-rs={only_b[:3]}"
        for rel in sorted(ta):
            ma, mb = mask(ta[rel], fstamps, roots), mask(tb[rel], fstamps, roots)
            if ma != mb:
                return f"{rel}: {first_line_diff(ma, mb)}"
            # byte-level check beyond the masked text: line endings and BOMs
            if ta[rel].count(b"\r") != tb[rel].count(b"\r") or ta[rel][:3] != tb[rel][:3]:
                return f"{rel}: line endings or BOM differ"
        return None
    finally:
        if keep:
            print(f"  kept {work}")
        else:
            shutil.rmtree(work, ignore_errors=True)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("-k", help="run only cases whose name contains this")
    ap.add_argument("--keep", action="store_true", help="keep the fixture roots")
    ap.add_argument("-v", "--verbose", action="store_true", help="print each step's Python stdout and refusals")
    args = ap.parse_args()
    if not os.path.isfile(BIN):
        print(f"parity: release binary missing at {BIN}; run cargo build --release in system/af/")
        return 2
    failed = 0
    for name, build in CASES:
        if args.k and args.k not in name:
            continue
        diff = compare(name, build, keep=args.keep, verbose=args.verbose)
        if diff:
            failed += 1
            print(f"DIFF {name}: {diff}")
        else:
            print(f"PASS {name}")
    from parity_index import extra_cases  # R3: index/search, compared table by table
    for name, runner in extra_cases():
        if args.k and args.k not in name:
            continue
        status, detail = runner(args.keep, args.verbose)
        failed += status == "DIFF"
        print(f"{status} {name}" + (f": {detail}" if detail else ""))
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
