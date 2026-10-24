//! `af studio`: the creator calendar board (workspace/studio/calendar.md `slots:` rows) and
//! its post folders: every `cmd_studio_*` and the slot helpers.

use crate::core::{self, now_iso, parse_iso_date, read, today, today_date, write, Ctx};
use crate::die;
use crate::error::R;
use crate::fm::{esc, join_fm, mapping_row_get, mapping_row_span, mapping_rows, mapping_span, rx, set_scalar, split_fm, sub_first, yaml_quote};
use crate::outln;
use crate::pipe::{folder_safe, illegal_transition, topology_pack};
use crate::pyglob;
use crate::pyos::{self, join, join_all};
use clap::Subcommand;

pub const STUDIO_STATES: [&str; 6] = ["planned", "captured", "cut", "scheduled", "posted", "dropped"];
pub const STUDIO_CAPTURE_DAYS: i64 = 14;

/// `STUDIO_TRANSITIONS[cur]`; an unknown state has no exits.
pub fn studio_transitions(cur: Option<&str>) -> &'static [&'static str] {
    match cur {
        Some("planned") => &["captured", "dropped"],
        Some("captured") => &["cut", "dropped"],
        Some("cut") => &["scheduled", "posted", "captured", "dropped"],
        Some("scheduled") => &["posted", "cut", "dropped"],
        _ => &[],
    }
}

fn nonempty(v: Option<&str>) -> Option<&str> {
    v.filter(|s| !s.is_empty())
}

// ------------------------------------------------------------------ argparse surface

