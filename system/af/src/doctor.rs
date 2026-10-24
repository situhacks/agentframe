//! `af doctor`: project, pipeline, studio, and system-surface checks. Issues are broken
//! invariants (exit 1); notes are drift alarms. Nothing is ever auto-fixed.

use crate::core::{
    self, exportable_ingredient, load_pack, manifest_path_status, pack_exportables, parse_iso_date, project_dir, py_opt,
    read, today_date, truthy, versions_in, Ctx, BINARY_HEAD_HINT, LIFECYCLE_ENUM, PROJECT_SCHEMA_VERSION, STATUS_ENUM,
};
use crate::error::{soft, R};
use crate::fm::{
    self, all_rows, clean_value, fm_list, fm_list_values, get_scalar, has_field, mapping_row_get, mapping_rows, pipe_rows,
    row_get, row_span, rx, split_fm, split_fm_optional,
};
use crate::media_shelf;
use crate::outln;
use crate::packs::{self, careers, Rules};
use crate::pyglob;
use crate::pyos::{self, join, join_all, py_splitlines, py_strip};
use crate::runlog;

pub const ACTIVITY_LINE_CAP: usize = 200;
pub const ACTIVITY_ARCHIVE_LINES: usize = 200;
pub const MEDIA_MANIFEST_FIELDS: [&str; 2] = ["shipped_media", "exports"];
pub const MEDIA_PREVIEW_EXTS: [&str; 13] =
    [".html", ".png", ".jpg", ".jpeg", ".webp", ".gif", ".svg", ".pdf", ".mp4", ".mov", ".webm", ".pptx", ".docx"];

pub const PIPE_STAGES: [&str; 10] =
    ["saved", "preparing", "applied", "interviewing", "offer", "accepted", "declined", "rejected", "ghosted", "dropped"];
pub const PIPE_STALE_SAVED_DAYS: i64 = 30;
pub const STUDIO_STATES: [&str; 6] = ["planned", "captured", "cut", "scheduled", "posted", "dropped"];
pub const STUDIO_CAPTURE_DAYS: i64 = 14;
pub const STUDIO_POST_FIELDS: [&str; 7] = ["name", "slug", "schema_version", "created_at", "domain", "platform", "last_activity"];
pub const APPS_ARCHIVE: &str = "completed";

pub const PERSONA_WORD_BUDGET: usize = 1000;
pub const PROCESS_WORD_BUDGET: usize = 1500;
pub const TEMPLATE_WORD_BUDGET: usize = 900;

pub const LINK_SCAN_GLOBS: [&str; 13] = [
    "AGENTS.md",
    "AGENTS.builder.md",
    "AGENTS.operator.md",
    "AGENTS.worker.md",
    ".claude/agents/*.md",
    "library/process/**/*.md",
    "library/deliverables/**/*.md",
    "library/domains/**/*.md",
    "library/assets/README.md",
    "system/vendor/README.md",
    "system/vendor/*/SKILL.md",
    ".claude/skills/README.md",
    ".claude/skills/*/SKILL.md",
];

pub fn activity_line_re() -> regex::Regex {
    rx(r"\A\d{4}-\d{2}-\d{2}( \d{2}:\d{2})?\s*[—-]\s*[a-z][a-z0-9_]*:\s")
}

fn is_versioned(base: &str) -> Option<(String, String)> {
    core::version_re().captures(base).map(|c| (c[1].to_string(), c[2].to_string()))
}

fn max_version(dir: &str, name: &str) -> u64 {
    versions_in(dir, name).into_iter().max().unwrap_or(0)
}

/// Read and split project.md, mirroring `try: split_fm(...) except SystemExit`.
fn soft_project_fm(cdir: &str) -> R<Option<String>> {
    let text = read(&join(cdir, "project.md"))?;
    Ok(soft(split_fm(&text, "project.md"))?.map(|p| p.0))
}

/// `split_fm_optional(read(p))` with `except (UnicodeDecodeError, OSError): None`.
fn read_optional_fm(p: &str) -> R<Option<(String, String)>> {
    match read(p) {
        Ok(t) => Ok(split_fm_optional(&t)),
        Err(e) if e.is_decode_or_os() => Ok(None),
        Err(e) => Err(e),
    }
}

// ------------------------------------------------------------------ rows

pub fn tracker_row_issues(rel: &str, cdir: &str, cfm: &str) -> Vec<String> {
    let mut issues = Vec::new();
    for dslug in all_rows(cfm) {
        let st = row_get(cfm, &dslug, "status");
        let f = row_get(cfm, &dslug, "file");
        if !matches!(st.as_deref(), Some(s) if STATUS_ENUM.contains(&s)) {
            issues.push(format!("{rel}: row '{dslug}' status '{}' invalid", py_opt(&st)));
        }
        let f = match f {
            Some(f) if !f.is_empty() => f,
            _ => {
                issues.push(format!("{rel}: row '{dslug}' has no file pointer"));
                continue;
            }
        };
        let p = join(cdir, &f);
        if st.as_deref() != Some("not_started") && !pyos::isfile(&p) {
            issues.push(format!("{rel}: row '{dslug}' file missing: {f}"));
        } else if pyos::isfile(&p) {
            if let Some((name, n)) = is_versioned(&pyos::basename(&p)) {
                let highest = max_version(&pyos::dirname(&p), &name);
                if n.parse::<u64>().unwrap_or(u64::MAX) != highest {
                    issues.push(format!("{rel}: row '{dslug}' points at v{n} but head is v{highest}"));
                }
            }
        }
    }
    issues
}

