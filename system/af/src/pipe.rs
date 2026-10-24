//! `af pipe`: the careers pipeline board (workspace/pipeline/pipeline.md) and its
//! application folders. Port of af.py `cmd_pipe_*`, `topology_pack`, and the board helpers.

use crate::core::{self, append_activity, load_pack, now_iso, read, today, today_date, write, Ctx};
use crate::die;
use crate::error::{crash, CrashKind, R};
use crate::fm::{get_scalar, join_fm, pipe_rows, row_get, row_set, row_span, rx, set_scalar, split_fm, sub_first, upsert_scalar, yaml_quote};
use crate::outln;
use crate::pyos::{self, join, join_all, py_split_ws, py_strip};
use clap::Subcommand;

pub const PIPE_STAGES: [&str; 10] =
    ["saved", "preparing", "applied", "interviewing", "offer", "accepted", "declined", "rejected", "ghosted", "dropped"];
pub const PIPE_TERMINAL: [&str; 5] = ["accepted", "declined", "rejected", "ghosted", "dropped"];
pub const PIPE_NUDGE_DAYS: i64 = 7;
pub const APPS_ARCHIVE: &str = "completed";
pub const ROUND_TYPES: [&str; 6] = ["recruiter-screen", "hiring-manager", "panel", "technical", "partner", "client"];

/// `PIPE_TRANSITIONS[cur]`; an unknown stage has no exits.
pub fn pipe_transitions(cur: Option<&str>) -> &'static [&'static str] {
    match cur {
        Some("saved") => &["preparing", "dropped"],
        Some("preparing") => &["applied", "interviewing", "rejected", "ghosted", "dropped"],
        Some("applied") => &["interviewing", "rejected", "ghosted", "dropped"],
        Some("interviewing") => &["offer", "rejected", "ghosted", "dropped"],
        Some("ghosted") => &["interviewing", "rejected", "dropped"],
        Some("offer") => &["accepted", "declined", "rejected"],
        _ => &[],
    }
}

/// The refusal for a move the state machine does not allow (shared with studio).
pub fn illegal_transition(cur: &Option<String>, new: &str, legal: &[&str]) -> String {
    let cur = core::py_opt(cur);
    if legal.is_empty() {
        format!("illegal transition {cur} -> {new} ({cur} is terminal)")
    } else {
        let mut sorted: Vec<&str> = legal.to_vec();
        sorted.sort();
        format!("illegal transition {cur} -> {new} (legal from {cur}: {})", sorted.join(", "))
    }
}

/// `re.match(r"^[a-z0-9][a-z0-9-]*$", s)`
pub fn folder_safe(s: &str) -> bool {
    rx(r"\A[a-z0-9][a-z0-9-]*\n?\z").is_match(s)
}

fn nonempty(v: Option<&str>) -> Option<&str> {
    v.filter(|s| !s.is_empty())
}

/// argparse `type=int`: Python `int(str)` over ASCII digits (whitespace, sign, single underscores).
pub fn parse_py_int(s: &str) -> Result<i128, String> {
    let bad = || format!("invalid int value: {}", pyos::py_repr_str(s));
    let t = py_strip(s);
    let (neg, digits) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    if digits.is_empty() || digits.starts_with('_') || digits.ends_with('_') || digits.contains("__") {
        return Err(bad());
    }
    if !digits.chars().all(|c| c.is_ascii_digit() || c == '_') {
        return Err(bad());
    }
    let v: i128 = digits.replace('_', "").parse().map_err(|_| bad())?;
    Ok(if neg { -v } else { v })
}

// ------------------------------------------------------------------ argparse surface