#[derive(Subcommand, Debug)]
pub enum StudioCmd {
    New {
        slug: String,
        #[arg(long)]
        date: Option<String>,
        #[arg(long, default_value = "tiktok")]
        platform: String,
        #[arg(long)]
        series: Option<String>,
        #[arg(long = "from")]
        source: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    Stage {
        slug: String,
        state: String,
    },
    Post {
        slug: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        posted_at: Option<String>,
    },
}

pub fn dispatch(ctx: &Ctx, cmd: StudioCmd) -> R<()> {
    match cmd {
        StudioCmd::New { slug, date, platform, series, source, name } => cmd_studio_new(
            ctx,
            &NewArgs {
                slug: &slug,
                date: date.as_deref(),
                platform: &platform,
                series: series.as_deref(),
                source: source.as_deref(),
                name: name.as_deref(),
            },
        ),
        StudioCmd::Stage { slug, state } => cmd_studio_stage(ctx, &slug, &state),
        StudioCmd::Post { slug, url, posted_at } => cmd_studio_post(ctx, &slug, &url, posted_at.as_deref()),
    }
}

// ------------------------------------------------------------------ board helpers

pub fn studio_pack(ctx: &Ctx) -> R<(String, String, String)> {
    topology_pack(ctx, "studio")
}

pub fn calendar_path(ctx: &Ctx) -> String {
    join(&ctx.studio, "calendar.md")
}

pub fn posts_root(ctx: &Ctx) -> String {
    join(&ctx.studio, "posts")
}

pub fn post_dir(ctx: &Ctx, slug: &str) -> String {
    join(&posts_root(ctx), slug)
}

fn ensure_calendar(ctx: &Ctx, pack_dir: &str) -> R<()> {
    let cp = calendar_path(ctx);
    if pyos::isfile(&cp) {
        return Ok(());
    }
    let skel = join(pack_dir, "calendar-skeleton.md");
    if !pyos::isfile(&skel) {
        die!("{} ships no calendar-skeleton.md", pyos::relpath(pack_dir, &ctx.root)?);
    }
    pyos::makedirs(&posts_root(ctx))?;
    write(&cp, &core::py_format(&read(&skel)?, &[("date", &today()), ("ts", &now_iso())])?)
}

pub fn load_calendar(ctx: &Ctx) -> R<(String, String)> {
    let cp = calendar_path(ctx);
    if !pyos::isfile(&cp) {
        die!("no board at workspace/studio/calendar.md — 'af studio new' creates it");
    }
    split_fm(&read(&cp)?, "calendar.md")
}

pub fn write_calendar(ctx: &Ctx, fm: &str, body: &str) -> R<()> {
    let fm = set_scalar(fm, "last_activity", &now_iso(), "calendar.md")?;
    write(&calendar_path(ctx), &join_fm(&fm, body))
}

/// `mapping_row_set(fm, mapping, slug, key, value)`
pub fn mapping_row_set(fm: &str, mapping: &str, slug: &str, key: &str, value: &str) -> R<String> {
    let (s, e) = match mapping_row_span(fm, mapping, slug) {
        Some(sp) => sp,
        None => die!("project.md: '{slug}' not found in {mapping} block"),
    };
    let block = &fm[s..e];
    let re = rx(&format!(r"(?m)^(    {}:)[ \t]*.*$", esc(key)));
    let block = if re.is_match(block) {
        sub_first(&re, block, &format!("\\g<1> {value}"))?
    } else {
        format!("{}\n    {key}: {value}\n", block.trim_end_matches('\n'))
    };
    Ok(format!("{}{}{}", &fm[..s], block, &fm[e..]))
}

/// `mapping_row_add(fm, mapping, slug, fields)`
pub fn mapping_row_add(fm: &str, mapping: &str, slug: &str, fields: &[(&str, String)]) -> R<String> {
    if mapping_row_span(fm, mapping, slug).is_some() {
        die!("project.md: '{slug}' already exists in {mapping} block");
    }
    let mut row = format!("  {slug}:\n");
    for (k, v) in fields {
        row.push_str(&format!("    {k}: {v}\n"));
    }
    if let Some((s, e)) = mapping_span(fm, mapping) {
        let mut block = fm[s..e].to_string();
        let empty = rx(&format!(r"(?m)^{}:\s*\{{\}}\s*$", esc(mapping)));
        if empty.is_match(&block) {
            block = sub_first(&empty, &block, &format!("{mapping}:\n"))?;
        }
        let block = format!("{}\n{row}", block.trim_end_matches('\n'));
        return Ok(format!("{}{}{}", &fm[..s], block, &fm[e..]));
    }
    let section = format!("\n# {}\n{mapping}:\n{row}", mapping.to_uppercase());
    if let Some(m) = rx(r"(?m)^# COUNTERS\s*$").find(fm) {
        return Ok(format!("{}\n{section}\n{}", fm[..m.start()].trim_end(), &fm[m.start()..]));
    }
    Ok(format!("{}\n{}\n", fm.trim_end(), section.trim_end()))
}

pub fn slot_rows(fm: &str) -> Vec<String> {
    mapping_rows(fm, "slots")
}

pub fn slot_get(fm: &str, slug: &str, key: &str) -> Option<String> {
    mapping_row_get(fm, "slots", slug, key)
}

pub fn slot_set(fm: &str, slug: &str, key: &str, value: &str) -> R<String> {
    mapping_row_set(fm, "slots", slug, key, value)
}

/// `slot_add(fm, slug, fields)`: empty fields are left off the row.
pub fn slot_add(fm: &str, slug: &str, fields: &[(&str, Option<&str>)]) -> R<String> {
    let kept: Vec<(&str, String)> =
        fields.iter().filter_map(|(k, v)| nonempty(*v).map(|v| (*k, v.to_string()))).collect();
    mapping_row_add(fm, "slots", slug, &kept)
}

pub fn post_render(ctx: &Ctx, slug: &str) -> String {
    join_all(&[&post_dir(ctx, slug), "renders", "final.mp4"])
}

pub fn post_source(ctx: &Ctx, slug: &str) -> Option<String> {
    let vdir = join(&post_dir(ctx, slug), "video");
    if !pyos::isdir(&vdir) {
        return None;
    }
    pyglob::sorted_glob(&join(&vdir, "source.*"), false).into_iter().next()
}

fn touch_post(ctx: &Ctx, slug: &str) -> R<()> {
    let ppath = join(&post_dir(ctx, slug), "post.md");
    if pyos::isfile(&ppath) {
        let (pfm, pbody) = split_fm(&read(&ppath)?, "post.md")?;
        write(&ppath, &join_fm(&set_scalar(&pfm, "last_activity", &now_iso(), "post.md")?, &pbody))?;
    }
    Ok(())
}

/// Python's `str.lower()` is Unicode-aware; so is Rust's.
fn py_lower(s: &str) -> String {
    s.to_lowercase()
}

/// `shutil.copy2(src, dst)`: bytes plus the access and modification times.
fn copy2(src: &str, dst: &str) -> R<()> {
    std::fs::copy(src, dst).map_err(|e| pyos::os_error(&e, src))?;
    if let Ok(md) = std::fs::metadata(src) {
        let mut times = std::fs::FileTimes::new();
        if let Ok(m) = md.modified() {
            times = times.set_modified(m);
        }
        if let Ok(a) = md.accessed() {
            times = times.set_accessed(a);
        }
        if let Ok(f) = std::fs::OpenOptions::new().write(true).open(dst) {
            let _ = f.set_times(times);
        }
    }
    Ok(())
}

// ------------------------------------------------------------------ new

pub struct NewArgs<'a> {
    pub slug: &'a str,
    pub date: Option<&'a str>,
    pub platform: &'a str,
    pub series: Option<&'a str>,
    pub source: Option<&'a str>,
    pub name: Option<&'a str>,
}