pub fn media_manifest_issues_for_fm(ctx: &Ctx, cdir: &str, cfm: &str, source_label: &str) -> R<Vec<String>> {
    let mut issues = Vec::new();
    let rel = ctx.rel(cdir)?;
    let extra = pack_exportables(ctx, cfm)?;
    for slug in all_rows(cfm) {
        let st = row_get(cfm, &slug, "status");
        let f = row_get(cfm, &slug, "file");
        let st = match st.as_deref() {
            Some(s @ ("ready" | "published")) => s.to_string(),
            _ => continue,
        };
        let f = match f {
            Some(f) if !f.is_empty() && pyos::isfile(&join(cdir, &f)) => f,
            _ => continue,
        };
        if !f.to_lowercase().ends_with(".md") {
            continue;
        }
        let dfm = match read_optional_fm(&join(cdir, &f))? {
            Some((d, _)) => d,
            None => {
                issues.push(format!(
                    "{rel}: {source_label} row '{slug}' file has no/invalid frontmatter or is unreadable: {f}"
                ));
                continue;
            }
        };
        if exportable_ingredient(&f, &extra).is_some() && fm_list_values(&dfm, "exports").is_empty() {
            issues.push(format!(
                "{rel}: {source_label} row '{slug}' is a {st} exportable deliverable with empty exports[] - land the finals in the deliverable's media/ folder and record them"
            ));
        }
        for field in MEDIA_MANIFEST_FIELDS {
            for value in fm_list_values(&dfm, field) {
                if let Some(status) = manifest_path_status(cdir, &f, &value) {
                    issues.push(format!("{rel}: {source_label} row '{slug}' {field} path {status}: {value}"));
                }
            }
        }
    }
    Ok(issues)
}

fn row_field_names(cfm: &str, slug: &str) -> Vec<(String, String)> {
    let (s, e) = match row_span(cfm, slug) {
        Some(sp) => sp,
        None => return Vec::new(),
    };
    rx(r"(?m)^    ([A-Za-z0-9_]*(?:_url|_draft))\s*:\s*(\S.*?)\s*$")
        .captures_iter(&cfm[s..e])
        .filter(|c| {
            let v = clean_value(&c[2]);
            !v.is_empty() && v != "null"
        })
        .map(|c| (c[1].to_string(), c[2].to_string()))
        .collect()
}

/// Rows that record a thing as published and not ready at the same time.
pub fn state_truth_issues(ctx: &Ctx, cdir: &str, cfm: &str) -> R<Vec<String>> {
    let mut issues = Vec::new();
    let rel = ctx.rel(cdir)?;
    for slug in all_rows(cfm) {
        let st = row_get(cfm, &slug, "status");
        if matches!(st.as_deref(), Some("ready" | "published")) {
            continue;
        }
        for (field, value) in row_field_names(cfm, &slug) {
            issues.push(format!(
                "{rel}: project.md row '{slug}' is '{}' but carries {field} ({}) - the row records the thing as published and not ready at once. Publishing off-button skips every gate hanging off readiness.",
                py_opt(&st),
                clean_value(&value)
            ));
        }
    }
    Ok(issues)
}

pub fn empty_head_notes(ctx: &Ctx, cdir: &str) -> R<Vec<String>> {
    let mut notes = Vec::new();
    let rel = ctx.rel(cdir)?;
    let cfm = match soft_project_fm(cdir)? {
        Some(c) => c,
        None => return Ok(notes),
    };
    for slug in all_rows(&cfm) {
        let st = row_get(&cfm, &slug, "status");
        let f = row_get(&cfm, &slug, "file");
        if matches!(st.as_deref(), None | Some("") | Some("not_started")) {
            continue;
        }
        let f = match f {
            Some(f) if !f.is_empty() && f.to_lowercase().ends_with(".md") => f,
            _ => continue,
        };
        let path = join(cdir, &f);
        if !pyos::isfile(&path) {
            continue;
        }
        let parsed = match read(&path) {
            Ok(t) => split_fm_optional(&t),
            Err(e) if e.is_decode_or_os() => continue,
            Err(e) => return Err(e),
        };
        if let Some((_, body)) = parsed {
            if py_strip(&body).is_empty() {
                notes.push(format!(
                    "{rel}: row '{slug}' points at an empty head ({f}) - fill it, or check the address if you did not just create it ('af draft --file' takes a PROJECT-relative path)"
                ));
            }
        }
    }
    Ok(notes)
}

pub fn media_manifest_issues(ctx: &Ctx, cdir: &str, cfm: &str) -> R<Vec<String>> {
    let mut issues = media_manifest_issues_for_fm(ctx, cdir, cfm, "project.md")?;
    let archive = join_all(&[cdir, "knowledge", "_archive", "deliverables-archive.md"]);
    if pyos::isfile(&archive) {
        let (afm, _) = split_fm(&read(&archive)?, "deliverables-archive.md")?;
        issues.extend(media_manifest_issues_for_fm(ctx, cdir, &afm, "deliverables-archive.md")?);
    }
    Ok(issues)
}

fn project_manifest_ingredients(cfm: &str) -> Vec<String> {
    match rx(r"(?m)^\s*ingredients:\s*\[(.*?)\]\s*$").captures(cfm) {
        None => Vec::new(),
        Some(c) => c[1].split(',').map(clean_value).filter(|s| !s.is_empty()).collect(),
    }
}

/// `is_dir()` for a walkdir entry without a stat per file (links still followed).
fn walk_is_dir(e: &walkdir::DirEntry) -> bool {
    let ft = e.file_type();
    ft.is_dir() || (ft.is_symlink() && e.path().is_dir())
}

fn has_previewable_file(folder: &str) -> bool {
    if !pyos::isdir(folder) {
        return false;
    }
    let walker = walkdir::WalkDir::new(folder).into_iter().filter_entry(|e| {
        e.depth() == 0 || !walk_is_dir(e) || {
            let name = e.file_name().to_string_lossy();
            !(name.starts_with('.') || name == "history" || name == "raw")
        }
    });
    for e in walker.flatten() {
        if e.depth() == 0 || walk_is_dir(&e) {
            continue;
        }
        let name = e.file_name().to_string_lossy().into_owned();
        let ext = pyos::splitext(&name).1.to_lowercase();
        if MEDIA_PREVIEW_EXTS.contains(&ext.as_str()) {
            return true;
        }
    }
    false
}

