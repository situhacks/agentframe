//! `af brief`: the zero-token catch-up. Reads open loops, the registry, the backlog, the
//! pipeline and studio boards, and the index age; prints one Markdown block.

use crate::core::{parse_iso_date, today_date, Ctx};
use crate::doctor::STUDIO_STATES;
use crate::error::R;
use crate::fm::{self, mapping_row_get, mapping_rows, pipe_rows, row_get};
use crate::life::{self, vpath, TextFile};
use crate::open::{self, OpenDoc};
use crate::out;
use crate::outln;
use crate::pyos;
use crate::sessions::{self, Registry};
use serde_json::{json, Value};

pub const STUDIO_WEEK_DAYS: i64 = 6;

/// Pipeline rows whose follow-up is due: (slug, stage, next_nudge). Doctor's check.
pub fn pipeline_followups(ctx: &Ctx) -> Vec<(String, String, String)> {
    let board = pyos::join(&ctx.pipeline, "pipeline.md");
    let text = match TextFile::load(&board) {
        Ok(Some(t)) => t.text,
        _ => return Vec::new(),
    };
    let bfm = match fm::split_fm_optional(&text) {
        Some((f, _)) => f,
        None => return Vec::new(),
    };
    let today = today_date();
    let mut out = Vec::new();
    for slug in pipe_rows(&bfm) {
        let stage = row_get(&bfm, &slug, "stage").unwrap_or_default();
        if stage != "applied" && stage != "interviewing" {
            continue;
        }
        if let Some(n) = parse_iso_date(row_get(&bfm, &slug, "next_nudge").as_deref()) {
            if n <= today {
                out.push((slug.clone(), stage, n.format("%Y-%m-%d").to_string()));
            }
        }
    }
    out
}

/// Studio slots dated within the coming week (or already passed) that have not posted:
/// (slug, state, date).
pub fn studio_due(ctx: &Ctx) -> Vec<(String, String, String)> {
    let cal = pyos::join(&ctx.studio, "calendar.md");
    let text = match TextFile::load(&cal) {
        Ok(Some(t)) => t.text,
        _ => return Vec::new(),
    };
    let cfm = match fm::split_fm_optional(&text) {
        Some((f, _)) => f,
        None => return Vec::new(),
    };
    let horizon = today_date() + chrono::Duration::days(STUDIO_WEEK_DAYS);
    let mut out = Vec::new();
    for slug in mapping_rows(&cfm, "slots") {
        let state = mapping_row_get(&cfm, "slots", &slug, "state").unwrap_or_default();
        if !STUDIO_STATES.contains(&state.as_str()) || state == "posted" || state == "dropped" {
            continue;
        }
        if let Some(d) = parse_iso_date(mapping_row_get(&cfm, "slots", &slug, "date").as_deref()) {
            if d <= horizon {
                out.push((slug.clone(), state, d.format("%Y-%m-%d").to_string()));
            }
        }
    }
    out.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)));
    out
}

/// The newest `YYYY-MM-DD.md` under `workspace/life/briefs/`; None when the folder is absent
/// (Some("none") when it holds no brief yet).
pub fn last_brief(ctx: &Ctx) -> Option<String> {
    let dir = vpath(ctx, "workspace/life/briefs");
    if !pyos::isdir(&dir) {
        return None;
    }
    let re = fm::rx(r"\A(\d{4}-\d{2}-\d{2})\.md\z");
    let mut dates: Vec<String> = pyos::listdir(&dir)
        .unwrap_or_default()
        .iter()
        .filter_map(|n| re.captures(n).map(|c| c[1].to_string()))
        .collect();
    dates.sort();
    Some(dates.pop().unwrap_or_else(|| "none".to_string()))
}