#[derive(Subcommand, Debug)]
pub enum PipeCmd {
    Save {
        #[arg(long)]
        company: String,
        #[arg(long)]
        role: String,
        #[arg(long)]
        url: String,
        #[arg(long, default_value = "unknown")]
        ats: String,
        #[arg(long, default_value = "manual")]
        source: String,
        #[arg(long)]
        posted: Option<String>,
        #[arg(long)]
        deadline: Option<String>,
        #[arg(long)]
        salary: Option<String>,
        #[arg(long)]
        slug: Option<String>,
    },
    Start {
        slug: String,
    },
    Stage {
        slug: String,
        stage: String,
        /// the title actually offered, when it differs from the posting (offer, accepted, declined)
        #[arg(long)]
        offered_role: Option<String>,
    },
    #[command(name = "close-search")]
    CloseSearch {
        #[arg(long)]
        reason: String,
    },
    Board,
    Round {
        slug: String,
        #[arg(allow_negative_numbers = true, value_parser = parse_py_int)]
        number: i128,
        #[arg(long = "with")]
        person: Vec<String>,
        #[arg(long = "type", value_parser = ROUND_TYPES)]
        round_type: Option<String>,
        #[arg(long)]
        at: Option<String>,
        #[arg(long)]
        folder: Option<String>,
    },
    Archive {
        slug: String,
    },
    Unarchive {
        slug: String,
    },
}

pub fn dispatch(ctx: &Ctx, cmd: PipeCmd) -> R<()> {
    match cmd {
        PipeCmd::Save { company, role, url, ats, source, posted, deadline, salary, slug } => cmd_pipe_save(
            ctx,
            &SaveArgs {
                company: &company,
                role: &role,
                url: &url,
                ats: &ats,
                source: &source,
                posted: posted.as_deref(),
                deadline: deadline.as_deref(),
                salary: salary.as_deref(),
                slug: slug.as_deref(),
            },
        ),
        PipeCmd::Start { slug } => cmd_pipe_start(ctx, &slug),
        PipeCmd::Stage { slug, stage, offered_role } => cmd_pipe_stage(ctx, &slug, &stage, offered_role.as_deref()),
        PipeCmd::CloseSearch { reason } => cmd_pipe_close_search(ctx, Some(&reason)),
        PipeCmd::Board => cmd_pipe_board(ctx),
        PipeCmd::Round { slug, number, person, round_type, at, folder } => cmd_pipe_round(
            ctx,
            &RoundArgs { slug: &slug, number, person: &person, round_type: round_type.as_deref(), at: at.as_deref(), folder: folder.as_deref() },
        ),
        PipeCmd::Archive { slug } => cmd_pipe_archive(ctx, &slug),
        PipeCmd::Unarchive { slug } => cmd_pipe_unarchive(ctx, &slug),
    }
}

// ------------------------------------------------------------------ topology and board

/// The single pack declaring `topology: {kind}`: (domain, descriptor frontmatter, pack dir).
pub fn topology_pack(ctx: &Ctx, kind: &str) -> R<(String, String, String)> {
    if !pyos::isdir(&ctx.domains) {
        die!("library/domains/ missing");
    }
    let mut hits = Vec::new();
    for name in pyos::sorted_listdir(&ctx.domains)? {
        let (desc, pack_dir) = load_pack(ctx, Some(&name))?;
        if let Some(d) = desc {
            if !d.is_empty() && get_scalar(&d, "topology").as_deref() == Some(kind) {
                hits.push((name, d, pack_dir.unwrap_or_default()));
            }
        }
    }
    let verb = match kind {
        "pipeline" => "af pipe".to_string(),
        "studio" => "af studio".to_string(),
        k => format!("the {k} verbs"),
    };
    if hits.is_empty() {
        die!("no domain pack declares topology: {kind} — author one before using '{verb}'");
    }
    if hits.len() > 1 {
        let names: Vec<&str> = hits.iter().map(|h| h.0.as_str()).collect();
        die!("multiple {kind}-topology packs ({}) — the singleton board can serve only one", names.join(", "));
    }
    Ok(hits.remove(0))
}

pub fn pipe_pack(ctx: &Ctx) -> R<(String, String, String)> {
    topology_pack(ctx, "pipeline")
}

pub fn board_path(ctx: &Ctx) -> String {
    join(&ctx.pipeline, "pipeline.md")
}

fn ensure_board(ctx: &Ctx, pack_dir: &str) -> R<()> {
    let bp = board_path(ctx);
    if pyos::isfile(&bp) {
        return Ok(());
    }
    let skel = join(pack_dir, "pipeline-skeleton.md");
    if !pyos::isfile(&skel) {
        die!("{} ships no pipeline-skeleton.md", pyos::relpath(pack_dir, &ctx.root)?);
    }
    pyos::makedirs(&ctx.pipeline)?;
    write(&bp, &core::py_format(&read(&skel)?, &[("date", &today()), ("ts", &now_iso())])?)
}