pub fn media_manifest_notes(ctx: &Ctx, cdir: &str) -> R<Vec<String>> {
    let mut notes = Vec::new();
    let rel = ctx.rel(cdir)?;
    let cfm = match soft_project_fm(cdir)? {
        Some(c) => c,
        None => return Ok(notes),
    };
    let project_ingredients = project_manifest_ingredients(&cfm);
    for slug in all_rows(&cfm) {
        if !rx(r"\Apost-\d+\n?\z").is_match(&slug) || row_get(&cfm, &slug, "status").as_deref() != Some("published") {
            continue;
        }
        let f = match row_get(&cfm, &slug, "file") {
            Some(f) if !f.is_empty() && pyos::isfile(&join(cdir, &f)) => f,
            _ => continue,
        };
        let (dfm, _) = split_fm(&read(&join(cdir, &f))?, &f)?;
        if !fm_list_values(&dfm, "shipped_media").is_empty() {
            continue;
        }
        let row_ingredients = fm::list_from_value(row_get(&cfm, &slug, "ingredients").as_deref());
        let ingredients = if row_ingredients.is_empty() { project_ingredients.clone() } else { row_ingredients };
        let post_dir = pyos::dirname(&join(cdir, &f));
        let image_bearing = ingredients.iter().any(|i| i == "image-prompts")
            || has_previewable_file(&join(&post_dir, "visuals"))
            || has_previewable_file(&join(&post_dir, "media"));
        if image_bearing {
            notes.push(format!(
                "{rel}: published {slug} has imagery signals but empty shipped_media[] - land shipped media in the post folder and record it on post-FINAL.md"
            ));
        }
    }
    Ok(notes)
}

/// One line when an active project's activity.md outgrows the archive threshold.
pub fn activity_size_note(ctx: &Ctx, cdir: &str) -> R<Option<String>> {
    let rel = ctx.rel(cdir)?;
    let cfm = match soft_project_fm(cdir)? {
        Some(c) => c,
        None => return Ok(None),
    };
    if get_scalar(&cfm, "status").as_deref() != Some("active") {
        return Ok(None);
    }
    let path = join(cdir, "activity.md");
    if !pyos::isfile(&path) {
        return Ok(None);
    }
    let lines = py_splitlines(&read(&path)?).len();
    if lines <= ACTIVITY_ARCHIVE_LINES {
        return Ok(None);
    }
    Ok(Some(format!(
        "{rel}: activity.md {lines} lines, over {ACTIVITY_ARCHIVE_LINES}; archive the tail to knowledge/_archive/"
    )))
}

/// Shape lint for activity.md (active projects only).
pub fn activity_notes(ctx: &Ctx, cdir: &str) -> R<Vec<String>> {
    let path = join(cdir, "activity.md");
    if !pyos::isfile(&path) {
        return Ok(Vec::new());
    }
    let cfm = match soft_project_fm(cdir)? {
        Some(c) => c,
        None => return Ok(Vec::new()),
    };
    if get_scalar(&cfm, "status").as_deref() != Some("active") {
        return Ok(Vec::new());
    }
    let rel = ctx.rel(cdir)?;
    let re = activity_line_re();
    let (mut unshaped, mut overlong) = (0usize, 0usize);
    for raw in py_splitlines(&read(&path)?) {
        let s = py_strip(&raw);
        if s.is_empty() || s.starts_with('#') || s.starts_with("- [") {
            continue;
        }
        if !re.is_match(s) {
            unshaped += 1;
        }
        if s.chars().count() > ACTIVITY_LINE_CAP {
            overlong += 1;
        }
    }
    let mut notes = Vec::new();
    if unshaped > 0 {
        notes.push(format!(
            "{rel}: activity.md {unshaped} line(s) not shaped 'YYYY-MM-DD [HH:MM] — event_type: ...' — one line per material event"
        ));
    }
    if overlong > 0 {
        notes.push(format!(
            "{rel}: activity.md {overlong} line(s) over {ACTIVITY_LINE_CAP} chars — iteration narration belongs in the version files' changes_from_vN, not here"
        ));
    }
    Ok(notes)
}

/// Advisory: rows pointing at a binary artifact.
pub fn binary_head_notes(ctx: &Ctx, cdir: &str) -> R<Vec<String>> {
    let mut notes = Vec::new();
    let rel_dir = ctx.rel(cdir)?;
    let cfm = match soft_project_fm(cdir)? {
        Some(c) => c,
        None => return Ok(notes),
    };
    for slug in all_rows(&cfm) {
        if let Some(f) = row_get(&cfm, &slug, "file") {
            if !f.is_empty() && !f.to_lowercase().ends_with(".md") {
                notes.push(format!(
                    "{rel_dir}: row '{slug}' points at a binary head ({f}) — no button can stamp it; {BINARY_HEAD_HINT}"
                ));
            }
        }
    }
    Ok(notes)
}

// ------------------------------------------------------------------ project

