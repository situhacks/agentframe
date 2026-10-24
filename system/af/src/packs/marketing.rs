//! Marketing pack rules (library/domains/marketing/rules.py): the post-FINAL assembly
//! record on draft and ready, and the receipt-writing publish for post rows.

use crate::core::{self, today};
use crate::die;
use crate::error::R;
use crate::fm::{self, all_rows, get_scalar, row_get, row_set, row_span, rx, set_scalar, split_fm};
use crate::outln;
use crate::pyos::{self, join, py_strip};

const POST_FINAL_SKELETON: &str = "---\nstatus: drafting\nlast_updated: {date}\n---\n\n# {title} — FINAL\n\nAssembled from the post's ready ingredients per the project manifest.\n";

fn skeleton(title: &str) -> String {
    POST_FINAL_SKELETON.replace("{date}", &today()).replacen("{title}", title, 1)
}

fn manifest_ingredients(cfm: &str) -> Vec<String> {
    match rx(r"(?m)^\s*ingredients:\s*\[(.*?)\]\s*$").captures(cfm) {
        None => Vec::new(),
        Some(c) => c[1].split(',').map(|i| py_strip(i).to_string()).filter(|i| !i.is_empty()).collect(),
    }
}

fn is_post_slug(s: &str) -> bool {
    rx(r"\Apost-\d+\n?\z").is_match(s)
}

pub fn published_posts(fm_text: &str) -> usize {
    all_rows(fm_text)
        .iter()
        .filter(|s| is_post_slug(s) && row_get(fm_text, s, "status").as_deref() == Some("published"))
        .count()
}

pub fn archived_published_posts(cdir: &str) -> R<usize> {
    let p = join(cdir, &pyos::join_all(&["knowledge", "_archive", "deliverables-archive.md"]));
    if !pyos::isfile(&p) {
        return Ok(0);
    }
    let (afm, _) = split_fm(&core::read(&p)?, "deliverables-archive.md")?;
    Ok(published_posts(&afm))
}

