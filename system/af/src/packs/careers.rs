//! Careers pack rules (library/domains/careers/rules.py): the readiness gate for
//! submission materials and the pipeline doctor pass over applications.

use crate::core::{self, today, Ctx};
use crate::die;
use crate::error::R;
use crate::fm::{fm_list, get_scalar, row_get, rx, split_fm};
use crate::pyos::{self, join, py_strip};

const HAZARD_CHARS: [(&str, &str); 6] = [
    ("—", "em dash"),
    ("–", "en dash"),
    ("•", "bullet glyph"),
    ("▪", "square glyph"),
    ("→", "arrow glyph"),
    ("✓", "checkmark glyph"),
];

const MONTH: &str = r"(?:jan(?:uary)?|feb(?:ruary)?|mar(?:ch)?|apr(?:il)?|may|jun(?:e)?|jul(?:y)?|aug(?:ust)?|sep(?:t(?:ember)?)?|oct(?:ober)?|nov(?:ember)?|dec(?:ember)?)";

const RESUME_HEADINGS: [&str; 7] = ["work experience", "experience", "projects", "education", "skills", "skills / extras", "extras"];

const AI_TELLS: [&str; 9] = [
    "spearheaded",
    "delve",
    "delving",
    "testament to",
    "synergies",
    "proven track record",
    "unique blend",
    "beacon of",
    "catalyst for",
];

pub const TEXT_MATERIALS: [&str; 2] = ["resume", "cover-letter"];

fn date_range_re() -> regex::Regex {
    rx(&format!(
        r"(?i)(?:(?P<lmonth>{MONTH})\b\.?\s+)?(?P<lyear>(?:19|20)\d{{2}})\s*[-–—]\s*(?:(?:{MONTH})\b\.?\s+)?(?:(?:19|20)\d{{2}}|present)\b"
    ))
}

/// Date ranges whose left side carries no month.
pub fn year_only_ranges(text: &str) -> Vec<String> {
    date_range_re()
        .captures_iter(text)
        .filter(|c| c.name("lmonth").is_none())
        .map(|c| c[0].to_string())
        .collect()
}

pub fn lint(rel: &str, body: &str, is_resume: bool) -> Vec<String> {
    let mut issues = Vec::new();
    for (ch, label) in HAZARD_CHARS {
        let n = body.matches(ch).count();
        if n > 0 {
            issues.push(format!("{rel}: {n}x {label} ({ch}) — parse hazard, replace before export"));
        }
    }
    let bare = year_only_ranges(body);
    if let Some(first) = bare.first() {
        issues.push(format!(
            "{rel}: year-only date range ({first}) — use 'Month YYYY - Month YYYY' (year-only zeroes tenure calculators)"
        ));
    }
    if is_resume {
        for c in rx(r"(?m)^##\s+(.+?)\s*$").captures_iter(body) {
            let h = &c[1];
            if !RESUME_HEADINGS.contains(&py_strip(h).to_lowercase().as_str()) {
                issues.push(format!("{rel}: non-canonical heading '## {h}' — parsers map entities by standard headings"));
            }
        }
    }
    issues
}

fn tell_notes(rel: &str, body: &str) -> Vec<String> {
    let low = body.to_lowercase();
    let mut hits: Vec<&str> = AI_TELLS.iter().copied().filter(|t| low.contains(t)).collect();
    hits.sort();
    hits.dedup();
    if hits.is_empty() {
        return Vec::new();
    }
    vec![format!(
        "{rel}: AI-tell wording ({}) — rewrite it in the voice clean pass before verification",
        hits.join(", ")
    )]
}

fn body_of(path: &str, rel: &str) -> R<String> {
    Ok(split_fm(&core::read(path)?, rel)?.1)
}

/// Frontmatter of a freeform round file, or None when it carries none.
fn fm_of(path: &str, rel: &str) -> R<Option<String>> {
    if !pyos::isfile(path) {
        return Ok(None);
    }
    let text = core::read(path)?;
    if !text.starts_with("---") {
        return Ok(None);
    }
    Ok(Some(split_fm(&text, rel)?.0))
}