pub fn check_project(ctx: &Ctx, cdir: &str) -> R<Vec<String>> {
    let mut issues = Vec::new();
    let rel = ctx.rel(cdir)?;
    let cfm = match soft_project_fm(cdir)? {
        Some(c) => c,
        None => return Ok(vec![format!("{rel}: project.md missing or has no frontmatter")]),
    };

    for field in ["name", "slug", "schema_version", "created_at", "domain", "status", "current_phase", "last_activity"] {
        if !truthy(&get_scalar(&cfm, field)) {
            issues.push(format!("{rel}: required field '{field}' missing"));
        }
    }
    if !has_field(&cfm, "deliverables") {
        issues.push(format!("{rel}: required field 'deliverables' missing"));
    }
    if get_scalar(&cfm, "schema_version").as_deref() != Some(PROJECT_SCHEMA_VERSION) {
        issues.push(format!("{rel}: schema_version must be {PROJECT_SCHEMA_VERSION}; migrate the project forward"));
    }
    let slug = get_scalar(&cfm, "slug");
    if let Some(s) = &slug {
        if *s != pyos::basename(cdir) {
            issues.push(format!("{rel}: slug '{s}' != folder name"));
        }
    }
    let lifecycle = get_scalar(&cfm, "status");
    if !matches!(lifecycle.as_deref(), Some(l) if LIFECYCLE_ENUM.contains(&l)) {
        issues.push(format!(
            "{rel}: lifecycle status '{}' not in ['active', 'cancelled', 'complete']",
            py_opt(&lifecycle)
        ));
    }
    let set = |v: &Option<String>| matches!(v.as_deref(), Some(s) if !s.is_empty() && s != "null");
    let completed_at = get_scalar(&cfm, "completed_at");
    let cancelled_at = get_scalar(&cfm, "cancelled_at");
    let lc = lifecycle.as_deref();
    if lc == Some("complete") && !set(&completed_at) {
        issues.push(format!("{rel}: status complete requires completed_at"));
    }
    if lc == Some("cancelled") && !set(&cancelled_at) {
        issues.push(format!("{rel}: status cancelled requires cancelled_at"));
    }
    if lc != Some("complete") && set(&completed_at) {
        issues.push(format!("{rel}: completed_at is present but status is {}", py_opt(&lifecycle)));
    }
    if lc != Some("cancelled") && set(&cancelled_at) {
        issues.push(format!("{rel}: cancelled_at is present but status is {}", py_opt(&lifecycle)));
    }

    let domain = get_scalar(&cfm, "domain");
    let (desc, _) = load_pack(ctx, domain.as_deref())?;
    if truthy(&domain) && !truthy(&desc) {
        issues.push(format!(
            "{rel}: domain '{}' has no pack at library/domains/{}/",
            domain.as_deref().unwrap_or(""),
            domain.as_deref().unwrap_or("")
        ));
    }
    if let Some(d) = desc.as_ref().filter(|d| !d.is_empty()) {
        for f in fm_list(d, "extension_fields") {
            if !has_field(&cfm, &f) {
                issues.push(format!("{rel}: domain '{}' requires field '{f}' (missing)", py_opt(&domain)));
            }
        }
    }

    for slug in all_rows(&cfm) {
        let st = row_get(&cfm, &slug, "status");
        let f = row_get(&cfm, &slug, "file");
        if !matches!(st.as_deref(), Some(s) if STATUS_ENUM.contains(&s)) {
            issues.push(format!("{rel}: row '{slug}' status '{}' invalid", py_opt(&st)));
        }
        let f = match f {
            Some(f) if !f.is_empty() && f != "null" => f,
            _ => {
                issues.push(format!("{rel}: row '{slug}' has no file pointer"));
                continue;
            }
        };
        let assembly = domain.as_deref() == Some("marketing") && f.ends_with("post-FINAL.md");
        let base = pyos::basename(&f);
        if !assembly && !rx(r"\A.+-v\d+\.md\z").is_match(&base) {
            issues.push(format!("{rel}: row '{slug}' file is not a numeric version head: {f}"));
        }
        let p = join(cdir, &f);
        if st.as_deref() == Some("not_started") {
            // planned path may not exist yet
        } else if !pyos::isfile(&p) {
            issues.push(format!("{rel}: row '{slug}' file missing: {f}"));
        } else {
            let parsed = match read_optional_fm(&p)? {
                Some(p) => p,
                None => {
                    issues.push(format!("{rel}: row '{slug}' file has no/invalid frontmatter or is unreadable: {f}"));
                    continue;
                }
            };
            let artifact_status = get_scalar(&parsed.0, "status");
            if artifact_status != st {
                issues.push(format!(
                    "{rel}: row '{slug}' status {} != artifact status {}: {f}",
                    py_opt(&st),
                    py_opt(&artifact_status)
                ));
            }
            if let Some((name, n)) = is_versioned(&pyos::basename(&p)) {
                let highest = max_version(&pyos::dirname(&p), &name);
                if n.parse::<u64>().unwrap_or(u64::MAX) != highest {
                    issues.push(format!("{rel}: row '{slug}' points at v{n} but head is v{highest}"));
                }
            }
        }
    }

    if get_scalar(&cfm, "status").as_deref() == Some("complete") {
        let unfinished: Vec<String> = all_rows(&cfm)
            .into_iter()
            .filter(|s| matches!(row_get(&cfm, s, "status").as_deref(), Some("not_started" | "drafting")))
            .collect();
        if !unfinished.is_empty() {
            issues.push(format!("{rel}: completed project has unfinished rows: {}", unfinished.join(", ")));
        }
    }

    for c_slug in fm_list(&cfm, "channels") {
        let c_file = join_all(&[&ctx.root, "workspace", "life", "channels", &c_slug, "profile.md"]);
        if !pyos::isfile(&c_file) {
            issues.push(format!(
                "{rel}: channel '{c_slug}' referenced in frontmatter but workspace/life/channels/{c_slug}/profile.md does not exist"
            ));
        }
    }
    for s_slug in fm_list(&cfm, "stakeholders") {
        let s_file = join_all(&[&ctx.root, "workspace", "life", "people", &format!("{s_slug}.md")]);
        if !pyos::isfile(&s_file) {
            issues.push(format!(
                "{rel}: stakeholder '{s_slug}' referenced in frontmatter but workspace/life/people/{s_slug}.md does not exist"
            ));
        }
        let overlay = join_all(&[cdir, "knowledge", "people", &format!("{s_slug}.md")]);
        if !pyos::isfile(&overlay) {
            issues.push(format!(
                "{rel}: stakeholder '{s_slug}' per-project overlay missing at knowledge/people/{s_slug}.md"
            ));
        }
    }

    issues.extend(media_manifest_issues(ctx, cdir, &cfm)?);
    issues.extend(state_truth_issues(ctx, cdir, &cfm)?);
    Ok(issues)
}

