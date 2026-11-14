//! The session page: the shape of `system/sessions/<name>/handoff.md`, the checks on it, and
//! `af session tidy`, which moves finished items to `archive.md` beside it.
//!
//! The page is what is current and nothing else. The numbers below are the only place the
//! limits are written; the schema page (`library/schema/handoff.md`) quotes them.

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::fm;
use crate::life::{vpath, TextFile};
use crate::outln;
use crate::pyos;
use crate::sessions::{self, Registry, Row};
use chrono::NaiveDate;

/// The body (frontmatter not counted) may hold this many words.
pub const PAGE_WORD_CAP: usize = 1200;
/// `## State` may hold this many words.
pub const STATE_WORD_CAP: usize = 150;
/// `## Done` holds at most this many items.
pub const DONE_MAX_ITEMS: usize = 10;
/// A `Done` item older than this many days goes to the archive.
pub const DONE_MAX_DAYS: i64 = 14;
/// A `To do` item dated more than this many days ago needs a decision.
pub const TODO_MAX_DAYS: i64 = 21;
/// A `Gated` item dated more than this many days ago needs its condition checked.
pub const GATED_MAX_DAYS: i64 = 45;
/// A `Rules` item dated more than this many days ago needs confirming.
pub const RULE_MAX_DAYS: i64 = 30;
/// How much of an item the issue texts quote.
pub const QUOTE_CHARS: usize = 40;

/// Every `## ` heading a page may hold, in order.
pub const SECTIONS: [&str; 9] =
    ["State", "Doing", "To do", "Gated", "Done", "Rules", "Open questions", "Next step", "Files touched"];
/// The sections every page has.
pub const REQUIRED: [&str; 3] = ["State", "Next step", "Files touched"];
/// The sections whose lines are dated items.
pub const ITEM_SECTIONS: [&str; 5] = ["Doing", "To do", "Gated", "Done", "Rules"];

pub fn archive_rel(name: &str) -> String {
    format!("system/sessions/{name}/archive.md")
}

// ------------------------------------------------------------------ parsing

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    /// Index of the `## ` line in `Page::lines`.
    pub heading: usize,
    /// The content lines are `start..end`.
    pub start: usize,
    pub end: usize,
}

/// One dated item: its first line and the indented lines under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub first: usize,
    /// One past its last line.
    pub end: usize,
    /// None when the line does not start with `- YYYY-MM-DD · ` (or the date is not a real one).
    pub date: Option<NaiveDate>,
    /// The first line after the date prefix (the whole line when there is no prefix).
    pub head: String,
    /// The first line's text and its continuation lines, joined with spaces.
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// The raw frontmatter block with its `---` lines, empty when the page has none.
    pub front: String,
    /// Everything after the frontmatter, as written.
    pub body: String,
    /// The body's lines without their line ends.
    pub lines: Vec<String>,
    /// Per body line: inside a fenced code block (the fence lines included).
    pub fenced: Vec<bool>,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `[fix]`: mechanical, a chat fixes it in a minute, and the Stop hook asks for it.
    Fix,
    /// `[review]`: needs the chat's judgment; never blocks a stop.
    Review,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub kind: Kind,
    pub text: String,
}

impl Page {
    /// Split a page's text. A leading byte-order mark is ignored.
    pub fn parse(text: &str) -> Page {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let (front, body) = match fm::split_fm_optional(text) {
            Some((_, b)) => (text[..text.len() - b.len()].to_string(), b),
            None => (String::new(), text.to_string()),
        };
        let lines: Vec<String> = body.lines().map(|l| l.to_string()).collect();
        let fenced = fence_lines(&lines);
        let mut sections: Vec<Section> = Vec::new();
        for (i, l) in lines.iter().enumerate() {
            if fenced[i] {
                continue;
            }
            if let Some(name) = l.strip_prefix("## ") {
                if let Some(prev) = sections.last_mut() {
                    prev.end = i;
                }
                sections.push(Section { name: name.trim().to_string(), heading: i, start: i + 1, end: lines.len() });
            }
        }
        Page { front, body, lines, fenced, sections }
    }

    pub fn words(&self) -> usize {
        self.body.split_whitespace().count()
    }

    /// The first section with this name.
    pub fn section(&self, name: &str) -> Option<&Section> {
        self.sections.iter().find(|s| s.name == name)
    }

    fn section_words(&self, s: &Section) -> usize {
        self.lines[s.start..s.end].iter().map(|l| l.split_whitespace().count()).sum()
    }

