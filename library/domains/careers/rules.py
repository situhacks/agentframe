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
YEAR_ONLY_RANGE = re.compile(r"\b(19|20)\d{2}\s*[-–—]\s*((19|20)\d{2}|present)\b", re.I)

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
    if YEAR_ONLY_RANGE.search(body):
        issues.append(f"{rel}: year-only date range — use 'Month YYYY - Month YYYY' (year-only zeroes tenure calculators)")
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


def check_application(ctx, adir, afm):
    """Doctor pass over the application's material deliverables."""
    issues, notes = [], []
    rel_dir = os.path.relpath(adir, ctx.ROOT).replace("\\", "/")
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