/// Span of the section an earlier ready of this ingredient assembled, as the Python's
/// `^## .*\(ready from ING-v\d+\.md\)\n.*?(?=^## |\Z)` (MULTILINE|DOTALL) finds it: from the
/// first `## ` line that precedes the last receipt line, to the next `## ` line or the end.
fn old_section(body: &str, ingredient: &str) -> Option<(usize, usize)> {
    let occ = rx(&format!(r"\(ready from {}-v\d+\.md\)\n", fm::esc(ingredient)));
    let mut last: Option<(usize, usize)> = None;
    let mut pos = 0;
    while pos <= body.len() {
        match occ.find_at(body, pos) {
            None => break,
            Some(m) => {
                last = Some((m.start(), m.end()));
                pos = m.start() + body[m.start()..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            }
        }
    }
    let (s, e) = last?;
    let line_start = |p: usize| p == 0 || body.as_bytes()[p - 1] == b'\n';
    let p = (0..=s.saturating_sub(3)).find(|&p| p + 3 <= s && line_start(p) && body.as_bytes()[p..].starts_with(b"## "))?;
    let q = (e..=body.len())
        .find(|&q| q == body.len() || (line_start(q) && body.as_bytes()[q..].starts_with(b"## ")))
        .unwrap_or(body.len());
    Some((p, q))
}

fn assemble_post_final(post_dir: &str, ready_path: &str, ingredient: &str) -> R<String> {
    let pf = join(post_dir, "post-FINAL.md");
    let title = pyos::basename(post_dir);
    if !pyos::isfile(&pf) {
        core::write(&pf, &skeleton(&title))?;
    }
    let (pfm, mut body) = split_fm(&core::read(&pf)?, "post-FINAL.md")?;
    let (_, src_body) = split_fm(&core::read(ready_path)?, ready_path)?;
    let fname = pyos::basename(ready_path);
    let heading = format!("## {} (ready from {fname})", pyos::py_title(&ingredient.replace('-', " ")));
    let section = format!("{heading}\n{}\n", py_strip(&src_body));
    body = match old_section(&body, ingredient) {
        Some((s, e)) => format!("{}{}\n{}", &body[..s], section, &body[e..]),
        None => format!("{}\n\n{}", body.trim_end_matches('\n'), section),
    };
    let pfm = set_scalar(&pfm, "last_updated", &today(), "post-FINAL.md")?;
    core::write(&pf, &fm::join_fm(&pfm, &body))?;
    Ok(pf)
}

fn under_posts(rel: &str) -> bool {
    rx(r"(^|/)posts/").is_match(&rel.replace('\\', "/"))
}

/// Create the post assembly record when its first ingredient starts.
pub fn on_draft(cdir: &str, dpath: &str, rel: &str, cfm: String, parent_row: &str) -> R<(String, Vec<String>)> {
    let mut notes = Vec::new();
    if !under_posts(rel) {
        return Ok((cfm, notes));
    }
    let post_dir = pyos::dirname(dpath);
    let pf = join(&post_dir, "post-FINAL.md");
    if !pyos::isfile(&pf) {
        core::write(&pf, &skeleton(&pyos::basename(&post_dir)))?;
        notes.push("post-FINAL.md created".to_string());
    }
    let mut cfm = cfm;
    if row_span(&cfm, parent_row).is_some() {
        let pf_rel = pyos::relpath(&pf, cdir)?.replace('\\', "/");
        cfm = row_set(&cfm, parent_row, "file", &pf_rel)?;
    }
    Ok((cfm, notes))
}

fn post_complete(post_dir: &str, ingredients: &[String], current_ready_path: Option<&str>) -> R<bool> {
    for ing in ingredients {
        let ns = core::versions_in(post_dir, ing);
        let max = match ns.into_iter().max() {
            Some(m) => m,
            None => return Ok(false),
        };
        let best = join(post_dir, &format!("{ing}-v{max}.md"));
        if let Some(cur) = current_ready_path {
            if !cur.is_empty() && pyos::abspath(&best) == pyos::abspath(cur) {
                continue;
            }
        }
        let (bfm, _) = split_fm(&core::read(&best)?, &best)?;
        if get_scalar(&bfm, "status").as_deref() != Some("ready") {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Assemble post-FINAL when the ready file is a post ingredient; promote it once every
/// manifest ingredient is ready.
pub fn on_ready(cdir: &str, dpath: &str, rel: &str, cfm: String) -> R<(String, Vec<String>)> {
    let mut notes = Vec::new();
    let norm = rel.replace('\\', "/");
    if !under_posts(&norm) {
        return Ok((cfm, notes));
    }
    let post_dir = pyos::dirname(dpath);
    let base = pyos::basename(dpath);
    let ing = match rx(r"\A(.+)-v\d+\.md\n?\z").captures(&base) {
        Some(c) => c[1].to_string(),
        None => {
            if base == "post-FINAL.md" {
                let ings = manifest_ingredients(&cfm);
                if !ings.is_empty() && !post_complete(&post_dir, &ings, None)? {
                    let mut pending = Vec::new();
                    for i in &ings {
                        if !post_complete(&post_dir, std::slice::from_ref(i), None)? {
                            pending.push(i.clone());
                        }
                    }
                    let row = all_rows(&cfm)
                        .into_iter()
                        .find(|r| row_get(&cfm, r, "file").as_deref() == Some(norm.as_str()))
                        .unwrap_or_else(|| pyos::basename(&post_dir));
                    let how: Vec<String> = pending.iter().map(|i| format!("af ready <project> {row} --artifact {i}")).collect();
                    die!(
                        "{rel}: ready requires every manifest ingredient to be ready. Still drafting: {}. Ready each one first: {}",
                        pending.join(", "),
                        how.join(" ; ")
                    );
                }
            }
            return Ok((cfm, notes));
        }
    };
    assemble_post_final(&post_dir, dpath, &ing)?;
    notes.push("post-FINAL.md updated".to_string());
    let ings = manifest_ingredients(&cfm);
    let mut cfm = cfm;
    if !ings.is_empty() && post_complete(&post_dir, &ings, Some(dpath))? {
        let pf = join(&post_dir, "post-FINAL.md");
        let (pfm, pbody) = split_fm(&core::read(&pf)?, "post-FINAL.md")?;
        let pfm = set_scalar(&pfm, "status", "ready", "post-FINAL.md")?;
        let pfm = set_scalar(&pfm, "last_updated", &today(), "post-FINAL.md")?;
        core::write(&pf, &fm::join_fm(&pfm, &pbody))?;
        let pf_rel = pyos::relpath(&pf, cdir)?.replace('\\', "/");
        let post_slug = all_rows(&cfm)
            .into_iter()
            .find(|r| row_get(&cfm, r, "file").as_deref() == Some(pf_rel.as_str()))
            .unwrap_or_else(|| pyos::basename(&post_dir));
        if row_span(&cfm, &post_slug).is_some() {
            cfm = row_set(&cfm, &post_slug, "status", "ready")?;
            cfm = row_set(&cfm, &post_slug, "last_updated", &today())?;
        }
        notes.push("all manifest ingredients ready — post-FINAL ready".to_string());
    }
    Ok((cfm, notes))
}

pub struct PublishArgs<'a> {
    pub deliverable: &'a str,
    pub url: Option<&'a str>,
    pub posted_at: Option<&'a str>,
    pub platform: Option<&'a str>,
    pub media: &'a [String],
}

/// True only for marketing post assembly rows.
pub fn handles_publish(cdir: &str, args: &PublishArgs) -> R<bool> {
    let cpath = join(cdir, "project.md");
    let (cfm, _) = split_fm(&core::read(&cpath)?, "project.md")?;
    let target = args.deliverable.replace('\\', "/");
    let rel = if target.contains('/') { Some(target) } else { row_get(&cfm, &target, "file") };
    Ok(matches!(rel, Some(r) if !r.is_empty() && r.ends_with("post-FINAL.md")))
}

fn nonempty<'a>(v: Option<&'a str>) -> Option<&'a str> {
    v.filter(|s| !s.is_empty())
}

/// The marketing receipt plus the shared published state.
pub fn publish(cdir: &str, args: &PublishArgs) -> R<()> {
    let cpath = join(cdir, "project.md");
    let (cfm, cbody) = split_fm(&core::read(&cpath)?, "project.md")?;
    let target = args.deliverable.replace('\\', "/");
    let (post, rel) = if target.contains('/') {
        let rows: Vec<String> =
            all_rows(&cfm).into_iter().filter(|r| row_get(&cfm, r, "file").as_deref() == Some(target.as_str())).collect();
        if rows.len() != 1 {
            die!("no unique tracker row points at '{target}'");
        }
        (rows[0].clone(), target.clone())
    } else {
        match row_get(&cfm, &target, "file") {
            Some(f) if !f.is_empty() => (target.clone(), f),
            _ => die!("tracker row '{target}' not found or has no file"),
        }
    };
    if !rel.ends_with("post-FINAL.md") {
        die!("row '{post}' points at {rel}, not a post-FINAL.md — publish operates on the assembly record");
    }
    let pf = join(cdir, &rel);
    let (pfm, pbody) = split_fm(&core::read(&pf)?, &rel)?;
    let status = get_scalar(&pfm, "status");
    if status.as_deref() != Some("ready") {
        let found = match &status {
            Some(s) if !s.is_empty() => s.clone(),
            _ => "missing".to_string(),
        };
        die!("{rel}: publish requires status ready (found {found}); run af ready first");
    }
    let url = match nonempty(args.url) {
        Some(u) => u,
        None => die!("marketing publish requires --url so the live receipt is reconstructable"),
    };
    let posted = nonempty(args.posted_at).map(|s| s.to_string()).unwrap_or_else(core::now_iso);
    let platform = nonempty(args.platform).unwrap_or("linkedin");
    let shipped = match rx(r"\A(\d{4}-\d{2}-\d{2})").captures(&posted) {
        Some(c) => c[1].to_string(),
        None => today(),
    };
    let pfm = set_scalar(&pfm, "status", "published", &rel)?;
    let pfm = set_scalar(&pfm, "last_updated", &today(), &rel)?;
    let pfm = rx(r"\n(shipped_at:.*|published:(\n  .*)*|shipped_media:(\n  - .*)*)").replace_all(&pfm, "").into_owned();
    let mut block = vec![
        format!("shipped_at: {shipped}"),
        "published:".to_string(),
        format!("  platform: {platform}"),
        format!("  url: {url}"),
        format!("  posted_at: {posted}"),
    ];
    if !args.media.is_empty() {
        block.push("shipped_media:".to_string());
        block.extend(args.media.iter().map(|m| format!("  - {m}")));
    }
    let pfm = format!("{}\n{}", pfm.trim_end_matches('\n'), block.join("\n"));
    core::write(&pf, &fm::join_fm(&pfm, &pbody))?;

    let cfm = row_set(&cfm, &post, "status", "published")?;
    let mut cfm = row_set(&cfm, &post, "last_updated", &today())?;
    let published = published_posts(&cfm) + archived_published_posts(cdir)?;
    if matches!(get_scalar(&cfm, "shipped_at").as_deref(), None | Some("null") | Some("")) {
        cfm = fm::upsert_scalar(&cfm, "shipped_at", &shipped, Some("deliverables"))?;
    }
    let cfm = core::touch_lifecycle(&cfm)?;
    core::write(&cpath, &fm::join_fm(&cfm, &cbody))?;
    core::append_activity(cdir, &format!("post_published: {post} → {url}"))?;

    outln!("af publish: {post} -> published ({url}); published posts={published}");
    outln!("\nJudgment checklist (agent + operator):");
    outln!("  [ ] Published copy reconciled before this immutable receipt: if the live post differs");
    outln!("      materially, create a new tracked edition rather than mutating this published record");
    outln!("  [ ] shipped_media recorded for every asset that actually shipped (--media)");
    outln!("  [ ] Performance capture scheduled (~14 days after posted_at, per performance-capture.md)");
    outln!("  [ ] User-voiced? Run voice-harvest and deliverable-harvest now");
    outln!("      (.claude/skills/voice-harvest/SKILL.md, .claude/skills/deliverable-harvest/SKILL.md);");
    outln!("      corpus promotion is eligible from this publish. Not a box to tick: run them.");
    Ok(())
}
