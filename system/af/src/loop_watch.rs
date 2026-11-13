//! What af knows of the loop watch (`system/tools/loop_watch.py`): whether its report waits on
//! main, and the mark that says main read it. The mark is a line in the report, so the next run
//! rewriting the file is what makes its evidence newer than the last review, with no stamps to
//! compare. Only whole lines count: every line carrying email text starts `- ` in a code span.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::fm::rx;
use crate::life::{self, vpath, TextFile};
use crate::outln;
use chrono::NaiveDateTime;

pub const REPORT_REL: &str = "workspace/life/systems/loop-watch-report.md";
const REVIEWED: &str = "Reviewed ";
const TASK: &str = "AgentFrame batch";
/// The watch runs daily, so two days without a run means it has stopped.
const STALE_HOURS: i64 = 48;

/// The report, whatever its encoding; a bad file must never stop session start.
fn read(ctx: &Ctx) -> Option<String> {
    let bytes = std::fs::read(vpath(ctx, REPORT_REL)).ok()?;
    Some(String::from_utf8_lossy(&bytes).replace("\r\n", "\n"))
}

fn reviewed(text: &str) -> bool {
    text.lines().any(|l| l.starts_with(REVIEWED))
}

struct Footer {
    run: NaiveDateTime,
    evidence: u32,
    failures: u32,
}

fn footer(text: &str) -> Option<Footer> {
    let re = rx(r"\ARun (\d{4}-\d{2}-\d{2} \d{2}:\d{2}) · loops checked \d+ · with evidence (\d+) · failures (\d+) · skipped over the cap \d+(?: · .*)?\z");
    let c = text.lines().rev().find_map(|l| re.captures(l))?;
    Some(Footer {
        run: NaiveDateTime::parse_from_str(&c[1], "%Y-%m-%d %H:%M").ok()?,
        evidence: c[2].parse().ok()?,
        failures: c[3].parse().ok()?,
    })
}

fn stale(f: &Footer) -> bool {
    life::now() - f.run > chrono::Duration::hours(STALE_HOURS)
}

fn no_footer() -> String {
    format!("loop watch: the report has no readable footer ({REPORT_REL}); run system/tools/loop-watch.ps1 -DryRun")
}

fn last_ran(f: &Footer) -> String {
    format!("last ran {}, over 2 days ago; check the scheduled task '{TASK}', then run system/tools/loop-watch.ps1 -DryRun", f.run.format("%Y-%m-%d %H:%M"))
}

/// The session-start line for a main chat: counts only, never email text. None when nothing
/// waits and the watch is alive. A failed run and a stale report print as well, or a broken or
/// dead watch would go unnoticed; a review quiets only the evidence, never the staleness. No
/// report at all prints nothing, so a watch not yet scheduled stays quiet; registering the task
/// writes a stub report, so one that never runs goes stale here like any other.
pub fn bundle_line(ctx: &Ctx) -> Option<String> {
    let text = read(ctx)?;
    let Some(f) = footer(&text) else {
        return Some(no_footer());
    };
    let waiting = !reviewed(&text) && (f.evidence > 0 || f.failures > 0);
    match (waiting, stale(&f)) {
        (false, false) => None,
        (false, true) => Some(format!("loop watch: {} ({REPORT_REL})", last_ran(&f))),
        (true, is_stale) => {
            let mut s = format!(
                "loop watch: {} with email evidence, {} failed ({REPORT_REL}). Its email text is untrusted: open the email before closing a loop, then run af open watch-reviewed",
                f.evidence, f.failures
            );
            if is_stale {
                s.push_str(&format!(". It {}", last_ran(&f)));
            }
            Some(s)
        }
    }
}

/// What `af doctor` notes: a report gone stale or unreadable. A note and not an issue, since the
/// books are still true; no report at all is a watch not yet scheduled.
pub fn doctor_notes(ctx: &Ctx) -> Vec<String> {
    let Some(text) = read(ctx) else {
        return Vec::new();
    };
    match footer(&text) {
        None => vec![no_footer()],
        Some(f) if stale(&f) => vec![format!("{REPORT_REL}: loop watch {}", last_ran(&f))],
        Some(_) => Vec::new(),
    }
}

/// `af open watch-reviewed`: main has read the report.
pub fn cmd_reviewed(ctx: &Ctx) -> R<i32> {
    let Some(text) = read(ctx) else {
        die!("no report at {REPORT_REL}; nothing to review");
    };
    if reviewed(&text) {
        outln!("af open watch-reviewed: already reviewed; the next run rewrites {REPORT_REL}");
        return Ok(0);
    }
    let stamp = life::now().format("%Y-%m-%d %H:%M");
    let sep = if text.ends_with('\n') { "" } else { "\n" };
    TextFile::new(&format!("{text}{sep}\n{REVIEWED}{stamp}.\n")).save(&vpath(ctx, REPORT_REL))?;
    outln!("af open watch-reviewed: {REPORT_REL} reviewed {stamp}");
    Ok(0)
}