pub fn load_board(ctx: &Ctx) -> R<(String, String)> {
    let bp = board_path(ctx);
    if !pyos::isfile(&bp) {
        die!("no board at workspace/pipeline/pipeline.md — 'af pipe save' creates it");
    }
    split_fm(&read(&bp)?, "pipeline.md")
}

pub fn write_board(ctx: &Ctx, fm: &str, body: &str) -> R<()> {
    let fm = set_scalar(fm, "last_activity", &now_iso(), "pipeline.md")?;
    write(&board_path(ctx), &join_fm(&fm, body))
}

/// `pipe_row_add(fm, slug, fields)`: append a row to `applications:`, skipping empty fields.
pub fn pipe_row_add(fm: &str, slug: &str, fields: &[(&str, Option<String>)]) -> R<String> {
    let fm = sub_first(&rx(r"(?m)^applications:\s*\{\}\s*$"), fm, "applications:")?;
    let m = match rx(r"(?m)^applications:\s*$").find(&fm) {
        None => die!("pipeline.md has no applications block"),
        Some(m) => m,
    };
    let rest = &fm[m.end()..];
    let end = m.end() + rx(r"(?m)^\S").find(rest).map(|n| n.start()).unwrap_or(rest.len());
    let mut row = format!("  {slug}:\n");
    for (k, v) in fields {
        if let Some(v) = v {
            if !v.is_empty() {
                row.push_str(&format!("    {k}: {v}\n"));
            }
        }
    }
    let mut head = fm[..end].to_string();
    if !head.ends_with('\n') {
        head.push('\n');
    }
    Ok(format!("{head}{row}{}", &fm[end..]))
}

fn yaml_str(v: Option<&str>) -> Option<String> {
    nonempty(v).map(|s| format!("\"{s}\""))
}

/// `app_materials(afm)`: the submission rows; resume-only when `materials` is absent.
pub fn app_materials(afm: &str) -> Vec<String> {
    let m = crate::fm::fm_list(afm, "materials");
    if m.is_empty() {
        vec!["resume".to_string()]
    } else {
        m
    }
}

pub fn apps_root(ctx: &Ctx) -> String {
    join(&ctx.pipeline, "applications")
}

/// Active application folder, falling back to the archived one once the row is retired.
pub fn app_dir(ctx: &Ctx, slug: &str) -> String {
    let active = join(&apps_root(ctx), slug);
    let archived = join_all(&[&apps_root(ctx), APPS_ARCHIVE, slug]);
    if !pyos::isdir(&active) && pyos::isdir(&archived) {
        return archived;
    }
    active
}

pub fn app_is_archived(ctx: &Ctx, slug: &str) -> bool {
    pyos::isdir(&join_all(&[&apps_root(ctx), APPS_ARCHIVE, slug]))
}

pub fn jd_cache_path(ctx: &Ctx, slug: &str) -> String {
    join_all(&[&ctx.pipeline, "scout", "jd-cache", &format!("{slug}.jd.md")])
}

/// `shutil.move(src, dst)` for a destination that does not exist.
pub fn move_path(src: &str, dst: &str) -> R<()> {
    std::fs::rename(src, dst).map_err(|e| pyos::os_error(&e, src))
}

fn nudge_date() -> String {
    (today_date() + chrono::Duration::days(PIPE_NUDGE_DAYS)).format("%Y-%m-%d").to_string()
}

// ------------------------------------------------------------------ save

pub struct SaveArgs<'a> {
    pub company: &'a str,
    pub role: &'a str,
    pub url: &'a str,
    pub ats: &'a str,
    pub source: &'a str,
    pub posted: Option<&'a str>,
    pub deadline: Option<&'a str>,
    pub salary: Option<&'a str>,
    pub slug: Option<&'a str>,
}

