"""Careers pack rules — the only Python that knows careers.

Hosted by system/af.py's plugin host (never imports af.py; receives ctx).
Two hooks:
  check_application(ctx, adir, afm) -> (issues, notes)   # pipeline doctor
  on_ready(ctx, cdir, dpath, rel, cfm) -> (cfm, notes)   # verification gate
"""

import os
import re

# Characters that break ATS plain-text extraction (July-2026 parser research).
HAZARD_CHARS = (
    ("—", "em dash"),
    ("–", "en dash"),
    ("•", "bullet glyph"),
    ("▪", "square glyph"),
    ("→", "arrow glyph"),
    ("✓", "checkmark glyph"),
)

# Year-only ranges zero out ATS tenure calculators ("2022 - 2023" -> 0 months).
# The left year must be bare: "September 2022 - Present" is the format the resume
# template mandates, so a month prefix disqualifies the match.
MONTH = (r"(?:jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|jun(?:e)?|jul(?:y)?"
         r"|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)")
DATE_RANGE = re.compile(
    rf"(?:(?P<lmonth>{MONTH})\b\.?\s+)?(?P<lyear>(?:19|20)\d{{2}})"
    rf"\s*[-–—]\s*(?:(?:{MONTH})\b\.?\s+)?(?:(?:19|20)\d{{2}}|present)\b",
    re.I,
)


def year_only_ranges(text):
    """Date ranges whose left side carries no month — the tenure-zeroing shape."""
    return [m.group(0) for m in DATE_RANGE.finditer(text) if not m.group("lmonth")]

# Canonical resume headings; anything else gets miscategorized by entity mappers.
RESUME_HEADINGS = {"work experience", "experience", "projects", "education", "skills", "skills / extras", "extras"}

# Worst lexical AI tells (recruiter auto-reject territory). Notes, not issues —
# wording is judgment; the humanizer pass owns the rewrite.
AI_TELLS = ("spearheaded", "delve", "delving", "testament to", "synergies",
            "proven track record", "unique blend", "beacon of", "catalyst for")

# Text materials are ATS-parsed and get the hazard lint; every material
# (frontmatter `materials`) gets the jd-map verification gate.
TEXT_MATERIALS = ("resume", "cover-letter")


def _body(ctx, path, rel):
    _, body = ctx.split_fm(ctx.read(path), rel)
    return body


def _lint(rel, body, is_resume):
    issues = []
    for ch, label in HAZARD_CHARS:
        n = body.count(ch)
        if n:
            issues.append(f"{rel}: {n}x {label} ({ch}) — parse hazard, replace before export")
    bare = year_only_ranges(body)
    if bare:
        issues.append(f"{rel}: year-only date range ({bare[0]}) — use 'Month YYYY - Month YYYY' "
                      "(year-only zeroes tenure calculators)")
    if is_resume:
        for h in re.findall(r"^##\s+(.+?)\s*$", body, re.M):
            if h.strip().lower() not in RESUME_HEADINGS:
                issues.append(f"{rel}: non-canonical heading '## {h}' — parsers map entities by standard headings")
    return issues


def _tell_notes(rel, body):
    low = body.lower()
    hits = sorted({t for t in AI_TELLS if t in low})
    if hits:
        return [f"{rel}: AI-tell wording ({', '.join(hits)}) — run the humanizer pass before verification"]
    return []


ROUND_DIR = re.compile(r"^round-(\d+)-[a-z0-9-]+$")
ISO_DAY = re.compile(r"^\d{4}-\d{2}-\d{2}")


def _fm_of(ctx, path, rel):
    """Frontmatter of a freeform round file, or None when it carries none."""
    if not os.path.isfile(path):
        return None
    text = ctx.read(path)
    if not text.startswith("---"):
        return None
    fm, _ = ctx.split_fm(text, rel)
    return fm