// ------------------------------------------------------------------ pipeline

fn apps_root(ctx: &Ctx) -> String {
    join(&ctx.pipeline, "applications")
}

fn app_dir(ctx: &Ctx, slug: &str) -> String {
    let active = join(&apps_root(ctx), slug);
    let archived = join_all(&[&apps_root(ctx), APPS_ARCHIVE, slug]);
    if !pyos::isdir(&active) && pyos::isdir(&archived) {
        return archived;
    }
    active
}

pub fn check_pipeline(ctx: &Ctx) -> R<(Vec<String>, Vec<String>)> {
    let (mut issues, mut notes) = (Vec::new(), Vec::new());
    let board = join(&ctx.pipeline, "pipeline.md");
    if !pyos::isfile(&board) {
        return Ok((issues, notes));
    }
    let (bfm, _) = split_fm(&read(&board)?, "pipeline.md")?;
    let today_d = today_date();
    let rows = pipe_rows(&bfm);

    let root_dir = apps_root(ctx);
    let arch_dir = join(&root_dir, APPS_ARCHIVE);
    let mut folders: Vec<(String, String)> = Vec::new();
    if pyos::isdir(&root_dir) {
        for d in pyos::sorted_listdir(&root_dir)? {
            if pyos::isfile(&join_all(&[&root_dir, &d, "application.md"])) {
                folders.push((d, String::new()));
            }
        }
    }
    if pyos::isdir(&arch_dir) {
        for d in pyos::sorted_listdir(&arch_dir)? {
            if pyos::isfile(&join_all(&[&arch_dir, &d, "application.md"])) {
                folders.push((d, format!("{APPS_ARCHIVE}/")));
            }
        }
    }
    for (d, where_) in &folders {
        if !rows.contains(d) {
            issues.push(format!("workspace/pipeline/applications/{where_}{d}: folder has no board row"));
        }
    }

    let mut rules = None;
    if pyos::isdir(&ctx.domains) {
        for name in pyos::sorted_listdir(&ctx.domains)? {
            let (desc, pack_dir) = load_pack(ctx, Some(&name))?;
            if let Some(d) = desc.filter(|d| !d.is_empty()) {
                if get_scalar(&d, "topology").as_deref() == Some("pipeline") {
                    rules = packs::load_rules(ctx, pack_dir.as_deref())?;
                    break;
                }
            }
        }
    }

    for slug in &rows {
        let stage = row_get(&bfm, slug, "stage");
        let stage_s = match stage.as_deref() {
            Some(s) if PIPE_STAGES.contains(&s) => s.to_string(),
            _ => {
                issues.push(format!("pipeline.md: row '{slug}' stage '{}' invalid", py_opt(&stage)));
                continue;
            }
        };
        let adir = app_dir(ctx, slug);
        let started = pyos::isfile(&join(&adir, "application.md"));
        if stage_s != "saved" && stage_s != "dropped" && !started {
            issues.push(format!("pipeline.md: row '{slug}' is '{stage_s}' but has no application folder"));
        }
        if stage_s == "saved" {
            if let Some(saved) = parse_iso_date(row_get(&bfm, slug, "saved").as_deref()) {
                let days = (today_d - saved).num_days();
                if days > PIPE_STALE_SAVED_DAYS {
                    notes.push(format!(
                        "pipeline.md: '{slug}' saved {days}d ago — start it or drop it (ghost-job window passed)"
                    ));
                }
            }
        }
        if stage_s == "applied" || stage_s == "interviewing" {
            if let Some(nudge) = parse_iso_date(row_get(&bfm, slug, "next_nudge").as_deref()) {
                if nudge <= today_d {
                    notes.push(format!("pipeline.md: '{slug}' follow-up due since {} ({stage_s})", nudge.format("%Y-%m-%d")));
                }
            }
        }
        if !started {
            continue;
        }
        let rel = format!("workspace/pipeline/applications/{slug}");
        let (afm, _) = split_fm(&read(&join(&adir, "application.md"))?, "application.md")?;
        for field in ["name", "slug", "schema_version", "created_at", "domain", "company", "role", "job_url", "last_activity"] {
            if !truthy(&get_scalar(&afm, field)) {
                issues.push(format!("{rel}: required field '{field}' missing"));
            }
        }
        let mut materials = fm_list(&afm, "materials");
        if materials.is_empty() {
            materials = vec!["resume".to_string()];
        }
        let arows = all_rows(&afm);
        for mat in &materials {
            if !arows.contains(mat) {
                issues.push(format!("{rel}: materials names '{mat}' but no such deliverable row exists"));
            }
        }
        let aslug = get_scalar(&afm, "slug");
        if aslug.as_deref() != Some(slug.as_str()) {
            issues.push(format!("{rel}: slug '{}' != folder name", py_opt(&aslug)));
        }
        if stage_s != "saved" && !pyos::isfile(&join(&adir, "jd.md")) {
            issues.push(format!("{rel}: jd.md missing — tailoring without the verbatim posting is guesswork"));
        }
        issues.extend(tracker_row_issues(&rel, &adir, &afm));
        issues.extend(media_manifest_issues_for_fm(ctx, &adir, &afm, "application.md")?);
        if rules == Some(Rules::Careers) {
            let (ri, rn) = careers::check_application(ctx, &adir, &afm)?;
            issues.extend(ri);
            notes.extend(rn);
        }
    }
    Ok((issues, notes))
}

// ------------------------------------------------------------------ studio