pub fn cmd_studio_new(ctx: &Ctx, a: &NewArgs) -> R<()> {
    let (domain, _desc, pack_dir) = studio_pack(ctx)?;
    let slug = a.slug;
    if !folder_safe(slug) {
        die!("slug must be folder-safe lowercase kebab-case");
    }
    let source = nonempty(a.source);
    let date_slot = nonempty(a.date);
    if let Some(s) = source {
        if !pyos::isfile(s) {
            die!("--from file not found: {s}");
        }
    }
    if let Some(d) = date_slot {
        if parse_iso_date(Some(d)).is_none() {
            die!("--date must be YYYY-MM-DD (got {d})");
        }
    }
    if source.is_none() && date_slot.is_none() {
        die!("a post is born planned (--date D) or captured (--from FILE); an undated idea belongs in ideas.md, not on the board");
    }
    for skel in ["post-skeleton.md", "edit-skeleton.md"] {
        if !pyos::isfile(&join(&pack_dir, skel)) {
            die!("domain '{domain}' ships no {skel}");
        }
    }
    ensure_calendar(ctx, &pack_dir)?;
    let (fm, body) = load_calendar(ctx)?;
    if mapping_row_span(&fm, "slots", slug).is_some() {
        die!("board row '{slug}' already exists");
    }
    let pdir = post_dir(ctx, slug);
    if pyos::exists(&pdir) {
        die!("{} already exists", pyos::relpath(&pdir, &ctx.root)?);
    }

    let name = match nonempty(a.name) {
        Some(n) => n.to_string(),
        None => pyos::py_title(&slug.replace('-', " ")),
    };
    pyos::makedirs_new(&join(&pdir, "video"))?;
    pyos::makedirs_new(&join(&pdir, "renders"))?;
    let mut source_rel = "null".to_string();
    if let Some(s) = source {
        let ext = py_lower(&pyos::splitext(s).1);
        let ext = if ext.is_empty() { ".mp4".to_string() } else { ext };
        copy2(s, &join_all(&[&pdir, "video", &format!("source{ext}")]))?;
        source_rel = format!("video/source{ext}");
    }
    let post = core::py_format(
        &read(&join(&pack_dir, "post-skeleton.md"))?,
        &[
            ("name", &name),
            ("slug", slug),
            ("date", &today()),
            ("ts", &now_iso()),
            ("platform", a.platform),
            ("date_slot", date_slot.unwrap_or("null")),
            ("series", nonempty(a.series).unwrap_or("null")),
            ("source_file", &source_rel),
        ],
    )?;
    write(&join(&pdir, "post.md"), &post)?;
    let edit = core::py_format(&read(&join(&pack_dir, "edit-skeleton.md"))?, &[("name", &name)])?;
    write(&join(&pdir, "edit.md"), &edit)?;

    let state = if source.is_some() { "captured" } else { "planned" };
    let created = today();
    let fm = slot_add(
        &fm,
        slug,
        &[
            ("state", Some(state)),
            ("date", date_slot),
            ("platform", Some(a.platform)),
            ("series", a.series),
            ("created", Some(&created)),
        ],
    )?;
    write_calendar(ctx, &fm, &body)?;

    let copied = if source.is_some() { format!(" ({source_rel} copied in)") } else { String::new() };
    outln!("af studio new: workspace/studio/posts/{slug}/ scaffolded -> {state}{copied}");
    outln!("\nJudgment (stays with the agent):");
    outln!("  - Load the runbook: library/domains/{domain}/production.md (load map + the post steps).");
    if source.is_some() {
        outln!("  - Next is the edit route: transcribe, cut, fill edit.md, compose, render, then 'af studio stage cut'.");
    } else {
        outln!("  - Planned: draft script.md against the generic shape only if the piece is scripted; when the");
        outln!("    recording lands as video/source.*, 'af studio stage captured' moves it on.");
    }
    Ok(())
}