    /// The items of a section: every unindented non-blank line starts one; indented lines belong
    /// to the item above them.
    pub fn items(&self, s: &Section) -> Vec<Item> {
        let mut out: Vec<Item> = Vec::new();
        for i in s.start..s.end {
            let l = &self.lines[i];
            if l.trim().is_empty() {
                continue;
            }
            if self.fenced[i] {
                // A fenced block is never an item; under an item it belongs to it.
                if let Some(it) = out.last_mut() {
                    it.end = i + 1;
                    it.text.push(' ');
                    it.text.push_str(l.trim());
                }
                continue;
            }
            if l.starts_with(char::is_whitespace) {
                if let Some(it) = out.last_mut() {
                    it.end = i + 1;
                    it.text.push(' ');
                    it.text.push_str(l.trim());
                    continue;
                }
            }
            let (date, head) = match dated(l) {
                Some((d, rest)) => (Some(d), rest.trim().to_string()),
                None => (None, l.trim().to_string()),
            };
            out.push(Item { first: i, end: i + 1, date, text: head.clone(), head });
        }
        out
    }

    /// Every issue of the page, `[fix]` issues first, then `[review]` ones.
    pub fn issues(&self, today: NaiveDate) -> Vec<Issue> {
        let mut fix: Vec<String> = Vec::new();
        let mut review: Vec<String> = Vec::new();
        let list = SECTIONS.join(", ");

        let words = self.words();
        if words > PAGE_WORD_CAP {
            fix.push(format!(
                "the page is {words} words, over the {PAGE_WORD_CAP}-word cap: move finished items out (af session tidy moves old Done items to archive.md), cut what no longer matters, and shorten the long sections"
            ));
        }

        let mut last: Option<usize> = None;
        for s in &self.sections {
            match SECTIONS.iter().position(|n| *n == s.name) {
                None => fix.push(format!(
                    "heading \"## {}\" is not allowed: use only {list}; fold its content into one of them",
                    s.name
                )),
                Some(idx) => {
                    if last.is_some_and(|l| idx <= l) {
                        fix.push(format!(
                            "\"## {}\" is out of order or repeated: keep the sections in this order: {list}",
                            s.name
                        ));
                    } else {
                        last = Some(idx);
                    }
                }
            }
        }
        for r in REQUIRED {
            if self.section(r).is_none() {
                fix.push(format!("required section \"## {r}\" is missing: add it"));
            }
        }

        if let Some(s) = self.section("State") {
            let w = self.section_words(s);
            if w > STATE_WORD_CAP {
                review.push(format!(
                    "State is {w} words, over {STATE_WORD_CAP}: keep where the work is and move the rest to Doing, To do or Done"
                ));
            }
        }

        for name in ITEM_SECTIONS {
            let Some(s) = self.section(name) else { continue };
            let items = self.items(s);
            for it in &items {
                if it.date.is_none() {
                    fix.push(format!(
                        "{name}: item {} does not start with \"- YYYY-MM-DD · \": start the line with the date it was added",
                        quote(&it.head)
                    ));
                }
                if name == "Gated" && !has_waits_on(&it.text) {
                    fix.push(format!(
                        "Gated: item {} has no \"waits on: <condition>\": end it with \"· waits on: <condition>\", or move it to To do",
                        quote(&it.head)
                    ));
                }
            }
            if name == "Done" {
                if items.len() > DONE_MAX_ITEMS {
                    fix.push(format!(
                        "Done: {} items, more than {DONE_MAX_ITEMS}: run af session tidy to move the extra ones to archive.md",
                        items.len()
                    ));
                }
                for it in &items {
                    if let Some(d) = it.date {
                        let days = (today - d).num_days();
                        if days > DONE_MAX_DAYS {
                            fix.push(format!(
                                "Done: item {} is {days} days old, over {DONE_MAX_DAYS}: run af session tidy to move it to archive.md",
                                quote(&it.head)
                            ));
                        }
                    }
                }
            }
            let (limit, fix) = match name {
                "To do" => (TODO_MAX_DAYS, "do it, re-date it and say why it still stands, move it to Gated, or delete it"),
                "Gated" => (GATED_MAX_DAYS, "check what it waits on; re-date it or delete it"),
                "Rules" => (RULE_MAX_DAYS, "confirm the rule still applies and re-date it, or delete it"),
                _ => continue,
            };
            for it in &items {
                if let Some(d) = it.date {
                    let days = (today - d).num_days();
                    if days > limit {
                        review.push(format!(
                            "{name}: item {} is dated {d}, {days} days ago (over {limit}): {fix}",
                            quote(&it.head)
                        ));
                    }
                }
            }
        }

        fix.into_iter()
            .map(|text| Issue { kind: Kind::Fix, text })
            .chain(review.into_iter().map(|text| Issue { kind: Kind::Review, text }))
            .collect()
    }

    /// The frontmatter and the first lines of the body that fit in `cap` words, cut at a line end, and
    /// the number of words that are shown.
    pub fn cut(&self, cap: usize) -> (String, usize) {
        let mut out = self.front.clone();
        let mut used = 0usize;
        let mut kept: Vec<&str> = Vec::new();
        for l in &self.lines {
            let w = l.split_whitespace().count();
            if used + w > cap {
                break;
            }
            used += w;
            kept.push(l);
        }
        out.push_str(&kept.join("\n"));
        (out.trim_end().to_string(), used)
    }
}