pub fn cmd_pipe_save(ctx: &Ctx, a: &SaveArgs) -> R<()> {
    let (_domain, _desc, pack_dir) = pipe_pack(ctx)?;
    ensure_board(ctx, &pack_dir)?;
    let slug = match nonempty(a.slug) {
        Some(s) => s.to_string(),
        None => {
            let low = format!("{}-{}", a.company, a.role).to_lowercase();
            rx(r"[^a-z0-9]+").replace_all(&low, "-").trim_matches('-').to_string()
        }
    };
    if !folder_safe(&slug) {
        die!("derived slug '{slug}' is not folder-safe — pass --slug");
    }
    let (fm, body) = load_board(ctx)?;
    if row_span(&fm, &slug).is_some() {
        die!("board row '{slug}' already exists");
    }
    let mut fm = pipe_row_add(
        &fm,
        &slug,
        &[
            ("stage", Some("saved".to_string())),
            ("company", yaml_str(Some(a.company))),
            ("role", yaml_str(Some(a.role))),
            ("url", Some(a.url.to_string())),
            ("ats", Some(a.ats.to_string())),
            ("source", Some(a.source.to_string())),
            ("posted", a.posted.map(String::from)),
            ("deadline", a.deadline.map(String::from)),
            ("salary", yaml_str(a.salary)),
            ("saved", Some(today())),
        ],
    )?;
    let mut reopened = String::new();
    if get_scalar(&fm, "search_status").as_deref() == Some("closed") {
        fm = set_scalar(&fm, "search_status", "open", "pipeline.md")?;
        reopened = format!(
            "; search reopened (closed {}: {})",
            core::py_opt(&get_scalar(&fm, "search_closed_at")),
            core::py_opt(&get_scalar(&fm, "search_closed_reason"))
        );
    }
    write_board(ctx, &fm, &body)?;
    outln!("af pipe save: {slug} -> saved (board row; no folder until start){reopened}");
    outln!("\nJudgment (stays with the agent):");
    outln!("  - Cache the verbatim JD at workspace/pipeline/scout/jd-cache/{slug}.jd.md now — postings vanish.");
    outln!("  - Committing to it? 'af pipe start {slug}' scaffolds the sprint folder.");
    Ok(())
}

// ------------------------------------------------------------------ start

pub fn cmd_pipe_start(ctx: &Ctx, slug: &str) -> R<()> {
    let (domain, _desc, pack_dir) = pipe_pack(ctx)?;
    let (fm, body) = load_board(ctx)?;
    if row_span(&fm, slug).is_none() {
        die!("no board row '{slug}' — 'af pipe save' it first");
    }
    let cur = row_get(&fm, slug, "stage");
    if cur.as_deref() != Some("saved") {
        die!("'{slug}' is at stage '{}' — start applies to 'saved' rows only", core::py_opt(&cur));
    }
    if app_is_archived(ctx, slug) {
        die!("'{slug}' is archived - 'af pipe unarchive {slug}' to reopen it");
    }
    let adir = join(&apps_root(ctx), slug);
    if pyos::exists(&adir) {
        die!("{adir} already exists");
    }
    let skel = join(&pack_dir, "skeleton.md");
    if !pyos::isfile(&skel) {
        die!("domain '{domain}' ships no skeleton.md");
    }

    let field = |key: &str, default: &str| -> String {
        match row_get(&fm, slug, key) {
            Some(v) if !v.is_empty() => v,
            _ => default.to_string(),
        }
    };
    let company = field("company", "?");
    let role = field("role", "?");
    let name = format!("{company} - {role}");
    pyos::makedirs_new(&adir)?;
    let text = core::py_format(
        &read(&skel)?,
        &[
            ("name", &name),
            ("slug", slug),
            ("date", &today()),
            ("ts", &now_iso()),
            ("company", &company),
            ("role", &role),
            ("url", &field("url", "null")),
            ("source", &field("source", "manual")),
            ("ats", &field("ats", "unknown")),
            ("posted", &field("posted", "null")),
            ("salary", &field("salary", "")),
        ],
    )?;
    write(&join(&adir, "application.md"), &text)?;

    let mut jd_note = "no cached JD found — capture jd.md verbatim before mapping";
    let cache = jd_cache_path(ctx, slug);
    if pyos::isfile(&cache) {
        move_path(&cache, &join(&adir, "jd.md"))?;
        jd_note = "cached JD moved in as jd.md";
    }
    for sub in ["sources", "correspondence", "people"] {
        pyos::makedirs(&join(&adir, sub))?;
    }
    let src_skel = join(&pack_dir, "sources-skeleton.md");
    if pyos::isfile(&src_skel) {
        let idx = core::py_format(&read(&src_skel)?, &[("name", &name), ("date", &today())])?;
        write(&join_all(&[&adir, "sources", "INDEX.md"]), &idx)?;
    }

    let fm = row_set(&fm, slug, "stage", "preparing")?;
    write_board(ctx, &fm, &body)?;
    append_activity(&adir, &format!("application_started: {name} scaffolded ({domain}); {jd_note}"))?;

    outln!("af pipe start: workspace/pipeline/applications/{slug}/ scaffolded ({jd_note})");
    outln!("\nJudgment (stays with the agent):");
    outln!("  - Load the runbook: library/domains/{domain}/production.md (brief -> map -> tailor -> verify -> export).");
    outln!("  - The jd-map's gap stop and coverage choice are operator conversations, not drafting problems.");
    outln!("  - Every raw input (recording, recruiter mail, second JD) lands in sources/ with an INDEX.md row.");
    Ok(())
}