fn round_notes(adir: &str, rel_dir: &str) -> R<Vec<String>> {
    let mut notes = Vec::new();
    let today_s = today();
    let round_dir = rx(r"\Around-(\d+)-[a-z0-9-]+\z");
    let iso_day = rx(r"\A\d{4}-\d{2}-\d{2}");
    for name in pyos::sorted_listdir(adir)? {
        let rdir = join(adir, &name);
        let n = match round_dir.captures(&name) {
            Some(c) if pyos::isdir(&rdir) => c[1].to_string(),
            _ => continue,
        };
        let rfm = fm_of(&join(&rdir, "README.md"), &format!("{name}/README.md"))?;
        let held = match &rfm {
            Some(f) if !f.is_empty() => py_strip(&get_scalar(f, "held_at").unwrap_or_default()).to_string(),
            _ => String::new(),
        };
        let held10 = pyos::py_prefix(&held, 10);
        let was_held = iso_day.is_match(&held) && held10.as_str() <= today_s.as_str();

        let dpath = join(&rdir, "debrief.md");
        if !pyos::isfile(&dpath) {
            if was_held {
                notes.push(format!(
                    "{rel_dir}/{name}: round {n} was held {held10} and has no debrief.md — write it today; from memory a week later is the failure mode"
                ));
            }
            continue;
        }
        let dfm = match fm_of(&dpath, &format!("{name}/debrief.md"))? {
            Some(f) => f,
            None => continue,
        };
        let completeness = py_strip(&get_scalar(&dfm, "completeness").unwrap_or_default()).to_string();
        let promoted = py_strip(&get_scalar(&dfm, "promoted").unwrap_or_default()).to_lowercase();
        if completeness.to_lowercase().starts_with("partial") {
            notes.push(format!(
                "{rel_dir}/{name}/debrief.md: completeness is partial — the open questions need the operator before the next round preps against it"
            ));
        }
        if promoted != "true" {
            let shown = if promoted.is_empty() { "unset".to_string() } else { promoted.clone() };
            notes.push(format!(
                "{rel_dir}/{name}/debrief.md: promoted is '{shown}' — company-brief.md / people/ / jd-map.md have not received this round's facts"
            ));
        }
    }
    Ok(notes)
}

/// Doctor pass over the application's material deliverables: (issues, notes).
pub fn check_application(ctx: &Ctx, adir: &str, afm: &str) -> R<(Vec<String>, Vec<String>)> {
    let mut issues = Vec::new();
    let mut notes = Vec::new();
    let rel_dir = ctx.rel(adir)?;
    notes.extend(round_notes(adir, &rel_dir)?);
    for slug in TEXT_MATERIALS {
        let st = row_get(afm, slug, "status");
        let frel = row_get(afm, slug, "file");
        let frel = match frel {
            Some(f) if !f.is_empty() => f,
            _ => continue,
        };
        if matches!(st.as_deref(), None | Some("not_started")) {
            continue;
        }
        let path = join(adir, &frel);
        if !pyos::isfile(&path) {
            continue;
        }
        let body = body_of(&path, &frel)?;
        let rel = format!("{rel_dir}/{frel}");
        issues.extend(lint(&rel, &body, slug == "resume"));
        notes.extend(tell_notes(&rel, &body));
    }
    Ok((issues, notes))
}

fn verification_filled(cdir: &str) -> R<bool> {
    let jm = join(cdir, "jd-map.md");
    if !pyos::isfile(&jm) {
        return Ok(false);
    }
    let text = core::read(&jm)?;
    let head = match rx(r"(?m)^##\s+Verification\s*$").find(&text) {
        None => return Ok(false),
        Some(m) => m,
    };
    let start = head.end();
    let line_start = |q: usize| q == 0 || text.as_bytes()[q - 1] == b'\n';
    let stop = rx(r"\A##\s");
    let end = (start..=text.len())
        .find(|&q| q == text.len() || (line_start(q) && text.is_char_boundary(q) && stop.is_match(&text[q..])))
        .unwrap_or(text.len());
    Ok(!py_strip(&text[start..end]).is_empty())
}

/// Readiness gate for submission materials.
pub fn on_ready(cdir: &str, dpath: &str, rel: &str, cfm: String) -> R<(String, Vec<String>)> {
    let base = pyos::basename(dpath);
    let name = match rx(r"\A(.+)-v\d+\.md\z").captures(&base) {
        Some(c) => c[1].to_string(),
        None => return Ok((cfm, Vec::new())),
    };
    let mut materials = fm_list(&cfm, "materials");
    if materials.is_empty() {
        materials = vec!["resume".to_string()];
    }
    materials.extend(TEXT_MATERIALS.iter().map(|s| s.to_string()));
    if !materials.contains(&name) {
        return Ok((cfm, Vec::new()));
    }
    if !verification_filled(cdir)? {
        die!(
            "{rel}: ready refused — jd-map.md '## Verification' is missing or empty. Run the verification pass (criteria/requirements mapped, materials reviewed) and record it, then rerun."
        );
    }
    if TEXT_MATERIALS.contains(&name.as_str()) {
        let hazards = lint(rel, &body_of(dpath, rel)?, name == "resume");
        if !hazards.is_empty() {
            die!("{rel}: ready refused — parse hazards present: {}", hazards.join("; "));
        }
    }
    Ok((cfm, vec!["verification gate passed".to_string()]))
}