/// `- YYYY-MM-DD <middle dot> text`: the date and the text.
fn dated(line: &str) -> Option<(NaiveDate, &str)> {
    let rest = line.strip_prefix("- ")?;
    let stamp = rest.get(..10)?;
    let text = rest[10..].strip_prefix(" \u{b7} ")?;
    Some((NaiveDate::parse_from_str(stamp, "%Y-%m-%d").ok()?, text))
}

/// Which lines sit inside a fenced code block (``` or ~~~, closed by the same mark at least as long);
/// the fence lines count as inside, and an unclosed fence runs to the end.
fn fence_lines(lines: &[String]) -> Vec<bool> {
    let mut out = vec![false; lines.len()];
    let mut open: Option<(char, usize)> = None;
    for (i, l) in lines.iter().enumerate() {
        let t = l.trim_start();
        let run = t.chars().next().filter(|c| *c == '`' || *c == '~').map(|c| (c, t.chars().take_while(|x| *x == c).count()));
        match (open, run) {
            (None, Some((c, n))) if n >= 3 => {
                open = Some((c, n));
                out[i] = true;
            }
            (Some((oc, on)), run) => {
                out[i] = true;
                if let Some((c, n)) = run {
                    if c == oc && n >= on && t[n..].trim().is_empty() {
                        open = None;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn has_waits_on(text: &str) -> bool {
    text.find("waits on:").is_some_and(|i| !text[i + "waits on:".len()..].trim().is_empty())
}

fn quote(head: &str) -> String {
    let mut chars = head.chars();
    let start: String = chars.by_ref().take(QUOTE_CHARS).collect();
    if chars.next().is_some() {
        format!("\"{start}...\"")
    } else {
        format!("\"{start}\"")
    }
}

/// The issues of a page's text.
pub fn issues_of(text: &str, today: NaiveDate) -> Vec<Issue> {
    Page::parse(text).issues(today)
}

// ------------------------------------------------------------------ tidy

/// A `Done` item taken off the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Moved {
    pub date: NaiveDate,
    /// Its lines as they stood, without line ends.
    pub lines: Vec<String>,
}

/// The page with the `Done` items beyond the ten newest by date, and those older than 14 days, taken out,
/// and the items taken. Every other byte of `raw` (line ends, the byte-order mark) is kept. An
/// item with no usable date stays: the archive files by month.
pub fn tidy_text(raw: &str, today: NaiveDate) -> (String, Vec<Moved>) {
    let (bom, text) = match raw.strip_prefix('\u{feff}') {
        Some(t) => ("\u{feff}", t),
        None => ("", raw),
    };
    let page = Page::parse(text);
    let mut moved: Vec<Moved> = Vec::new();
    let mut drop = vec![false; page.lines.len()];
    if let Some(done) = page.section("Done") {
        let items = page.items(done);
        // The newest ten by date stay (a tie keeps the page's order); an item with no date is never moved.
        let mut by_date: Vec<usize> = (0..items.len()).filter(|&n| items[n].date.is_some()).collect();
        by_date.sort_by(|&a, &b| items[b].date.cmp(&items[a].date));
        let mut keep = vec![false; items.len()];
        for &n in by_date.iter().take(DONE_MAX_ITEMS) {
            keep[n] = true;
        }
        for (n, it) in items.iter().enumerate() {
            let Some(date) = it.date else { continue };
            if !keep[n] || (today - date).num_days() > DONE_MAX_DAYS {
                for flag in &mut drop[it.first..it.end] {
                    *flag = true;
                }
                moved.push(Moved { date, lines: page.lines[it.first..it.end].to_vec() });
            }
        }
    }
    if moved.is_empty() {
        return (raw.to_string(), moved);
    }
    let mut out = String::from(bom);
    out.push_str(&text[..text.len() - page.body.len()]);
    for (i, seg) in page.body.split_inclusive('\n').enumerate() {
        if !drop[i] {
            out.push_str(seg);
        }
    }
    (out, moved)
}

/// The archive's text with the items appended: each under the `## YYYY-MM` heading of its date,
/// at the end of that month's lines, the heading added at the end of the file when missing. An item whose
/// lines already stand verbatim in that month's section is not added again.
pub fn archive_with(existing: Option<&str>, moved: &[Moved]) -> String {
    let existing = existing.unwrap_or("");
    let eol = if existing.contains("\r\n") { "\r\n" } else { "\n" };
    let mut segs: Vec<String> = existing.split_inclusive('\n').map(|s| s.to_string()).collect();
    if let Some(last) = segs.last_mut() {
        if !last.ends_with('\n') {
            last.push_str(eol);
        }
    }
    for m in moved {
        let heading = format!("## {}", m.date.format("%Y-%m"));
        let lines: Vec<String> = m.lines.iter().map(|l| format!("{l}{eol}")).collect();
        match segs.iter().position(|s| s.trim_end() == heading) {
            Some(h) => {
                let end = segs.iter().skip(h + 1).position(|s| s.starts_with("## ")).map_or(segs.len(), |p| h + 1 + p);
                // A re-run after a crash between the two writes finds its items already filed.
                let n = m.lines.len();
                let filed = end >= h + 1 + n
                    && (h + 1..=end - n).any(|i| (0..n).all(|k| segs[i + k].trim_end() == m.lines[k].trim_end()));
                if filed {
                    continue;
                }
                let at = (h + 1..end).rev().find(|&i| !segs[i].trim().is_empty()).map_or(h + 1, |i| i + 1);
                for (k, l) in lines.into_iter().enumerate() {
                    segs.insert(at + k, l);
                }
            }
            None => {
                if segs.last().is_some_and(|s| !s.trim().is_empty()) {
                    segs.push(eol.to_string());
                }
                segs.push(format!("{heading}{eol}"));
                segs.extend(lines);
            }
        }
    }
    segs.concat()
}

/// Write exactly this text, through a temp file and a rename: a failed write leaves the old file, never half of one.
fn write_raw(path: &str, text: &str) -> R<()> {
    TextFile { text: text.to_string(), crlf: false, bom: false }.save_atomic(path)
}

fn read_raw(path: &str) -> R<Option<String>> {
    if !pyos::isfile(path) {
        return Ok(None);
    }
    let bytes = std::fs::read(path).map_err(|e| pyos::os_error(&e, path))?;
    match String::from_utf8(bytes) {
        Ok(s) => Ok(Some(s)),
        Err(_) => die!("{path} is not UTF-8"),
    }
}

/// `af session tidy [<name>] [--check] [--all]`: print each page's issues; unless `--check`,
/// first move its finished `Done` items to `archive.md`. Exit 1 when any issue remains.
pub fn cmd_tidy(ctx: &Ctx, name: Option<&str>, check: bool, all: bool) -> R<i32> {
    let reg = Registry::load(ctx)?;
    let rows: Vec<Row> = if all {
        reg.rows().into_iter().filter(|r| !r.is_ended()).collect()
    } else if let Some(n) = name.filter(|n| !n.is_empty()) {
        match reg.find_name(n) {
            Some(r) => vec![r],
            None => die!("no registry row for {n}"),
        }
    } else {
        match crate::compact::caller_session().and_then(|id| reg.find_session(&id)) {
            Some(r) => vec![r],
            None => die!("af session tidy needs a chat: give its name, or run it from a registered chat (CLAUDE_CODE_SESSION_ID)"),
        }
    };
    let today = crate::core::today_date();
    let mut remaining = false;
    for row in rows {
        let rel = sessions::handoff_rel(&row.name);
        let path = vpath(ctx, &rel);
        let Some(raw) = read_raw(&path)? else {
            outln!("{rel}: no handoff yet");
            continue;
        };
        let (text, moved) = if check { (raw.clone(), Vec::new()) } else { tidy_text(&raw, today) };
        if !moved.is_empty() {
            let apath = vpath(ctx, &archive_rel(&row.name));
            let archive = archive_with(read_raw(&apath)?.as_deref(), &moved);
            // The archive first: a failure between the two writes repeats an item and never loses one.
            write_raw(&apath, &archive)?;
            write_raw(&path, &text)?;
        }
        let page = Page::parse(&text);
        outln!("{rel}: {} of {PAGE_WORD_CAP} words", page.words());
        if !moved.is_empty() {
            outln!("  moved {} Done item(s) to {}", moved.len(), archive_rel(&row.name));
        }
        let issues = page.issues(today);
        if issues.is_empty() {
            outln!("  clean");
        }
        for i in &issues {
            let kind = match i.kind {
                Kind::Fix => "fix",
                Kind::Review => "review",
            };
            outln!("  [{kind}] {}", i.text);
        }
        remaining |= !issues.is_empty();
    }
    Ok(if remaining { 1 } else { 0 })
}

// ------------------------------------------------------------------ where the checks show

/// The block a chat gets at session start: every issue, and whether the page needs tidying first (a `[fix]`
/// issue) or only has items to review.
pub fn tidy_block(issues: &[Issue]) -> String {
    let lines: Vec<String> = issues.iter().map(|i| format!("- {}", i.text)).collect();
    let head = if issues.iter().any(|i| i.kind == Kind::Fix) {
        "af: your page needs tidying before other work:"
    } else {
        "af: your page has items to review when this task allows:"
    };
    format!("{head}\n{}", lines.join("\n"))
}

/// Doctor: one note per issue of every chat whose row has not ended.
pub fn doctor_notes(ctx: &Ctx) -> R<Vec<String>> {
    let today = crate::core::today_date();
    let mut notes = Vec::new();
    for row in Registry::load(ctx)?.rows().iter().filter(|r| !r.is_ended()) {
        let rel = sessions::handoff_rel(&row.name);
        let Some(text) = read_raw(&vpath(ctx, &rel))? else { continue };
        for i in issues_of(&text, today) {
            notes.push(format!("{rel}: {}", i.text));
        }
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    const TODAY: &str = "2026-11-14";

    fn ago(n: i64) -> String {
        (d(TODAY) - chrono::Duration::days(n)).format("%Y-%m-%d").to_string()
    }

    fn page(extra: &str) -> String {
        format!("---\nwritten: 2026-11-14T10:00\nsession_id: s1\n---\n## State\nwhere it is\n\n{extra}## Next step\nship it\n\n## Files touched\n- system/backlog.md\n")
    }

    fn texts(p: &str) -> Vec<String> {
        issues_of(p, d(TODAY)).into_iter().map(|i| i.text).collect()
    }

    fn has(p: &str, needle: &str) -> bool {
        texts(p).iter().any(|t| t.contains(needle))
    }

    fn words(n: usize) -> String {
        vec!["w"; n].join(" ")
    }

    #[test]
    fn parses_sections_items_and_continuations() {
        let p = Page::parse(&page("## Doing\n- 2026-11-10 · build it\n  more about it\n    and more\n- 2026-11-11 · second\n\n"));
        let names: Vec<&str> = p.sections.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["State", "Doing", "Next step", "Files touched"]);
        let items = p.items(p.section("Doing").unwrap());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].date, Some(d("2026-11-10")));
        assert_eq!(items[0].head, "build it");
        assert_eq!(items[0].text, "build it more about it and more");
        assert_eq!(items[0].end - items[0].first, 3);
        assert_eq!(items[1].end - items[1].first, 1);
        assert!(p.front.starts_with("---\nwritten:") && p.front.ends_with("---\n"));
    }

    #[test]
    fn the_word_count_leaves_the_frontmatter_out() {
        let p = Page::parse("---\nwritten: 2026-11-14T10:00\nsession_id: s1\n---\none two\nthree\n");
        assert_eq!(p.words(), 3);
        assert_eq!(Page::parse("one two\n").words(), 2, "no frontmatter: the whole text is the body");
    }

    #[test]
    fn a_page_with_the_four_original_sections_is_clean() {
        let old = "---\nwritten: 2026-11-14T10:00\n---\n## State\nmid-build\n## Open questions\nnone\n## Next step\ntest\n## Files touched\n- a.md\n";
        assert_eq!(texts(old), Vec::<String>::new());
        assert_eq!(texts(&page("")), Vec::<String>::new());
    }

    #[test]
    fn issue_1_the_body_cap_is_1200_words() {
        let at = format!("---\nwritten: x\n---\n## State\n{}\n## Next step\nn\n## Files touched\nf\n", words(1190));
        assert_eq!(Page::parse(&at).words(), 1200);
        assert!(!has(&at, "word cap"), "{:?}", texts(&at));
        let over = at.replace("\nn\n", "\nn n\n");
        assert_eq!(Page::parse(&over).words(), 1201);
        assert!(has(&over, "1201 words, over the 1200-word cap"));
        let frontmatter_heavy = at.replace("written: x", &format!("written: {}", words(500)));
        assert!(!has(&frontmatter_heavy, "word cap"), "the frontmatter is not counted");
    }

    #[test]
    fn issue_2_headings_required_sections_and_order() {
        assert!(has(&page("## Notes\nx\n"), "heading \"## Notes\" is not allowed"));
        let no_next = "---\nwritten: x\n---\n## State\ns\n## Files touched\nf\n";
        assert!(has(no_next, "required section \"## Next step\" is missing"));
        assert!(!has(no_next, "\"## State\" is missing"));
        let no_state = "## Next step\nn\n## Files touched\nf\n";
        assert!(has(no_state, "required section \"## State\" is missing"));
        let no_files = "## State\ns\n## Next step\nn\n";
        assert!(has(no_files, "required section \"## Files touched\" is missing"));
        let swapped = "## State\ns\n## Files touched\nf\n## Next step\nn\n";
        assert!(has(swapped, "\"## Next step\" is out of order or repeated"));
        let twice = page("## Doing\n- 2026-11-14 · a\n## Doing\n- 2026-11-14 · b\n");
        assert!(has(&twice, "\"## Doing\" is out of order or repeated"));
        let late_optional = "## State\ns\n## Next step\nn\n## Files touched\nf\n## Done\n- 2026-11-14 · a\n";
        assert!(has(late_optional, "\"## Done\" is out of order"));
        assert_eq!(texts(&page("## Doing\n- 2026-11-14 · a\n## To do\n- 2026-11-14 · b\n")), Vec::<String>::new());
    }

    #[test]
    fn issue_3_item_lines_need_a_date() {
        for sec in ITEM_SECTIONS {
            let extra = if sec == "Gated" { " · waits on: x" } else { "" };
            let ok = page(&format!("## {sec}\n- {} · fine{extra}\n", ago(1)));
            assert!(!has(&ok, "does not start with"), "{sec}: {:?}", texts(&ok));
            for bad in ["- no date here", "no dash", "- 2026-13-40 · not a date", "- 2026-11-14 - wrong dot"] {
                let p = page(&format!("## {sec}\n{bad}{extra}\n"));
                assert!(has(&p, &format!("{sec}: item \"{bad}")), "{sec} {bad}: {:?}", texts(&p));
            }
        }
        let cont = page("## Doing\n- 2026-11-14 · a\n  indented line belongs to it\n");
        assert!(!has(&cont, "does not start with"));
        let orphan = page("## Doing\n  indented with nothing above\n");
        assert!(has(&orphan, "does not start with"));
        let free = page("## Open questions\nanything goes\n");
        assert!(!has(&free, "does not start with"), "Open questions is free text");
    }

    #[test]
    fn the_quote_is_forty_characters() {
        let long = "x".repeat(41);
        let p = page(&format!("## Doing\n- bad {long}\n"));
        let t = texts(&p).into_iter().find(|t| t.starts_with("Doing:")).unwrap();
        assert!(t.contains(&format!("\"- bad {}...\"", "x".repeat(34))), "{t}");
        let exact = "y".repeat(40);
        let p = page(&format!("## Doing\n{exact}\n"));
        let t = texts(&p).into_iter().find(|t| t.starts_with("Doing:")).unwrap();
        assert!(t.contains(&format!("\"{exact}\"")), "exactly 40 characters is quoted whole: {t}");
    }

    #[test]
    fn issue_4_a_gated_item_names_what_it_waits_on() {
        assert!(!has(&page(&format!("## Gated\n- {} · call him · waits on: his reply\n", ago(1))), "waits on"));
        assert!(has(&page(&format!("## Gated\n- {} · call him\n", ago(1))), "Gated: item \"call him\" has no \"waits on:"));
        assert!(has(&page(&format!("## Gated\n- {} · call him · waits on:\n", ago(1))), "has no \"waits on:"));
        let cont = page(&format!("## Gated\n- {} · call him\n  · waits on: his reply\n", ago(1)));
        assert!(!has(&cont, "has no \"waits on:"), "a continuation line counts");
        assert!(!has(&page(&format!("## Doing\n- {} · no wait needed\n", ago(1))), "waits on"));
    }

    #[test]
    fn issue_5_done_holds_ten_items_none_older_than_fourteen_days() {
        let items = |n: usize, age: i64| (0..n).map(|i| format!("- {} · item {i}\n", ago(age))).collect::<String>();
        let ten = page(&format!("## Done\n{}", items(10, 1)));
        assert!(!has(&ten, "Done:"), "{:?}", texts(&ten));
        let eleven = page(&format!("## Done\n{}", items(11, 1)));
        assert!(has(&eleven, "Done: 11 items, more than 10"));
        let at_14 = page(&format!("## Done\n{}", items(1, 14)));
        assert!(!has(&at_14, "Done:"), "exactly 14 days old stays");
        let at_15 = page(&format!("## Done\n{}", items(1, 15)));
        assert!(has(&at_15, "Done: item \"item 0\" is 15 days old, over 14"));
        assert!(issues_of(&at_15, d(TODAY)).iter().all(|i| i.kind == Kind::Fix));
    }

    #[test]
    fn issue_6_state_is_150_words_and_is_a_review_issue() {
        let with_state = |n: usize| format!("## State\n{}\n## Next step\nn\n## Files touched\nf\n", words(n));
        assert_eq!(texts(&with_state(150)), Vec::<String>::new());
        assert!(has(&with_state(151), "State is 151 words, over 150"));
        let over = issues_of(&with_state(151), d(TODAY));
        assert!(over.iter().all(|i| i.kind == Kind::Review), "a long State never blocks a stop: {over:?}");
    }

    #[test]
    fn issues_7_to_9_are_review_issues_at_21_45_and_30_days() {
        let cases = [("To do", 21, "do it, re-date it"), ("Gated", 45, "check what it waits on"), ("Rules", 30, "confirm the rule still applies")];
        for (sec, limit, fix) in cases {
            let wait = if sec == "Gated" { " · waits on: x" } else { "" };
            let p = |age: i64| page(&format!("## {sec}\n- {} · the item{wait}\n", ago(age)));
            assert_eq!(texts(&p(limit)), Vec::<String>::new(), "{sec} at exactly {limit}");
            let over = issues_of(&p(limit + 1), d(TODAY));
            assert_eq!(over.len(), 1, "{sec}: {over:?}");
            assert_eq!(over[0].kind, Kind::Review);
            assert!(over[0].text.starts_with(&format!("{sec}: item \"the item")), "{}", over[0].text);
            assert!(over[0].text.contains(&format!("{} days ago (over {limit})", limit + 1)) && over[0].text.contains(fix), "{}", over[0].text);
        }
        let doing = page(&format!("## Doing\n- {} · old but in progress\n", ago(400)));
        assert_eq!(texts(&doing), Vec::<String>::new(), "Doing has no age limit");
    }

    #[test]
    fn fix_issues_come_before_review_issues() {
        let p = page(&format!("## To do\n- {} · stale\n## Notes\nx\n", ago(30)));
        let kinds: Vec<Kind> = issues_of(&p, d(TODAY)).iter().map(|i| i.kind).collect();
        assert_eq!(kinds, [Kind::Fix, Kind::Review]);
    }

    #[test]
    fn tidy_moves_the_eleventh_and_the_old_and_nothing_else() {
        let mut done = String::new();
        for i in 0..12 {
            done.push_str(&format!("- {} · item {i}\n", ago(1)));
        }
        done.push_str(&format!("- {} · ancient\n  with a second line\n", ago(15)));
        let raw = page(&format!("## Doing\n- {} · keep me\n\n## Done\n{done}\n", ago(2)));
        let (out, moved) = tidy_text(&raw, d(TODAY));
        let names: Vec<&str> = moved.iter().map(|m| m.lines[0].rsplit(" · ").next().unwrap()).collect();
        assert_eq!(names, ["item 10", "item 11", "ancient"]);
        assert_eq!(moved[2].lines, [format!("- {} · ancient", ago(15)), "  with a second line".to_string()]);
        assert!(out.contains("item 9\n") && !out.contains("item 10") && !out.contains("ancient") && !out.contains("second line"));
        assert!(out.contains("keep me") && out.starts_with("---\nwritten: 2026-11-14T10:00\n"));
        assert_eq!(Page::parse(&out).items(Page::parse(&out).section("Done").unwrap()).len(), 10);
        let (again, nothing) = tidy_text(&out, d(TODAY));
        assert!(nothing.is_empty() && again == out, "a tidy page is a fixed point");
    }

    #[test]
    fn tidy_keeps_every_other_byte_crlf_and_bom_included() {
        let lf = page(&format!("## Done\n- {} · old\n- {} · new\n", ago(20), ago(1)));
        let crlf = format!("\u{feff}{}", lf.replace('\n', "\r\n"));
        let (out, moved) = tidy_text(&crlf, d(TODAY));
        assert_eq!(moved.len(), 1);
        let expected = format!("\u{feff}{}", page(&format!("## Done\n- {} · new\n", ago(1))).replace('\n', "\r\n"));
        assert_eq!(out, expected);
        let (out, moved) = tidy_text(&lf, d(TODAY));
        assert_eq!(moved.len(), 1);
        assert_eq!(out, page(&format!("## Done\n- {} · new\n", ago(1))));
    }

    #[test]
    fn tidy_leaves_an_item_with_no_date_where_it_is() {
        let dated = |n: usize| (0..n).map(|i| format!("- {} · n{i}\n", ago(1))).collect::<String>();
        let raw = page(&format!("## Done\n- undated\n{}", dated(11)));
        let (out, moved) = tidy_text(&raw, d(TODAY));
        assert_eq!(moved.len(), 1, "the undated item takes no one's place among the ten newest");
        assert_eq!(moved[0].lines, [format!("- {} · n10", ago(1))]);
        assert!(out.contains("- undated\n") && !out.contains("n10"));
        let old_undated = page(&format!("## Done\n- 2026-01-01 no dot\n- {} · old\n", ago(30)));
        let (out, moved) = tidy_text(&old_undated, d(TODAY));
        assert_eq!(moved.len(), 1);
        assert!(out.contains("no dot"), "an item with no usable date cannot be filed by month");
    }

    #[test]
    fn tidy_keeps_the_ten_newest_by_date_not_the_first_ten() {
        // Page order is not date order: the oldest items come first, and the newest one is last.
        let mut done = String::new();
        for i in 0..10 {
            done.push_str(&format!("- {} · old {i}\n", ago(10 - (i % 2))));
        }
        done.push_str(&format!("- {} · newest\n", ago(0)));
        let (out, moved) = tidy_text(&page(&format!("## Done\n{done}")), d(TODAY));
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].lines, [format!("- {} · old 8", ago(10))], "the oldest; of the ties the later on the page goes");
        assert!(out.contains("newest") && !out.contains("old 8\n") && out.contains("old 0\n"));
    }

    #[test]
    fn a_tie_keeps_the_pages_order() {
        let done: String = (0..12).map(|i| format!("- {} · same {i}\n", ago(3))).collect();
        let (_, moved) = tidy_text(&page(&format!("## Done\n{done}")), d(TODAY));
        let names: Vec<String> = moved.iter().map(|m| m.lines[0].rsplit(" · ").next().unwrap().to_string()).collect();
        assert_eq!(names, ["same 10", "same 11"]);
    }

    #[test]
    fn a_fenced_line_is_not_a_heading_not_an_item_and_not_the_end_of_a_section() {
        for fence in ["```", "~~~", "````"] {
            let p = page(&format!(
                "## Doing\n- {} · run this\n{fence}\n## Not a heading\n- not an item\n{fence}\n- {} · second\n",
                ago(1),
                ago(1)
            ));
            assert_eq!(texts(&p), Vec::<String>::new(), "{fence}: {:?}", texts(&p));
            let pg = Page::parse(&p);
            let names: Vec<&str> = pg.sections.iter().map(|s| s.name.as_str()).collect();
            assert_eq!(names, ["State", "Doing", "Next step", "Files touched"], "{fence}");
            let items = pg.items(pg.section("Doing").unwrap());
            assert_eq!(items.len(), 2, "{fence}");
            assert_eq!(items[0].end - items[0].first, 5, "the fenced block belongs to the item above it");
        }
        // Without the fence the same lines are a heading and an item with no date.
        let bare = page(&format!("## Doing\n- {} · run this\n## Not a heading\n- not an item\n", ago(1)));
        assert!(has(&bare, "\"## Not a heading\" is not allowed") || has(&bare, "heading \"## Not a heading\" is not allowed"));
        // A shorter mark does not close a fence; an unclosed one runs to the end.
        let open = page("## Doing\n````\n```\n## Still inside\n");
        assert!(!has(&open, "Still inside"));
        let unclosed = "## State\ns\n## Next step\nn\n## Files touched\nf\n```\n## Nothing after this counts\n";
        assert!(!has(unclosed, "not allowed"), "{:?}", texts(unclosed));
    }

    #[test]
    fn tidy_moves_a_fenced_block_with_its_item_and_ignores_fenced_headings() {
        let raw = page(&format!("## Done\n- {} · old\n```\n## Done\n- {} · fake\n```\n- {} · new\n", ago(30), ago(30), ago(1)));
        let (out, moved) = tidy_text(&raw, d(TODAY));
        assert_eq!(moved.len(), 1, "the fake item inside the fence is not an item");
        assert_eq!(moved[0].lines.len(), 5);
        assert!(!out.contains("fake") && out.contains("new"));
    }

    #[test]
    fn a_rerun_after_a_crash_does_not_file_an_item_twice() {
        let m = |date: &str, text: &str| Moved { date: d(date), lines: vec![format!("- {date} · {text}"), "  more".to_string()] };
        let once = archive_with(None, &[m("2026-10-30", "a")]);
        assert_eq!(archive_with(Some(&once), &[m("2026-10-30", "a")]), once);
        let two = archive_with(Some(&once), &[m("2026-10-30", "a"), m("2026-10-29", "b")]);
        assert_eq!(two.matches("· a").count(), 1);
        assert!(two.contains("· b"));
        let other_month = archive_with(Some(&once), &[m("2026-11-01", "a")]);
        assert_eq!(other_month.matches("· a").count(), 2, "another month is another section: not filed");
        let changed = archive_with(Some(&once), &[Moved { date: d("2026-10-30"), lines: vec!["- 2026-10-30 · a".into(), "  different".into()] }]);
        assert!(changed.contains("  different"), "only a verbatim copy counts as filed");
    }

    #[test]
    fn the_archive_files_by_month_and_appends() {
        let m = |date: &str, text: &str| Moved { date: d(date), lines: vec![format!("- {date} · {text}")] };
        let fresh = archive_with(None, &[m("2026-10-30", "a"), m("2026-10-02", "b"), m("2026-09-30", "c")]);
        assert_eq!(fresh, "## 2026-10\n- 2026-10-30 · a\n- 2026-10-02 · b\n\n## 2026-09\n- 2026-09-30 · c\n");
        let more = archive_with(Some(&fresh), &[m("2026-10-01", "d"), m("2026-11-01", "e")]);
        assert_eq!(
            more,
            "## 2026-10\n- 2026-10-30 · a\n- 2026-10-02 · b\n- 2026-10-01 · d\n\n## 2026-09\n- 2026-09-30 · c\n\n## 2026-11\n- 2026-11-01 · e\n"
        );
        let no_newline = archive_with(Some("## 2026-10\n- 2026-10-30 · a"), &[m("2026-10-01", "d")]);
        assert_eq!(no_newline, "## 2026-10\n- 2026-10-30 · a\n- 2026-10-01 · d\n");
        let crlf = archive_with(Some("## 2026-10\r\n- 2026-10-30 · a\r\n"), &[m("2026-10-01", "d"), m("2026-08-01", "x")]);
        assert_eq!(crlf, "## 2026-10\r\n- 2026-10-30 · a\r\n- 2026-10-01 · d\r\n\r\n## 2026-08\r\n- 2026-08-01 · x\r\n");
    }

    #[test]
    fn cut_keeps_whole_lines_within_the_cap() {
        let body: String = (0..30).map(|i| format!("{}\n", words(100 + i % 2))).collect();
        let p = Page::parse(&format!("---\nwritten: x\n---\n{body}"));
        let (cut, shown) = p.cut(1200);
        assert!(cut.starts_with("---\nwritten: x\n---\n"));
        let w = Page::parse(&cut).words();
        assert!(w <= 1200 && w > 1100, "{w}");
        assert_eq!(shown, w, "the count returned is the words shown");
        assert!(cut.ends_with('w'), "cut at a line end, trailing newline trimmed");
        for l in cut.lines().skip(3) {
            assert!(l.split_whitespace().count() >= 100, "no line is cut in half");
        }
    }
}
