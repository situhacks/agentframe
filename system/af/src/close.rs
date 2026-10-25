//! `af close <slug>`: project closeout mechanics. Moves the folder to completed/, stamps
//! the lifecycle, logs the event, and prints the harvest checklist. Harvests stay in chat.

use crate::core::{append_activity, today, touch_lifecycle, Ctx};
use crate::die;
use crate::error::R;
use crate::fm::{self, all_rows, get_scalar, row_get, set_scalar, split_fm};
use crate::life::{self, TextFile};
use crate::outln;
use crate::pyos::{self, join};

/// Published rows and the harvests their activity lines do not yet record:
/// (row, file, missing harvest skills).
pub fn unharvested(cfm: &str, activity: &str) -> Vec<(String, String, Vec<&'static str>)> {
    let mut out = Vec::new();
    for slug in all_rows(cfm) {
        if row_get(cfm, &slug, "status").as_deref() != Some("published") {
            continue;
        }
        let file = row_get(cfm, &slug, "file").unwrap_or_default();
        let mut missing = Vec::new();
        for (skill, keys) in [
            ("deliverable-harvest", ["deliverable-harvest", "deliverable_harvest"]),
            ("voice-harvest", ["voice-harvest", "voice_harvest"]),
        ] {
            let done = activity.lines().any(|l| {
                keys.iter().any(|k| l.contains(k)) && (l.contains(slug.as_str()) || (!file.is_empty() && l.contains(file.as_str())))
            });
            if !done {
                missing.push(skill);
            }
        }
        if !missing.is_empty() {
            out.push((slug.clone(), file, missing));
        }
    }
    out
}

/// Put `completed_at` right after the top-level `status:` line. A project closed earlier
/// by hand (`--force` on a `complete` row) keeps the date it already carries.
fn stamp_completed(cfm: &str, date: &str) -> R<String> {
    if fm::has_field(cfm, "completed_at") {
        if get_scalar(cfm, "completed_at").map(|v| !v.trim().is_empty()).unwrap_or(false) {
            return Ok(cfm.to_string());
        }
        return set_scalar(cfm, "completed_at", date, "project.md");
    }
    let re = fm::rx(r"(?m)^status:.*$");
    match re.find(cfm) {
        Some(m) => Ok(format!("{}\ncompleted_at: {date}{}", &cfm[..m.end()], &cfm[m.end()..])),
        None => die!("project.md: field 'status' not found — fix the file or the schema first"),
    }
}

pub fn cmd_close(ctx: &Ctx, slug: &str, force: bool) -> R<i32> {
    if !fm::valid_slug(slug) {
        die!("project slug must contain only letters, numbers, underscores, and hyphens");
    }
    let src = join(&ctx.projects, slug);
    let completed = join(&ctx.projects, "completed");
    let dst = join(&completed, slug);
    if !pyos::isfile(&join(&src, "project.md")) {
        if pyos::isfile(&join(&dst, "project.md")) {
            die!("project '{slug}' is already closed (workspace/projects/completed/{slug}/)");
        }
        die!("project '{slug}' not found under workspace/projects/");
    }
    if pyos::exists(&dst) {
        die!("workspace/projects/completed/{slug}/ already exists; resolve it before closing");
    }
    let cpath = join(&src, "project.md");
    let tf = TextFile::load(&cpath)?.unwrap_or_else(|| TextFile::new(""));
    let (cfm, _) = split_fm(&tf.text, "project.md")?;
    let status = get_scalar(&cfm, "status");
    if status.as_deref() != Some("active") && !force {
        die!("project '{slug}' is '{}', not active; --force closes it anyway", status.as_deref().unwrap_or("missing"));
    }
    let drafting: Vec<String> =
        all_rows(&cfm).into_iter().filter(|r| row_get(&cfm, r, "status").as_deref() == Some("drafting")).collect();
    if !drafting.is_empty() && !force {
        die!("project '{slug}' has drafting deliverables: {}; finish or defer them, or --force", drafting.join(", "));
    }

    pyos::makedirs(&completed)?;
    std::fs::rename(&src, &dst).map_err(|e| pyos::os_error(&e, &src))?;

    let cpath = join(&dst, "project.md");
    let mut tf = TextFile::load(&cpath)?.unwrap_or_else(|| TextFile::new(""));
    let (cfm, body) = split_fm(&tf.text, "project.md")?;
    let cfm = set_scalar(&cfm, "status", "complete", "project.md")?;
    let cfm = stamp_completed(&cfm, &today())?;
    let cfm = if fm::has_field(&cfm, "last_activity") { touch_lifecycle(&cfm)? } else { cfm };
    tf.text = fm::join_fm(&cfm, &body);
    tf.save(&cpath)?;
    append_activity(&dst, &format!("project_closed: {slug}; folder moved to completed/"))?;
    let log = life::append_log(ctx, "project_closed", slug)?;

    let activity = TextFile::load(&join(&dst, "activity.md"))?.map(|t| t.text).unwrap_or_default();
    outln!("af close: {slug} -> complete (workspace/projects/completed/{slug}/; logged in {log})");
    outln!("\nHarvest checklist (the chat runs these; af never does):");
    let todo = unharvested(&cfm, &activity);
    if todo.is_empty() {
        outln!("  (every published head already shows its harvests in activity.md)");
    }
    for (row, file, missing) in &todo {
        outln!("  [ ] {row} ({file}): {}", missing.join(", "));
    }
    outln!("  [ ] career-harvest if the project produced a bankable win (library/process/career-harvest.md)");
    outln!("  [ ] Promote durable knowledge/ entries to workspace/life/ (library/process/project-flow.md, Closeout step 2)");
    Ok(0)
}