pub fn check_studio(ctx: &Ctx) -> R<(Vec<String>, Vec<String>)> {
    let (mut issues, mut notes) = (Vec::new(), Vec::new());
    let calendar = join(&ctx.studio, "calendar.md");
    if !pyos::isfile(&calendar) {
        return Ok((issues, notes));
    }
    let (cfm, _) = split_fm(&read(&calendar)?, "calendar.md")?;
    let today_d = today_date();
    let rows = mapping_rows(&cfm, "slots");
    let root_dir = join(&ctx.studio, "posts");
    if pyos::isdir(&root_dir) {
        for d in pyos::sorted_listdir(&root_dir)? {
            if pyos::isfile(&join_all(&[&root_dir, &d, "post.md"])) && !rows.contains(&d) {
                issues.push(format!("workspace/studio/posts/{d}: folder has no board row"));
            }
        }
    }
    for slug in &rows {
        let state = mapping_row_get(&cfm, "slots", slug, "state");
        let state_s = match state.as_deref() {
            Some(s) if STUDIO_STATES.contains(&s) => s.to_string(),
            _ => {
                issues.push(format!("calendar.md: row '{slug}' state '{}' invalid", py_opt(&state)));
                continue;
            }
        };
        let pd = join(&root_dir, slug);
        let rel = format!("workspace/studio/posts/{slug}");
        if !pyos::isfile(&join(&pd, "post.md")) {
            if state_s != "dropped" {
                issues.push(format!("calendar.md: row '{slug}' is '{state_s}' but has no post folder"));
            }
            continue;
        }
        let render = join_all(&[&pd, "renders", "final.mp4"]);
        if (state_s == "cut" || state_s == "scheduled") && !pyos::isfile(&render) {
            issues.push(format!("{rel}: state '{state_s}' but renders/final.mp4 is missing"));
        }
        let slot_date = parse_iso_date(mapping_row_get(&cfm, "slots", slug, "date").as_deref());
        if let Some(sd) = slot_date {
            if state_s == "scheduled" && sd < today_d {
                notes.push(format!(
                    "calendar.md: '{slug}' was scheduled for {} and has no receipt — 'af studio post' if it went live, or restage it",
                    sd.format("%Y-%m-%d")
                ));
            }
            if (state_s == "planned" || state_s == "captured") && sd < today_d {
                notes.push(format!(
                    "calendar.md: '{slug}' slot {} passed at '{state_s}' — reschedule or drop",
                    sd.format("%Y-%m-%d")
                ));
            }
        }
        let (pfm, _) = split_fm(&read(&join(&pd, "post.md"))?, "post.md")?;
        for field in STUDIO_POST_FIELDS {
            if !truthy(&get_scalar(&pfm, field)) {
                issues.push(format!("{rel}: required field '{field}' missing"));
            }
        }
        let pslug = get_scalar(&pfm, "slug");
        if pslug.as_deref() != Some(slug.as_str()) {
            issues.push(format!("{rel}: slug '{}' != folder name", py_opt(&pslug)));
        }
        if state_s == "posted" {
            let posted_at = get_scalar(&pfm, "posted_at").unwrap_or_default();
            let posted = parse_iso_date(Some(&pyos::py_prefix(&posted_at, 10)));
            let captured = get_scalar(&pfm, "metrics_captured_at");
            if let Some(p) = posted {
                let days = (today_d - p).num_days();
                if matches!(captured.as_deref(), None | Some("") | Some("null")) && days >= STUDIO_CAPTURE_DAYS {
                    notes.push(format!(
                        "{rel}: posted {days}d ago, metrics not captured — capture per performance-capture.md into performance.csv and stamp metrics_captured_at"
                    ));
                }
            }
        }
        issues.extend(tracker_row_issues(&rel, &pd, &pfm));
        issues.extend(media_manifest_issues_for_fm(ctx, &pd, &pfm, "post.md")?);
    }
    Ok((issues, notes))
}

// ------------------------------------------------------------------ system surfaces

pub fn link_scan_files(ctx: &Ctx) -> Vec<String> {
    let mut seen: Vec<String> = Vec::new();
    for pat in LINK_SCAN_GLOBS {
        for p in pyglob::sorted_glob(&join(&ctx.root, pat), true) {
            if pyos::isfile(&p) && !seen.contains(&p) {
                seen.push(p);
            }
        }
    }
    seen
}

pub fn dead_link_issues(ctx: &Ctx) -> R<Vec<String>> {
    let mut issues = Vec::new();
    let link = rx(r"\[[^\]]*\]\(([^)\s]+)\)");
    for path in link_scan_files(ctx) {
        let rel = ctx.rel(&path)?;
        let text = read(&path)?;
        let text = rx(r"(?s)```.*?```").replace_all(&text, "").into_owned();
        let text = rx(r"`[^`\n]*`").replace_all(&text, "").into_owned();
        for c in link.captures_iter(&text) {
            let target = &c[1];
            if ["http://", "https://", "mailto:", "#", "data:"].iter().any(|p| target.starts_with(p)) || target.contains('{') {
                continue;
            }
            let bare = target.split('#').next().unwrap_or("");
            if bare.is_empty() {
                continue;
            }
            let local = pyos::normpath(&join(&pyos::dirname(&path), bare));
            let rooted = pyos::normpath(&join(&ctx.root, bare));
            let rel_target = ctx.rel(&local)?;
            if rel_target.starts_with("workspace/life/") || rel_target.starts_with("system/backlog") {
                continue;
            }
            if !(pyos::exists(&local) || pyos::exists(&rooted)) {
                issues.push(format!("{rel}: dead link -> {target}"));
            }
        }
    }
    Ok(issues)
}

pub fn budget_notes(ctx: &Ctx) -> R<Vec<String>> {
    let mut notes = Vec::new();
    let mut targets: Vec<(String, usize, &str)> = Vec::new();
    for f in ["AGENTS.builder.md", "AGENTS.operator.md"] {
        targets.push((join(&ctx.root, f), PERSONA_WORD_BUDGET, "persona"));
    }
    for p in pyglob::sorted_glob(&join_all(&[&ctx.root, "library", "process", "**", "*.md"]), true) {
        targets.push((p, PROCESS_WORD_BUDGET, "process file"));
    }
    let mut tpl = pyglob::sorted_glob(&join_all(&[&ctx.root, "library", "deliverables", "*", "template.md"]), false);
    tpl.extend(pyglob::sorted_glob(
        &join_all(&[&ctx.root, "library", "domains", "*", "deliverables", "*", "template.md"]),
        false,
    ));
    for p in tpl {
        targets.push((p, TEMPLATE_WORD_BUDGET, "template"));
    }
    for (path, cap, label) in targets {
        if !pyos::isfile(&path) {
            continue;
        }
        let words = pyos::py_split_ws(&read(&path)?).len();
        if words > cap {
            notes.push(format!(
                "size budget: {} {words} words (cap {cap}, {label}) — diet before adding",
                ctx.rel(&path)?
            ));
        }
    }
    Ok(notes)
}

