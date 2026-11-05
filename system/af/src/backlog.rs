//! `af backlog`: the open rows of `system/backlog.md`, and which of them a builder can work
//! now. A row with a `Waits:` line needs an event or a decision first (library/schema/backlog.md).

use crate::core::Ctx;
use crate::error::R;
use crate::life::{vpath, TextFile};
use crate::outln;
use clap::Subcommand;
use serde_json::json;

const BACKLOG_REL: &str = "system/backlog.md";

#[derive(Subcommand, Debug)]
pub enum BacklogCmd {
    /// Open rows with no Waits: line, oldest first
    Ready {
        /// at most this many rows
        #[arg(long)]
        limit: Option<usize>,
        /// id, title, and the whole row text for each
        #[arg(long)]
        json: bool,
    },
    /// Every open row and what it waits on
    List,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub title: String,
    pub waits: Option<String>,
    pub text: String,
}

/// The rows of a backlog file, in file order. A row runs from its `## BB-` heading to the next
/// `## ` heading.
pub fn parse(text: &str) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut cur: Option<(String, String, Vec<String>)> = None;
    let mut push = |c: Option<(String, String, Vec<String>)>| {
        if let Some((id, title, lines)) = c {
            let waits = lines.iter().find_map(|l| l.strip_prefix("Waits:").map(|w| w.trim().to_string()));
            let text = lines.join("\n").trim_end().to_string();
            rows.push(Row { id, title, waits, text });
        }
    };
    for line in text.lines() {
        if line.starts_with("## ") {
            push(cur.take());
            if let Some(rest) = line.strip_prefix("## BB-") {
                let mut parts = rest.splitn(2, ' ');
                let id = format!("BB-{}", parts.next().unwrap_or(""));
                let title = parts.next().unwrap_or("").trim_start_matches(['·', '—', ' ']).trim().to_string();
                cur = Some((id, title, vec![line.to_string()]));
            }
        } else if let Some((_, _, lines)) = cur.as_mut() {
            lines.push(line.to_string());
        }
    }
    push(cur.take());
    rows
}

pub fn run(ctx: &Ctx, cmd: BacklogCmd) -> R<i32> {
    let file = TextFile::load(&vpath(ctx, BACKLOG_REL))?;
    let rows = file.map(|f| parse(&f.text)).unwrap_or_default();
    match cmd {
        BacklogCmd::Ready { limit, json } => {
            let mut ready: Vec<&Row> = rows.iter().filter(|r| r.waits.is_none()).collect();
            ready.sort_by(|a, b| a.id.cmp(&b.id));
            if let Some(n) = limit {
                ready.truncate(n);
            }
            if json {
                let v: Vec<_> = ready.iter().map(|r| json!({"id": r.id, "title": r.title, "text": r.text})).collect();
                outln!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
            } else {
                for r in &ready {
                    outln!("{} · {}", r.id, r.title);
                }
                outln!("{} row(s) ready, {} waiting", ready.len(), rows.iter().filter(|r| r.waits.is_some()).count());
            }
        }
        BacklogCmd::List => {
            for r in &rows {
                match &r.waits {
                    Some(w) => outln!("{} · {} · waits: {w}", r.id, r.title),
                    None => outln!("{} · {} · ready", r.id, r.title),
                }
            }
            outln!("{} open row(s)", rows.len());
        }
    }
    Ok(0)
}