/// The brief as (markdown, json).
pub fn build(ctx: &Ctx) -> R<(String, Value)> {
    let today = today_date();
    let mut md: Vec<String> = vec![format!("# Brief {} ({})", today.format("%Y-%m-%d"), life::now_min())];
    let mut js = json!({"date": today.format("%Y-%m-%d").to_string(), "as_of": life::now_min()});

    let doc = OpenDoc::load(ctx).ok().flatten();
    let due = doc.as_ref().map(|d| d.due(today)).unwrap_or_default();
    let mut s = format!("## Due loops ({})", due.len());
    if due.is_empty() {
        s.push_str("\n- none");
    }
    for (l, age) in &due {
        s.push_str(&format!("\n{}", open::due_line(l, *age)));
    }
    md.push(s);
    js["due"] = Value::Array(due.iter().map(|(l, a)| open::loop_json(l, Some(*a))).collect());

    let sections = doc.as_ref().map(|d| d.by_section()).unwrap_or_default();
    let total: usize = sections.iter().map(|(_, n)| n).sum();
    let mut s = format!("## Open loops ({total})");
    if sections.is_empty() {
        s.push_str("\n- none");
    }
    for (sec, n) in &sections {
        let label = if sec.is_empty() { "(no heading)" } else { sec.as_str() };
        s.push_str(&format!("\n- {label}: {n}"));
    }
    md.push(s);
    js["open"] = json!({"total": total, "by_section": sections.iter().map(|(s, n)| json!({"section": s, "count": n})).collect::<Vec<_>>()});

    let reg = Registry::load(ctx)?;
    let now = life::now();
    let mut chats = Vec::new();
    let mut cj = Vec::new();
    for r in reg.rows() {
        let st = sessions::effective_status(&r, now);
        if st != "live" && st != "idle" {
            continue;
        }
        let current = !sessions::handoff_required(ctx, &r)?;
        let hpath = vpath(ctx, &sessions::handoff_rel(&r.name));
        let has = pyos::isfile(&hpath);
        let label = if !has { "no handoff" } else if current { "handoff current" } else { "handoff stale" };
        chats.push(format!("- {} · {st} · last seen {} · {label}", r.name, r.last_seen));
        cj.push(json!({"name": r.name, "role": r.role, "status": st, "last_seen": r.last_seen, "handoff_exists": has, "handoff_current": has && current}));
    }
    if !chats.is_empty() {
        md.push(format!("## Chats ({})\n{}", chats.len(), chats.join("\n")));
    }
    js["chats"] = Value::Array(cj);

    if let Some(ids) = life::backlog_open_ids(ctx) {
        let mut s = format!("## Backlog ({} open)", ids.len());
        if !ids.is_empty() {
            s.push_str(&format!("\n{}", ids.join(", ")));
        }
        md.push(s);
        js["backlog"] = json!({"open": ids.len(), "ids": ids});
    }

    let pf = pipeline_followups(ctx);
    if !pf.is_empty() {
        let lines: Vec<String> = pf.iter().map(|(s, st, d)| format!("- {s}: follow-up due since {d} ({st})")).collect();
        md.push(format!("## Pipeline follow-ups ({})\n{}", pf.len(), lines.join("\n")));
    }
    js["pipeline_followups"] = Value::Array(pf.iter().map(|(s, st, d)| json!({"slug": s, "stage": st, "next_nudge": d})).collect());

    let sd = studio_due(ctx);
    if !sd.is_empty() {
        let lines: Vec<String> = sd.iter().map(|(s, st, d)| format!("- {s}: {d} ({st})")).collect();
        md.push(format!("## Studio this week ({})\n{}", sd.len(), lines.join("\n")));
    }
    js["studio_due"] = Value::Array(sd.iter().map(|(s, st, d)| json!({"slug": s, "state": st, "date": d})).collect());

    let ix = life::index_age_line(ctx);
    let lb = last_brief(ctx);
    let mut tail = Vec::new();
    if let Some(i) = &ix {
        tail.push(i.clone());
    }
    if let Some(b) = &lb {
        tail.push(format!("last written brief: {b}"));
    }
    if !tail.is_empty() {
        md.push(tail.join("\n"));
    }
    js["index"] = json!(ix);
    js["last_brief"] = json!(lb);
    Ok((md.join("\n\n"), js))
}

pub fn cmd_brief(ctx: &Ctx, as_json: bool, write: bool) -> R<i32> {
    let (md, js) = build(ctx)?;
    if write {
        let rel = format!("workspace/life/briefs/{}.md", today_date().format("%Y-%m-%d"));
        TextFile::new(&format!("{md}\n")).save(&vpath(ctx, &rel))?;
        if !as_json {
            outln!("{md}");
            outln!("\n(saved to {rel})");
            return Ok(0);
        }
    }
    if as_json {
        out::line(&serde_json::to_string_pretty(&js).unwrap_or_default());
    } else {
        out::line(&md);
    }
    Ok(0)
}