/// Staleness alarm for the voice method surface: the private tree under
/// workspace/life/voice/ against its public mirror under library/schema/operator/voice/.
pub fn voice_mirror_notes(ctx: &Ctx) -> R<Vec<String>> {
    let mut notes = Vec::new();
    let live_root = join_all(&[&ctx.root, "workspace", "life", "voice"]);
    let schema_root = join_all(&[&ctx.root, "library", "schema", "operator", "voice"]);
    if !(pyos::isdir(&live_root) && pyos::isdir(&schema_root)) {
        return Ok(notes);
    }
    let rel_live = |p: &str| -> R<String> { Ok(pyos::relpath(p, &live_root)?.replace('\\', "/")) };
    let mut method: Vec<String> = pyglob::glob(&join(&live_root, "*.md"), false).iter().map(|p| pyos::basename(p)).collect();
    method.sort();
    let mut readmes = Vec::new();
    for p in pyglob::glob(&join_all(&[&live_root, "*", "README.md"]), false) {
        readmes.push(rel_live(&p)?);
    }
    readmes.sort();
    method.extend(readmes);
    let mut shared = Vec::new();
    for p in pyglob::glob(&join_all(&[&live_root, "**", "*.md"]), true) {
        let r = pyos::relpath(&p, &live_root)?;
        if pyos::isfile(&join(&schema_root, &r)) {
            shared.push(r.replace('\\', "/"));
        }
    }
    shared.sort();
    for rel in &method {
        if !pyos::isfile(&join(&schema_root, rel)) {
            notes.push(format!(
                "voice mirror: {rel} has no mirror under library/schema/operator/voice/ — mirror the agnostic method surface or move personal content out of the method slot"
            ));
        }
    }
    for rel in &shared {
        let live = join(&live_root, rel);
        let schema = join(&schema_root, rel);
        if pyos::getmtime(&live)? > pyos::getmtime(&schema)? {
            notes.push(format!(
                "voice mirror: {rel} modified after its schema mirror — mirror the agnostic change or touch the schema file to confirm personal-only"
            ));
        }
    }
    Ok(notes)
}

fn manifest_records(path: &str) -> R<Vec<(String, Vec<(String, String)>)>> {
    let text = read(path)?;
    let starts: Vec<(usize, String)> = rx(r"(?m)^\s*-\s+path:\s*(\S.*?)\s*$")
        .captures_iter(&text)
        .map(|c| (c.get(0).unwrap().start(), py_strip(&c[1]).trim_matches(|ch| ch == '"' || ch == '\'').to_string()))
        .collect();
    let field_re = rx(r"(?m)^\s+([a-z_]+):\s*(\S.*?)\s*$");
    let mut out = Vec::new();
    for (i, (pos, asset)) in starts.iter().enumerate() {
        let end = starts.get(i + 1).map(|s| s.0).unwrap_or(text.len());
        let mut fields: Vec<(String, String)> = Vec::new();
        for c in field_re.captures_iter(&text[*pos..end]) {
            let k = c[1].to_string();
            let v = clean_value(&c[2]);
            if let Some(e) = fields.iter_mut().find(|e| e.0 == k) {
                e.1 = v;
            } else {
                fields.push((k, v));
            }
        }
        out.push((asset.clone(), fields));
    }
    Ok(out)
}

pub fn design_language_notes_at(shelf: &str) -> R<Vec<String>> {
    let mut notes = Vec::new();
    if !pyos::isdir(shelf) {
        return Ok(notes);
    }
    for entry in pyos::sorted_listdir(shelf)? {
        let base = join(shelf, &entry);
        if !pyos::isdir(&base) {
            continue;
        }
        let rel = format!("library/assets/design-languages/{entry}");
        let readme = join(&base, "README.md");
        if !pyos::isfile(&readme) {
            notes.push(format!(
                "design language: {rel} has no README.md — provenance, reuse notes, and what a new project swaps have no owner"
            ));
        }
        let manifest = join_all(&[&base, "imagery", "manifest.yaml"]);
        if !pyos::isfile(&manifest) {
            notes.push(format!(
                "design language: {rel} has no imagery/manifest.yaml — an asset with no record is not available for selection, so the pool is unusable"
            ));
        }
        let templates = join_all(&[&base, "package", "templates"]);
        if pyos::isdir(&templates) {
            if !pyos::isfile(&join(&templates, "design_spec.md")) {
                notes.push(format!(
                    "design language: {rel}/package/templates has no design_spec.md — the vendor refuses a library template without one"
                ));
            }
            if pyglob::glob(&join(&templates, "*.svg"), false).is_empty() {
                notes.push(format!(
                    "design language: {rel}/package/templates has no SVG prototypes — the roster is the half that reproduces archetypes"
                ));
            }
            if pyos::isdir(&join_all(&[&base, "package", "exports"])) {
                notes.push(format!(
                    "design language: {rel}/package/exports was promoted — exports are review evidence and stay in the capture workspace"
                ));
            }
        } else if pyos::isfile(&readme) {
            let text = read(&readme)?.to_lowercase();
            if !text.contains("reference-grade") || !rx(r"bb-\d{4}-\d{2}-\d{2}").is_match(&text) {
                notes.push(format!(
                    "design language: {rel} has no package/ and its README does not declare reference-grade with the BB-* row tracking promotion — package-less is a documented waypoint, not a second asset class"
                ));
            }
        }
        if !pyos::isfile(&manifest) {
            continue;
        }
        for (asset, fields) in manifest_records(&manifest)? {
            let target = pyos::normpath(&join_all(&[&base, "imagery", &asset]));
            if !pyos::exists(&target) {
                notes.push(format!("design language: {rel} manifest names {asset}, which is not on disk"));
            }
            let get = |k: &str| fields.iter().find(|e| e.0 == k).map(|e| e.1.as_str());
            if get("licence") == Some("unknown") && get("restriction") == Some("none") {
                notes.push(format!(
                    "design language: {rel} asset {asset} has licence: unknown but restriction: none — unknown licence stays project-scoped, never upgraded by assumption"
                ));
            }
        }
    }
    Ok(notes)
}