// ------------------------------------------------------------------ stage

/// Record which head actually went out (`shipped`), from the primary material's readiness.
fn stamp_shipped(ctx: &Ctx, fm: String, slug: &str, notes: &mut Vec<String>, sent_to: &str) -> R<String> {
    let ap = join(&app_dir(ctx, slug), "application.md");
    if !pyos::isfile(&ap) {
        return Ok(fm);
    }
    let (afm, _) = split_fm(&read(&ap)?, "application.md")?;
    let mats = app_materials(&afm);
    let rel = row_get(&afm, &mats[0], "file");
    let st = row_get(&afm, &mats[0], "status");
    let ready = |s: &Option<String>| matches!(s.as_deref(), Some("ready") | Some("published"));
    let mut fm = fm;
    let n = rx(r"-v(\d+)\.md\n?\z").captures(rel.as_deref().unwrap_or("")).map(|c| c[1].to_string());
    match n {
        Some(n) if ready(&st) => fm = row_set(&fm, slug, "shipped", &format!("v{n}"))?,
        _ => notes.push(format!(
            "primary material '{}' is '{}' — shipped left unset; it went to {sent_to} unverified (af ready gates it before the next send)",
            mats[0],
            core::py_opt(&st)
        )),
    }
    for mat in &mats[1..] {
        let mst = row_get(&afm, mat, "status");
        if !ready(&mst) {
            notes.push(format!("material '{mat}' is '{}'", core::py_opt(&mst)));
        }
    }
    Ok(fm)
}

pub fn cmd_pipe_stage(ctx: &Ctx, slug: &str, new: &str, offered_role: Option<&str>) -> R<()> {
    let (fm, body) = load_board(ctx)?;
    if row_span(&fm, slug).is_none() {
        die!("no board row '{slug}'");
    }
    if !PIPE_STAGES.contains(&new) {
        die!("unknown stage '{new}' (stages: {})", PIPE_STAGES.join(", "));
    }
    let cur = row_get(&fm, slug, "stage");
    let legal = pipe_transitions(cur.as_deref());
    if !legal.contains(&new) {
        die!("{}", illegal_transition(&cur, new, legal));
    }
    let offered_role = nonempty(offered_role);
    if offered_role.is_some() && !["offer", "accepted", "declined"].contains(&new) {
        die!("--offered-role records the title actually offered; it applies at offer, accepted, or declined");
    }

    let mut notes: Vec<String> = Vec::new();
    let mut fm = row_set(&fm, slug, "stage", new)?;
    if let Some(r) = offered_role {
        fm = row_set(&fm, slug, "offered_role", &yaml_quote(Some(r)))?;
        notes.push(format!("offered_role recorded: {r}"));
    }
    if new == "applied" {
        fm = row_set(&fm, slug, "applied", &today())?;
        fm = row_set(&fm, slug, "next_nudge", &nudge_date())?;
        fm = stamp_shipped(ctx, fm, slug, &mut notes, "the ATS")?;
    } else if new == "interviewing" {
        fm = row_set(&fm, slug, "next_nudge", &nudge_date())?;
        if cur.as_deref() == Some("preparing") {
            notes.push("skipped 'applied' - inbound/agency-submitted; record `submitted_by:` in application.md".to_string());
            fm = stamp_shipped(ctx, fm, slug, &mut notes, "the recruiter")?;
        }
    } else {
        fm = row_set(&fm, slug, "next_nudge", "null")?;
    }
    write_board(ctx, &fm, &body)?;
    let cur_s = core::py_opt(&cur);
    let tail = |sep: &str| if notes.is_empty() { String::new() } else { format!("{sep}{}", notes.join("; ")) };
    let adir = app_dir(ctx, slug);
    if pyos::isdir(&adir) {
        append_activity(&adir, &format!("stage: {cur_s} -> {new}{}", tail("; ")))?;
    }

    let shown = if notes.is_empty() { String::new() } else { format!(" ({})", notes.join("; ")) };
    outln!("af pipe stage: {slug} {cur_s} -> {new}{shown}");
    outln!("\nJudgment (stays with the agent):");
    if new == "applied" {
        outln!("  - Note the submission channel in application.md (direct career site beats boards for ranking).");
    }
    if ["offer", "accepted", "declined", "rejected"].contains(&new) {
        outln!("  - Anything worth banking? Run career-harvest while the evidence is fresh (library/process/career-harvest.md).");
    }
    if new == "offer" {
        outln!("  - comp-case.md before the first comp call; --offered-role here when the title differs from the posting.");
    }
    if new == "accepted" {
        outln!("  - If this ends the search: af pipe close-search --reason \"...\" drops the live rows and stamps the board.");
    }
    if new == "declined" {
        outln!("  - Record why in activity.md; comp-case.md keeps the numbers the decision rested on.");
    }
    if new == "interviewing" {
        outln!("  - Prep from the jd-map + stories, not the resume; refresh company-brief '## Now' if it is >30 days old.");
    }
    Ok(())
}