// ------------------------------------------------------------------ stage

pub fn cmd_studio_stage(ctx: &Ctx, slug: &str, new: &str) -> R<()> {
    let (fm, body) = load_calendar(ctx)?;
    if mapping_row_span(&fm, "slots", slug).is_none() {
        die!("no board row '{slug}'");
    }
    if !STUDIO_STATES.contains(&new) {
        die!("unknown state '{new}' (states: {})", STUDIO_STATES.join(", "));
    }
    if new == "posted" {
        die!("'posted' is written by 'af studio post <slug> --url U' — the receipt is the transition");
    }
    let cur = slot_get(&fm, slug, "state");
    let legal = studio_transitions(cur.as_deref());
    if !legal.contains(&new) {
        die!("{}", illegal_transition(&cur, new, legal));
    }
    if new == "captured" && post_source(ctx, slug).is_none() {
        die!("'{slug}' has no video/source.* — land the recording first (or 'af studio new --from')");
    }
    if (new == "cut" || new == "scheduled") && !pyos::isfile(&post_render(ctx, slug)) {
        die!("'{slug}' has no renders/final.mp4 — '{new}' means a render exists");
    }
    let fm = slot_set(&fm, slug, "state", new)?;
    write_calendar(ctx, &fm, &body)?;
    touch_post(ctx, slug)?;
    outln!("af studio stage: {slug} {} -> {new}", core::py_opt(&cur));
    outln!("\nJudgment (stays with the agent):");
    if new == "cut" {
        outln!("  - The operator watches renders/final.mp4 before anything is scheduled.");
    }
    if new == "scheduled" {
        outln!("  - Mirror the slot to the operator's calendar (production.md standing rules); the board stays truth.");
        outln!("  - When it goes live: 'af studio post {slug} --url <link>'.");
    }
    if new == "dropped" {
        outln!("  - Anything reusable in video/? Register it on the media shelf by path (media-intake.md).");
    }
    Ok(())
}

// ------------------------------------------------------------------ post

pub fn cmd_studio_post(ctx: &Ctx, slug: &str, url: &str, posted_at: Option<&str>) -> R<()> {
    let (fm, body) = load_calendar(ctx)?;
    if mapping_row_span(&fm, "slots", slug).is_none() {
        die!("no board row '{slug}'");
    }
    let cur = slot_get(&fm, slug, "state");
    if !matches!(cur.as_deref(), Some("cut") | Some("scheduled")) {
        die!("'{slug}' is at '{}' — post applies to cut or scheduled rows (a render exists)", core::py_opt(&cur));
    }
    let posted_at = match nonempty(posted_at) {
        Some(p) => p.to_string(),
        None => now_iso(),
    };
    let ppath = join(&post_dir(ctx, slug), "post.md");
    if !pyos::isfile(&ppath) {
        die!("no post.md for '{slug}'");
    }
    let (pfm, pbody) = split_fm(&read(&ppath)?, "post.md")?;
    let pfm = set_scalar(&pfm, "posted_at", &posted_at, "post.md")?;
    let pfm = set_scalar(&pfm, "url", &yaml_quote(Some(url)), "post.md")?;
    let pfm = set_scalar(&pfm, "last_activity", &now_iso(), "post.md")?;
    write(&ppath, &join_fm(&pfm, &pbody))?;
    let day = pyos::py_prefix(&posted_at, 10);
    let fm = slot_set(&fm, slug, "state", "posted")?;
    let fm = slot_set(&fm, slug, "link", url)?;
    let fm = slot_set(&fm, slug, "posted", &day)?;
    write_calendar(ctx, &fm, &body)?;
    let due = parse_iso_date(Some(&day)).unwrap_or_else(today_date) + chrono::Duration::days(STUDIO_CAPTURE_DAYS);
    outln!("af studio post: {slug} {} -> posted ({url})", core::py_opt(&cur));
    outln!("\nJudgment (stays with the agent):");
    outln!(
        "  - Performance capture is due around {}; doctor nudges. Stamp metrics_captured_at when done.",
        due.format("%Y-%m-%d")
    );
    outln!("  - Score the hook row in hooks.md once the numbers land; next week's hooks are proposed from it.");
    outln!("  - Mirror the posted slot to the operator's calendar if the scheduled mirror was never written.");
    Ok(())
}