pub fn design_language_notes(ctx: &Ctx) -> R<Vec<String>> {
    design_language_notes_at(&join_all(&[&ctx.root, "library", "assets", "design-languages"]))
}

/// Deck artifacts inside the vendored ppt-master tree.
pub fn ppt_master_stray_issues(root: &str) -> R<Vec<String>> {
    let skill = join_all(&[root, "system", "vendor", "ppt-master"]);
    let mut issues = Vec::new();
    // The vendor's init default when run from inside system/ without --dir.
    if pyos::isdir(&join_all(&[root, "system", "projects"])) {
        issues.push(
            "system/projects: deck project staged inside system/ — move it under the calling campaign (see system/vendor/ppt-master/AGENTS.md)"
                .to_string(),
        );
    }
    if !pyos::isdir(&skill) {
        return Ok(issues);
    }
    let mut err = None;
    pyos::walk(&skill, &mut |dirpath, dirnames, filenames| {
        let rel = match pyos::relpath(dirpath, root) {
            Ok(r) => r.replace('\\', "/"),
            Err(e) => {
                err.get_or_insert(e);
                return;
            }
        };
        if let Some(i) = dirnames.iter().position(|d| d == "projects") {
            issues.push(format!(
                "{rel}/projects: deck project staged inside the vendored skill tree — move it under the calling campaign (see system/vendor/ppt-master/AGENTS.md)"
            ));
            dirnames.remove(i);
        }
        let mut files = filenames.to_vec();
        files.sort();
        for f in files {
            if f.to_lowercase().ends_with(".pptx") {
                issues.push(format!(
                    "{rel}/{f}: exported deck inside the vendored skill tree — promote it to the calling deliverable folder, then clear the stray"
                ));
            }
        }
    });
    if let Some(e) = err {
        return Err(e);
    }
    Ok(issues)
}

pub fn check_system(ctx: &Ctx) -> R<(Vec<String>, Vec<String>)> {
    let mut issues = dead_link_issues(ctx)?;
    issues.extend(ppt_master_stray_issues(&ctx.root)?);
    let mut notes = budget_notes(ctx)?;
    notes.extend(voice_mirror_notes(ctx)?);
    notes.extend(design_language_notes(ctx)?);
    notes.extend(media_shelf::media_shelf_notes(ctx));
    notes.extend(runlog::button_notes(ctx));
    let (li, ln) = crate::life::doctor_checks(ctx)?;
    issues.extend(li);
    notes.extend(ln);
    Ok((issues, notes))
}

// ------------------------------------------------------------------ the verb

/// Returns the exit code: 0 clean, 1 with issues.
pub fn cmd_doctor(ctx: &Ctx, project: Option<&str>) -> R<i32> {
    let mut dirs: Vec<String> = Vec::new();
    let project = project.filter(|p| !p.is_empty());
    match project {
        Some("pipeline") | Some("studio") => {}
        Some(p) => dirs.push(project_dir(ctx, p)?),
        None => {
            if pyos::isdir(&ctx.projects) {
                for d in pyos::sorted_listdir(&ctx.projects)? {
                    if d != "completed" && pyos::isfile(&join_all(&[&ctx.projects, &d, "project.md"])) {
                        dirs.push(join(&ctx.projects, &d));
                    }
                }
            }
        }
    }
    let (mut all_issues, mut notes) = (Vec::new(), Vec::new());
    for d in &dirs {
        all_issues.extend(check_project(ctx, d)?);
        if let Some(n) = activity_size_note(ctx, d)? {
            notes.push(n);
        }
        notes.extend(media_manifest_notes(ctx, d)?);
        notes.extend(empty_head_notes(ctx, d)?);
        notes.extend(binary_head_notes(ctx, d)?);
        notes.extend(activity_notes(ctx, d)?);
    }
    let mut pipeline_scope = "";
    let mut studio_scope = "";
    if matches!(project, None | Some("pipeline")) {
        let (pi, pn) = check_pipeline(ctx)?;
        all_issues.extend(pi);
        notes.extend(pn);
        if pyos::isfile(&join(&ctx.pipeline, "pipeline.md")) {
            pipeline_scope = " + pipeline";
        }
    }
    if matches!(project, None | Some("studio")) {
        let (si, sn) = check_studio(ctx)?;
        all_issues.extend(si);
        notes.extend(sn);
        if pyos::isfile(&join(&ctx.studio, "calendar.md")) {
            studio_scope = " + studio";
        }
    }
    let mut system_scope = String::new();
    if project.is_none() {
        let (si, sn) = check_system(ctx)?;
        all_issues.extend(si);
        notes.extend(sn);
        system_scope.push_str(" + system surfaces");
    }
    system_scope.push_str(pipeline_scope);
    system_scope.push_str(studio_scope);
    for n in &notes {
        outln!("af doctor: note — {n}");
    }
    if !all_issues.is_empty() {
        outln!("af doctor: {} issue(s) — surfaced, never auto-fixed (operator decides):", all_issues.len());
        for i in &all_issues {
            outln!("  - {i}");
        }
        return Ok(1);
    }
    outln!("af doctor: {} project(s){system_scope} checked, books clean", dirs.len());
    Ok(0)
}