// ------------------------------------------------------------------ close-search

pub fn cmd_pipe_close_search(ctx: &Ctx, reason: Option<&str>) -> R<()> {
    pipe_pack(ctx)?;
    let (fm, body) = load_board(ctx)?;
    let reason = py_strip(reason.unwrap_or("")).to_string();
    if reason.is_empty() {
        die!("--reason is required: the one line every dropped row and the board carry");
    }
    if get_scalar(&fm, "search_status").as_deref() == Some("closed") {
        die!(
            "search already closed {} ({}); the next 'af pipe save' reopens it",
            core::py_opt(&get_scalar(&fm, "search_closed_at")),
            core::py_opt(&get_scalar(&fm, "search_closed_reason"))
        );
    }
    let mut fm = fm;
    let mut dropped = Vec::new();
    for slug in pipe_rows(&fm) {
        let stage = row_get(&fm, &slug, "stage");
        if matches!(stage.as_deref(), Some(s) if PIPE_TERMINAL.contains(&s)) {
            continue;
        }
        fm = row_set(&fm, &slug, "stage", "dropped")?;
        fm = row_set(&fm, &slug, "next_nudge", "null")?;
        let stage_s = core::py_opt(&stage);
        let adir = app_dir(ctx, &slug);
        if pyos::isdir(&adir) {
            append_activity(&adir, &format!("stage: {stage_s} -> dropped; search closed: {reason}"))?;
        }
        dropped.push(format!("{slug} ({stage_s})"));
    }
    let fm = upsert_scalar(&fm, "search_status", "closed", Some("applications"))?;
    let fm = upsert_scalar(&fm, "search_closed_at", &today(), Some("applications"))?;
    let fm = upsert_scalar(&fm, "search_closed_reason", &yaml_quote(Some(&reason)), Some("applications"))?;
    write_board(ctx, &fm, &body)?;
    let tail = if dropped.is_empty() { "; no live rows to drop".to_string() } else { format!("; dropped {}", dropped.join(", ")) };
    outln!("af pipe close-search: board closed ({reason}){tail}");
    outln!("\nJudgment (stays with the agent):");
    outln!("  - Update the search status line in search-profile.md (workspace/life/career/) to match.");
    outln!("  - Archive the terminal rows (af pipe archive <slug>) after career-harvest on anything worth banking.");
    outln!("  - The next 'af pipe save' reopens the search; note in that row why, if it is deliberate.");
    Ok(())
}

// ------------------------------------------------------------------ round

pub struct RoundArgs<'a> {
    pub slug: &'a str,
    pub number: i128,
    pub person: &'a [String],
    pub round_type: Option<&'a str>,
    pub at: Option<&'a str>,
    pub folder: Option<&'a str>,
}