def _round_notes(ctx, adir, rel_dir):
    """Interview-round drift: a held round with no debrief, or a debrief that never promoted.

    Notes, never issues — the operator judges these. The promote check is the one that matters:
    an unpromoted debrief means the living dossiers are stale and the next round preps against
    an old picture, which is the debt this structure exists to prevent.
    """
    notes = []
    today_s = ctx.today()
    for name in sorted(os.listdir(adir)):
        m = ROUND_DIR.fullmatch(name)
        rdir = os.path.join(adir, name)
        if not m or not os.path.isdir(rdir):
            continue
        n = m.group(1)
        rfm = _fm_of(ctx, os.path.join(rdir, "README.md"), f"{name}/README.md")
        held = (ctx.get_scalar(rfm, "held_at") or "").strip() if rfm else ""
        was_held = bool(ISO_DAY.match(held)) and held[:10] <= today_s

        dpath = os.path.join(rdir, "debrief.md")
        if not os.path.isfile(dpath):
            if was_held:
                notes.append(f"{rel_dir}/{name}: round {n} was held {held[:10]} and has no debrief.md — "
                             "write it today; from memory a week later is the failure mode")
            continue

        dfm = _fm_of(ctx, dpath, f"{name}/debrief.md")
        if dfm is None:
            continue
        completeness = (ctx.get_scalar(dfm, "completeness") or "").strip()
        promoted = (ctx.get_scalar(dfm, "promoted") or "").strip().lower()
        if completeness.lower().startswith("partial"):
            notes.append(f"{rel_dir}/{name}/debrief.md: completeness is partial — "
                         "the open questions need the operator before the next round preps against it")
        if promoted != "true":
            notes.append(f"{rel_dir}/{name}/debrief.md: promoted is '{promoted or 'unset'}' — "
                         "company-brief.md / people/ / jd-map.md have not received this round's facts")
    return notes


def check_application(ctx, adir, afm):
    """Doctor pass over the application's material deliverables."""
    issues, notes = [], []
    rel_dir = os.path.relpath(adir, ctx.ROOT).replace("\\", "/")
    notes += _round_notes(ctx, adir, rel_dir)
    for slug in TEXT_MATERIALS:
        st, frel = ctx.row_get(afm, slug, "status"), ctx.row_get(afm, slug, "file")
        if not frel or st in (None, "not_started"):
            continue
        path = os.path.join(adir, frel)
        if not os.path.isfile(path):
            continue  # missing files are the spine's finding
        body = _body(ctx, path, frel)
        rel = f"{rel_dir}/{frel}"
        issues += _lint(rel, body, is_resume=(slug == "resume"))
        notes += _tell_notes(rel, body)
    return issues, notes


def _verification_filled(ctx, cdir):
    jm = os.path.join(cdir, "jd-map.md")
    if not os.path.isfile(jm):
        return False
    text = ctx.read(jm)
    m = re.search(r"^##\s+Verification\s*$(.*?)(?=^##\s|\Z)", text, re.M | re.S)
    return bool(m and m.group(1).strip())


def on_ready(ctx, cdir, dpath, rel, cfm):
    """Readiness gate for submission materials: verification recorded; text materials also hazard-clean."""
    m = re.fullmatch(r"(.+)-v\d+\.md", os.path.basename(dpath))
    if not m:
        return cfm, []
    name = m.group(1)
    materials = set(ctx.fm_list(cfm, "materials") or ["resume"]) | set(TEXT_MATERIALS)
    if name not in materials:
        return cfm, []
    if not _verification_filled(ctx, cdir):
        ctx.die(f"{rel}: ready refused — jd-map.md '## Verification' is missing or empty. "
                "Run the verification pass (criteria/requirements mapped, materials reviewed) "
                "and record it, then rerun.")
    if name in TEXT_MATERIALS:
        hazards = _lint(rel, _body(ctx, dpath, rel), is_resume=(name == "resume"))
        if hazards:
            ctx.die(f"{rel}: ready refused — parse hazards present: {'; '.join(hazards)}")
    return cfm, ["verification gate passed"]