pub fn cmd_pipe_round(ctx: &Ctx, a: &RoundArgs) -> R<()> {
    let (_domain, _desc, pack_dir) = pipe_pack(ctx)?;
    let (fm, _body) = load_board(ctx)?;
    let slug = a.slug;
    if row_span(&fm, slug).is_none() {
        die!("no board row '{slug}'");
    }
    let stage = row_get(&fm, slug, "stage");
    if !matches!(stage.as_deref(), Some("preparing") | Some("applied") | Some("interviewing")) {
        die!("'{slug}' is at '{}' - rounds belong to preparing/applied/interviewing rows", core::py_opt(&stage));
    }
    let adir = app_dir(ctx, slug);
    if !pyos::isfile(&join(&adir, "application.md")) {
        die!("no application folder for '{slug}' - 'af pipe start {slug}' first");
    }

    let n = a.number;
    let people: Vec<String> = a.person.iter().map(|w| py_strip(w).to_string()).filter(|w| !w.is_empty()).collect();
    let fslug = if let Some(f) = nonempty(a.folder) {
        f.to_string()
    } else if let Some(first) = people.first() {
        let lead = first.split(',').next().unwrap_or("");
        let word = match py_split_ws(py_strip(lead)).first() {
            Some(w) => w.to_lowercase(),
            None => return Err(crash(CrashKind::Other, "IndexError: list index out of range")),
        };
        rx(r"[^a-z0-9]+").replace_all(&word, "-").trim_matches('-').to_string()
    } else {
        nonempty(a.round_type).unwrap_or("round").to_string()
    };
    if !folder_safe(&fslug) {
        die!("derived folder name '{fslug}' is not folder-safe - pass --folder");
    }

    let rdir = join(&adir, &format!("round-{n}-{fslug}"));
    if pyos::exists(&rdir) {
        die!("{} already exists", pyos::relpath(&rdir, &ctx.root)?);
    }
    pyos::makedirs_new(&rdir)?;

    let people_block = if people.is_empty() {
        "- _No interviewer named yet. Find out who is in the room before drafting anything else._".to_string()
    } else {
        people
            .iter()
            .map(|w| format!("- **{w}** - dossier `../people/`, read for this room in `interviewer-brief.md`"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let rtype = nonempty(a.round_type).unwrap_or("unknown");
    let skel = join(&pack_dir, "round-skeleton.md");
    if pyos::isfile(&skel) {
        let prev = if n > 1 { format!("round {}", n - 1) } else { "the application root".to_string() };
        let text = core::py_format(
            &read(&skel)?,
            &[
                ("slug", slug),
                ("round", &n.to_string()),
                ("round_type", rtype),
                ("held_at", nonempty(a.at).unwrap_or("TBD")),
                ("people", &people_block),
                ("date", &today()),
                ("prev", &prev),
            ],
        )?;
        write(&join(&rdir, "README.md"), &text)?;
    }
    let with = if people.is_empty() { String::new() } else { format!(" with {}", people.join(", ")) };
    append_activity(&adir, &format!("round_scaffolded: round {n} ({rtype}){with}"))?;

    outln!("af pipe round: {}/ scaffolded", pyos::relpath(&rdir, &ctx.root)?);
    outln!("\nJudgment (stays with the agent):");
    outln!("  - README.md is the folder map. Author round-sheet.md + interviewer-brief.md per the pack templates.");
    outln!("  - Research the room properly; a recruiter screen is the only round that earns a thin brief.");
    outln!("  - Cite the living dossiers (company-brief.md, jd-map.md, people/) - never restate them here.");
    outln!("  - After the round: transcript into sources/, then debrief.md, then promote its facts back out.");
    Ok(())
}

// ------------------------------------------------------------------ archive / unarchive

pub fn cmd_pipe_archive(ctx: &Ctx, slug: &str) -> R<()> {
    pipe_pack(ctx)?;
    let (fm, _body) = load_board(ctx)?;
    if row_span(&fm, slug).is_none() {
        die!("no board row '{slug}'");
    }
    let stage = row_get(&fm, slug, "stage");
    let stage_s = core::py_opt(&stage);
    if !matches!(stage.as_deref(), Some(s) if PIPE_TERMINAL.contains(&s)) {
        die!(
            "'{slug}' is at '{stage_s}' - archive applies to {}. Walking away from a live row is 'af pipe stage {slug} dropped', which is the honest record.",
            PIPE_TERMINAL.join(", ")
        );
    }
    let src = join(&apps_root(ctx), slug);
    if !pyos::isdir(&src) {
        die!("no active application folder for '{slug}'");
    }
    let dest_root = join(&apps_root(ctx), APPS_ARCHIVE);
    pyos::makedirs(&dest_root)?;
    let dest = join(&dest_root, slug);
    if pyos::exists(&dest) {
        die!("{} already exists", pyos::relpath(&dest, &ctx.root)?);
    }
    move_path(&src, &dest)?;
    append_activity(&dest, &format!("archived: stage '{stage_s}'; folder moved to applications/{APPS_ARCHIVE}/"))?;
    outln!("af pipe archive: {slug} -> workspace/pipeline/applications/{APPS_ARCHIVE}/{slug}/ (stage '{stage_s}')");
    outln!("\nJudgment (stays with the agent):");
    outln!("  - The board row stays put. Archiving moves the folder out of the browsing list, nothing else.");
    outln!("  - Anything worth banking? Run career-harvest before the detail goes cold.");
    Ok(())
}

pub fn cmd_pipe_unarchive(ctx: &Ctx, slug: &str) -> R<()> {
    pipe_pack(ctx)?;
    let (fm, _body) = load_board(ctx)?;
    if row_span(&fm, slug).is_none() {
        die!("no board row '{slug}'");
    }
    let src = join_all(&[&apps_root(ctx), APPS_ARCHIVE, slug]);
    if !pyos::isdir(&src) {
        die!("'{slug}' is not archived");
    }
    let dest = join(&apps_root(ctx), slug);
    if pyos::exists(&dest) {
        die!("{} already exists", pyos::relpath(&dest, &ctx.root)?);
    }
    move_path(&src, &dest)?;
    append_activity(&dest, "unarchived: folder returned to the active application list")?;
    outln!("af pipe unarchive: {slug} -> workspace/pipeline/applications/{slug}/");
    outln!("\nJudgment (stays with the agent):");
    outln!(
        "  - Stage is '{}'; a revived row usually wants 'af pipe stage {slug} interviewing'.",
        core::py_opt(&row_get(&fm, slug, "stage"))
    );
    Ok(())
}

// ------------------------------------------------------------------ board

fn ljust(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(w - n))
    }
}

pub fn cmd_pipe_board(ctx: &Ctx) -> R<()> {
    if !pyos::isfile(&board_path(ctx)) {
        outln!("af pipe board: no pipeline yet — 'af pipe save' opens the first row");
        return Ok(());
    }
    let (fm, _) = load_board(ctx)?;
    let mut rows = pipe_rows(&fm);
    if rows.is_empty() {
        outln!("af pipe board: board is empty");
        return Ok(());
    }
    const COLS: [&str; 7] = ["stage", "company", "role", "deadline", "applied", "next_nudge", "shipped"];
    let order = |r: &str| -> usize {
        row_get(&fm, r, "stage").and_then(|s| PIPE_STAGES.iter().position(|p| *p == s)).unwrap_or(99)
    };
    rows.sort_by(|a, b| (order(a), a).cmp(&(order(b), b)));
    let table: Vec<Vec<String>> = rows
        .iter()
        .map(|s| {
            let mut r = vec![s.clone()];
            for c in COLS {
                r.push(match row_get(&fm, s, c) {
                    Some(v) if !v.is_empty() => v,
                    _ => "-".to_string(),
                });
            }
            r
        })
        .collect();
    let mut header = vec!["slug".to_string()];
    header.extend(COLS.iter().map(|c| c.to_string()));
    let widths: Vec<usize> = (0..header.len())
        .map(|i| table.iter().chain(std::iter::once(&header)).map(|r| r[i].chars().count()).max().unwrap_or(0))
        .collect();
    let render = |r: &Vec<String>| r.iter().zip(&widths).map(|(v, w)| ljust(v, *w)).collect::<Vec<_>>().join("  ");
    outln!("{}", render(&header));
    for r in &table {
        outln!("{}", render(r));
    }
    Ok(())
}
