//! The deterministic backstop for the zero-budget voice rules.
//!
//! Offsets are bytes throughout; where the Python's arithmetic depends on code-point
//! counts (snippet windows, the sentence-to-line mapping) the count is taken in chars.

use crate::error::R;
use crate::fm::rx;
use crate::pyjson::{float_repr, round2, PyVal};
use crate::pyos::{self, py_isspace, py_split_ws, py_strip};
use fancy_regex::Regex as FRegex;
use regex::Regex;
use std::collections::HashSet;
use std::sync::OnceLock;

pub const DASH_RATE_ALARM: f64 = 3.0;
pub const SPACED_HYPHEN: &str = " - ";

// Python's `\s` (str.isspace) is the Unicode White_Space set plus U+001C..U+001F.
const WS: &str = r"[\s\x1c-\x1f]";

fn f_spans(re: &FRegex, text: &str) -> Vec<(usize, usize)> {
    re.find_iter(text).filter_map(|m| m.ok()).map(|m| (m.start(), m.end())).collect()
}

fn dash_count(text: &str) -> usize {
    f_spans(&rxs().dash, text).len()
}

/// `QUOTED.finditer(text)` for `"[^"\n]{1,400}"`, scanned by hand: the counted repetition
/// of a Unicode class costs the regex crate about 4 ms to compile, a hook's whole budget.
fn quoted_spans(text: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(off) = text[from..].find('"') {
        let s = from + off;
        let mut n = 0usize;
        let mut end = None;
        for (k, c) in text[s + 1..].char_indices() {
            if c == '"' {
                if n >= 1 {
                    end = Some(s + 1 + k + 1);
                }
                break;
            }
            if c == '\n' || n == 400 {
                break;
            }
            n += 1;
        }
        match end {
            Some(e) => {
                out.push((s, e));
                from = e;
            }
            None => from = s + 1,
        }
    }
    out
}

const WHERE_LANDS: &str = r"(?i)\bwhere (?:this|that|it|I|we|you|things?) (?:actually |really )?lands?\b";
const BANNED_TICS: [(&str, &str); 3] = [
    ("my read", r"(?i)\bmy (?:honest )?read\b"),
    ("quietly", r"(?i)\bquietly\b"),
    ("where this lands", WHERE_LANDS),
];
const LANDS: &str = r"(?i)\b(?:lands?|landed)\b(?! pages?\b)";
const REACH_FOR: &str = r"(?i)\breach(?:ed|es|ing)? for\b";
const SHIP_MODEL: &str = r"(?i)\b(?:models?|GPT-\d|Claude|Gemini|Astra|Fable|Mythos|Opus|Sonnet|Haiku|Llama|releases?)\b[^.!?\n]{0,80}?\bship(?:s|ped|ping)?\b|\bship(?:s|ped|ping)?\b[^.!?\n]{0,80}?\b(?:models?|GPT-\d|Claude|Gemini|Astra|Fable|Mythos|Opus|Sonnet|Haiku|Llama)\b";
const ACT_REF: &str = r"\b(?:in|from|after|before|to|of|see|until|later in|back in|earlier in) Act (?:I{1,3}|IV|V|\d+)\b";
const LITOTES: &str = r"(?i)\b(?:not un\w+|not im(?:possible|probable)\w*|not without\b|no small\b|no stranger to\b|no accident\b|hardly (?:surprising|a surprise|new)\b|scarcely\b|not (?:bad|terrible|the worst)\b)";
const NEGATION: &str = r"(?:is|was|are|were|does|do|did|wo|ca)n['’]t|is not|was not|are not|does not|do not|did not|will not|cannot";
const CONTRAST_NOT: &str = r"(?i), not (?:a|an|the|just|only|your|my|our|their)\b";
const NEGATED_COPULA: &str = r"(?i)\b(?:is|was|are|were)n['’]t\b|\b(?:is|was|are|were) not\b|['’](?:s|re) not\b";
const SUBORDINATOR: &str = r"(?i)\b(?:if|when|whenever|unless|although|though|because|since|while|whether|once|until)\b";
const PIVOT_OPEN: &str = r"\A(?:It|That|This|They)(?:['’]s|['’]re| is| are| was| were)\b";
const SUBORDINATE: &str = r"(?i)\A\W*(?:if|when|whenever|unless|although|though|because|since|while|whether|once|until|as long as|even if)\b";
const THREAD: &str = r"(?i)(?<!\bemail )(?<!\bcomment )(?<!\bX )(?<!\bTwitter )(?<!\bSlack )(?<!\bReddit )(?<!\bforum )(?<!\bDiscord )(?<!\bmessage )\bthreads?\b(?! the needle)";

fn contrast_one_pat() -> String {
    format!(
        r"(?i)\b(?:{NEGATION})\b[^.!?;]{{1,80}}?[,;] (?:instead |rather )?(?:it|they|that|this)(?:['’]s|['’]re| is| are| was| were| \w+s)\b"
    )
}

/// The lint's patterns, compiled once per process. Compiling them takes about 15 ms of
/// CPU (Unicode classes under counted repetition), so the groups compile on parallel
/// threads; the patterns themselves are the Python's, unchanged.
struct Rx {
    tics: Vec<Regex>,
    where_lands: Regex,
    para: Regex,
    reach_for: Regex,
    ship_model: Regex,
    act_ref: Regex,
    litotes: Regex,
    contrast_one: Regex,
    contrast_not: Regex,
    negated_copula: Regex,
    subordinator: Regex,
    pivot_open: Regex,
    subordinate: Regex,
    dash: FRegex,
    split: FRegex,
    lands: FRegex,
    thread: FRegex,
}

static RX: OnceLock<Rx> = OnceLock::new();

fn re(p: &str) -> Regex {
    Regex::new(p).unwrap_or_else(|e| panic!("bad regex {p}: {e}"))
}

fn fre(p: &str) -> FRegex {
    FRegex::new(p).unwrap_or_else(|e| panic!("bad regex {p}: {e}"))
}

fn rxs() -> &'static Rx {
    RX.get_or_init(|| {
        std::thread::scope(|s| {
            let a = s.spawn(|| (re(SHIP_MODEL), re(PIVOT_OPEN), re(CONTRAST_NOT)));
            let b = s.spawn(|| (re(&contrast_one_pat()), re(REACH_FOR)));
            let c = s.spawn(|| (re(LITOTES), re(ACT_REF), re(NEGATED_COPULA)));
            let d = s.spawn(|| (re(SUBORDINATOR), re(SUBORDINATE), re(WHERE_LANDS)));
            let e = s.spawn(|| {
                (
                    fre(&format!(r"—|(?<={WS})–(?={WS})")),
                    fre(&format!(
                        r#"(?<=[.!?]){WS}+(?=["'(\[A-Z0-9*])|(?<=[.!?]["')\]]){WS}+(?=["'(\[A-Z0-9*])"#
                    )),
                    fre(LANDS),
                    fre(THREAD),
                )
            });
            let tics: Vec<Regex> = BANNED_TICS.iter().map(|(_, p)| re(p)).collect();
            let para = re(r"(?:[^\n]+\n?)+");
            let (ship_model, pivot_open, contrast_not) = a.join().unwrap();
            let (contrast_one, reach_for) = b.join().unwrap();
            let (litotes, act_ref, negated_copula) = c.join().unwrap();
            let (subordinator, subordinate, where_lands) = d.join().unwrap();
            let (dash, split, lands, thread) = e.join().unwrap();
            Rx {
                tics,
                where_lands,
                para,
                reach_for,
                ship_model,
                act_ref,
                litotes,
                contrast_one,
                contrast_not,
                negated_copula,
                subordinator,
                pivot_open,
                subordinate,
                dash,
                split,
                lands,
                thread,
            }
        })
    })
}

// ---------------------------------------------------------------- text shaping

fn lstrip_bom(text: &str) -> &str {
    text.trim_start_matches('\u{feff}')
}

/// (frontmatter, body); frontmatter is '' when the file has none.
pub fn split_frontmatter(text: &str) -> (String, String) {
    let text = lstrip_bom(text);
    let re = rx(r"(?s)\A---\r?\n(.*?)\r?\n---\r?\n?");
    match re.captures(text) {
        None => (String::new(), text.to_string()),
        Some(c) => (c[1].to_string(), text[c.get(0).unwrap().end()..].to_string()),
    }
}

pub fn is_user_voiced(frontmatter: &str) -> bool {
    rx(r"(?m)^(?:voice|register):").is_match(frontmatter)
}

fn blank_keeping_lines(re: &regex::Regex, text: &str) -> String {
    re.replace_all(text, |c: &regex::Captures| "\n".repeat(c[0].matches('\n').count())).into_owned()
}

/// Body with comments, code, images, tables and rules blanked; line count preserved.
pub fn prose(body: &str) -> String {
    let text = body.replace("\r\n", "\n");
    let text = blank_keeping_lines(&rx(r"(?s)<!--.*?-->"), &text);
    let text = blank_keeping_lines(&rx(r"(?s)```.*?```"), &text);
    let rule = rx(r"\A(?:-{3,}|\*{3,}|_{3,})\z");
    text.split('\n')
        .map(|line| {
            let s = py_strip(line);
            if s.starts_with("![") || s.starts_with('|') || rule.is_match(s) {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `re.split(SENTENCE_SPLIT, flat)`
fn split_sentences(flat: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut last = 0;
    for (s, e) in f_spans(&rxs().split, flat) {
        out.push(&flat[last..s]);
        last = e;
    }
    out.push(&flat[last..]);
    out
}

/// Per paragraph, its (sentence, 1-based line within `text`) pairs; heading lines excluded.
pub fn paragraph_sentences(text: &str) -> Vec<Vec<(String, usize)>> {
    let mut paras = Vec::new();
    for m in rxs().para.find_iter(text) {
        let block = m.as_str();
        if py_strip(block).is_empty() {
            continue;
        }
        let block_line = text[..m.start()].matches('\n').count() + 1;
        let flat_full = block.replace('\n', " ");
        let flat = py_strip(&flat_full);
        let mut cursor = 0usize;
        let mut out = Vec::new();
        for part in split_sentences(flat) {
            let part = py_strip(part);
            if part.is_empty() {
                continue;
            }
            let pos = flat.get(cursor..).and_then(|rest| rest.find(part)).map(|i| i + cursor);
            let line = match pos {
                Some(p) => {
                    cursor = p + part.len();
                    // `block[:pos]` with pos counted in code points of the stripped flat text
                    let nchars = flat[..p].chars().count();
                    block_line + block.chars().take(nchars).filter(|&c| c == '\n').count()
                }
                None => block_line,
            };
            if !part.starts_with('#') {
                out.push((part.to_string(), line));
            }
        }
        if !out.is_empty() {
            paras.push(out);
        }
    }
    paras
}

pub fn sentences(text: &str) -> Vec<(String, usize)> {
    paragraph_sentences(text).into_iter().flatten().collect()
}

fn quoted_blank(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut last = 0;
    for (a, b) in quoted_spans(s) {
        out.push_str(&s[last..a]);
        out.push_str("\"\"");
        last = b;
    }
    out.push_str(&s[last..]);
    out
}

fn py_rstrip(s: &str) -> &str {
    s.trim_end_matches(py_isspace)
}

/// (line, sentence, closes_paragraph) for each contrastive pivot outside quotation marks.
pub fn contrastive_pivots(text: &str) -> Vec<(usize, String, bool)> {
    let mut found = Vec::new();
    let r = rxs();
    for para in paragraph_sentences(text) {
        let last = para.len() - 1;
        for (i, (sent, line)) in para.iter().enumerate() {
            let bare = quoted_blank(sent);
            let subordinate = r.subordinate.is_match(&bare);
            let mut one = if subordinate { None } else { r.contrast_one.find(&bare) };
            if let Some(m) = one {
                let from = bare[..m.start()].rfind(',').map(|i| i + 1).unwrap_or(0);
                if r.subordinator.is_match(&bare[from..m.start()]) {
                    one = None;
                }
            }
            if one.is_some() || r.contrast_not.is_match(&bare) {
                found.push((*line, sent.clone(), i == last));
                continue;
            }
            if i < last && !subordinate && !py_rstrip(&bare).ends_with('?') && r.negated_copula.is_match(&bare) {
                let (nxt, nxt_line) = &para[i + 1];
                if r.pivot_open.is_match(&quoted_blank(nxt)) {
                    found.push((*nxt_line, format!("{sent} {nxt}"), i + 1 == last));
                }
            }
        }
    }
    found
}

fn norm(s: &str) -> String {
    py_split_ws(s).join(" ")
}

/// `_snippet(text, start, end, width)` with a window counted in code points.
pub fn snippet(text: &str, start: usize, end: usize, width: usize) -> String {
    let total = text.chars().count();
    let cs = text[..start].chars().count();
    let ce = text[..end].chars().count();
    let lo = cs.saturating_sub(width);
    let hi = (ce + width).min(total);
    let mid: String = text.chars().skip(lo).take(hi - lo).collect::<String>().replace('\n', " ");
    format!("{}{}{}", if lo > 0 { "…" } else { "" }, py_strip(&mid), if hi < total { "…" } else { "" })
}

fn whole(s: &str) -> String {
    snippet(s, 0, s.len(), 90)
}

fn inside(spans: &[(usize, usize)], pos: usize) -> bool {
    spans.iter().any(|&(a, b)| a <= pos && pos < b)
}

// ---------------------------------------------------------------- the lint

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub code: &'static str,
    pub line: usize,
    pub message: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Default)]
pub struct LintResult {
    pub user_voiced: bool,
    pub hard: Vec<Finding>,
    pub soft: Vec<Finding>,
    /// (key, value) in the Python's insertion order.
    pub stats: Vec<(&'static str, PyVal)>,
    pub path: Option<String>,
    pub prev: Option<String>,
}

impl LintResult {
    pub fn hard_codes(&self) -> Vec<&'static str> {
        self.hard.iter().map(|f| f.code).collect()
    }
    pub fn soft_codes(&self) -> Vec<&'static str> {
        self.soft.iter().map(|f| f.code).collect()
    }
    pub fn stat(&self, key: &str) -> Option<&PyVal> {
        self.stats.iter().find(|(k, _)| *k == key).map(|(_, v)| v)
    }
    fn set_stat(&mut self, key: &'static str, v: PyVal) {
        if let Some(e) = self.stats.iter_mut().find(|(k, _)| *k == key) {
            e.1 = v;
        } else {
            self.stats.push((key, v));
        }
    }
}

pub fn lint(text: &str, prev_text: Option<&str>) -> LintResult {
    let (fm, body) = split_frontmatter(text);
    let mut result = LintResult { user_voiced: is_user_voiced(&fm), ..Default::default() };
    if !result.user_voiced {
        return result;
    }
    let stripped = lstrip_bom(text);
    let offset = stripped[..stripped.len() - body.len()].matches('\n').count();
    let p = prose(&body);
    let r = rxs();
    let quoted: Vec<(usize, usize)> = quoted_spans(&p);
    let line_of = |pos: usize| offset + p[..pos].matches('\n').count() + 1;
    let mut hard: Vec<Finding> = Vec::new();
    let mut soft: Vec<Finding> = Vec::new();
    let mk = |code: &'static str, line: usize, message: String, snippet: String| Finding { code, line, message, snippet };

    for ((name, _), tic) in BANNED_TICS.iter().zip(&r.tics) {
        for m in tic.find_iter(&p) {
            if inside(&quoted, m.start()) {
                continue;
            }
            hard.push(mk(
                "banned-tic",
                line_of(m.start()),
                format!("\"{name}\" is a banned tic (anti-patterns.md § Banned tics)"),
                snippet(&p, m.start(), m.end(), 60),
            ));
        }
    }

    let sents = sentences(&p);
    let mut prev_had_dash = false;
    for (sent, line) in &sents {
        let n = dash_count(sent);
        if n >= 2 {
            hard.push(mk(
                "double-dash",
                offset + line,
                format!("{n} em dashes in one sentence (always wrong: commas or parentheses own the aside)"),
                whole(sent),
            ));
        }
        if n > 0 && prev_had_dash {
            soft.push(mk("dash-consecutive", offset + line, "em dash in each of two consecutive sentences".into(), whole(sent)));
        }
        prev_had_dash = n > 0;
    }

    for (s, e) in f_spans(&r.thread, &p) {
        if !inside(&quoted, s) {
            soft.push(mk(
                "thread-as-topic",
                line_of(s),
                "\"thread\" for a subject; name the subject instead (literal threads are fine)".into(),
                snippet(&p, s, e, 60),
            ));
        }
    }
    for m in r.litotes.find_iter(&p) {
        if !inside(&quoted, m.start()) {
            soft.push(mk(
                "litotes",
                line_of(m.start()),
                "litotes shape; write the affirmative or hedge openly".into(),
                snippet(&p, m.start(), m.end(), 60),
            ));
        }
    }
    let where_spans: Vec<(usize, usize)> = r.where_lands.find_iter(&p).map(|m| (m.start(), m.end())).collect();
    for (s, e) in f_spans(&r.lands, &p) {
        if !inside(&quoted, s) && !inside(&where_spans, s) {
            soft.push(mk(
                "lands-as-arrival",
                line_of(s),
                "\"lands\" for a conclusion or an arrival; say the conclusion (a literal landing stays)".into(),
                snippet(&p, s, e, 60),
            ));
        }
    }
    for m in r.reach_for.find_iter(&p) {
        if !inside(&quoted, m.start()) {
            soft.push(mk(
                "reach-for",
                line_of(m.start()),
                "\"reach for\" standing in for said or chose; say the word".into(),
                snippet(&p, m.start(), m.end(), 60),
            ));
        }
    }
    for m in r.ship_model.find_iter(&p) {
        if !inside(&quoted, m.start()) {
            soft.push(mk(
                "ship-for-release",
                line_of(m.start()),
                "\"ship\" for a model release; he writes launches / is released".into(),
                snippet(&p, m.start(), m.end(), 60),
            ));
        }
    }
    for m in r.act_ref.find_iter(&p) {
        let line_start = p[..m.start()].rfind('\n').map(|i| i + 1).unwrap_or(0);
        if p[line_start..m.start()].trim_start_matches(py_isspace).starts_with('#') {
            continue;
        }
        if !inside(&quoted, m.start()) {
            soft.push(mk(
                "act-cross-ref",
                line_of(m.start()),
                "act cross-reference in prose; the reader does not know what the act holds, so name the thing or cut the clause".into(),
                snippet(&p, m.start(), m.end(), 60),
            ));
        }
    }
    for (line, sent, closes) in contrastive_pivots(&p) {
        if closes {
            hard.push(mk(
                "contrastive-kicker",
                offset + line,
                "contrastive pivot closing a paragraph is an aphorism kicker; name the thing directly (anti-patterns.md § Budgeted moves)".into(),
                whole(&sent),
            ));
        } else {
            soft.push(mk(
                "contrastive",
                offset + line,
                "contrastive negation; zero in agent prose, keep it only if he typed it (anti-patterns.md § Budgeted moves)".into(),
                whole(&sent),
            ));
        }
    }

    let words = py_split_ws(&p).len();
    let dashes = dash_count(&p);
    let hyphens = p.matches(SPACED_HYPHEN).count();
    let rate = if words > 0 { dashes as f64 * 1000.0 / words as f64 } else { 0.0 };
    result.stats = vec![
        ("words", PyVal::Int(words as i64)),
        ("em_dashes", PyVal::Int(dashes as i64)),
        ("dash_rate_per_1k", PyVal::Float(round2(rate))),
        ("spaced_hyphens", PyVal::Int(hyphens as i64)),
    ];
    if rate > DASH_RATE_ALARM {
        soft.push(mk(
            "dash-rate",
            0,
            format!(
                "{rate:.1} em dashes per 1,000 words (drift alarm {DASH_RATE_ALARM:.0}); fuse or repunctuate, never split into periods"
            ),
            String::new(),
        ));
    }

    if let Some(prev_text) = prev_text {
        let (_, prev_body) = split_frontmatter(prev_text);
        let pp = prose(&prev_body);
        let head_norm: HashSet<String> = sents.iter().map(|(s, _)| norm(s)).collect();
        for (sent, _line) in sentences(&pp) {
            if !sent.contains(SPACED_HYPHEN) || head_norm.contains(&norm(&sent)) {
                continue;
            }
            let swapped = [
                sent.replace(SPACED_HYPHEN, " — "),
                sent.replace(SPACED_HYPHEN, "—"),
                sent.replace(SPACED_HYPHEN, " – "),
            ];
            if let Some(hit) = swapped.iter().map(|v| norm(v)).find(|v| head_norm.contains(v)) {
                let line = sents.iter().find(|(s, _)| norm(s) == hit).map(|(_, l)| *l).unwrap_or(0);
                hard.push(mk(
                    "hyphen-normalised",
                    offset + line,
                    "the previous version's spaced hyphen became an em dash; the operator's punctuation is not a typo (anti-patterns.md § Punctuation)".into(),
                    whole(&hit),
                ));
            }
        }
        let prev_hyphens = pp.matches(SPACED_HYPHEN).count();
        let prev_dashes = dash_count(&pp);
        if hyphens < prev_hyphens && dashes > prev_dashes {
            soft.push(mk(
                "hyphen-shift",
                0,
                format!(
                    "spaced hyphens {prev_hyphens} -> {hyphens} while em dashes {prev_dashes} -> {dashes} since the previous version"
                ),
                String::new(),
            ));
        }
        result.set_stat("prev_em_dashes", PyVal::Int(prev_dashes as i64));
        result.set_stat("prev_spaced_hyphens", PyVal::Int(prev_hyphens as i64));
    }
    result.hard = hard;
    result.soft = soft;
    result
}

// ---------------------------------------------------------------- files

/// `{name}-v{N-1}.md` beside `{name}-v{N}.md`, when N > 1 and it exists.
pub fn previous_version(path: &str) -> Option<String> {
    let (folder, base) = pyos::split(path);
    let c = rx(r"\A(.+)-v(\d+)\.md\z").captures(&base)?;
    let n: u64 = c[2].parse().ok()?;
    if n <= 1 {
        return None;
    }
    let candidate = pyos::join(&folder, &format!("{}-v{}.md", &c[1], n - 1));
    if pyos::isfile(&candidate) {
        Some(candidate)
    } else {
        None
    }
}

/// How `lint_file` finds the previous version.
#[derive(Debug, Clone, Copy)]
pub enum Prev<'a> {
    Auto,
    None,
    Path(&'a str),
}

/// Lint one head.
pub fn lint_file(path: &str, prev: Prev) -> R<LintResult> {
    let prev_path = match prev {
        Prev::Auto => previous_version(path),
        Prev::None => None,
        Prev::Path(p) if !p.is_empty() => Some(p.to_string()),
        Prev::Path(_) => None,
    };
    let prev_text = match &prev_path {
        Some(p) if pyos::isfile(p) => Some(pyos::read(p)?),
        _ => None,
    };
    let mut result = lint(&pyos::read(path)?, prev_text.as_deref());
    result.path = Some(path.to_string());
    result.prev = if prev_text.is_some() { prev_path } else { None };
    Ok(result)
}

pub fn format_report(result: &LintResult, root: &str) -> String {
    let path = result.path.clone().unwrap_or_else(|| "<text>".into());
    let rel = if pyos::isabs(&path) && path.starts_with(root) {
        pyos::relpath(&path, root).unwrap_or(path.clone())
    } else {
        path.clone()
    };
    if !result.user_voiced {
        return format!("voice lint: {rel} is not user-voiced (no voice:/register: in frontmatter); skipped");
    }
    let mut lines = vec![match &result.prev {
        Some(p) => format!("voice lint: {rel} vs {}", pyos::basename(p)),
        None => format!("voice lint: {rel} (no previous version to diff)"),
    }];
    if !result.hard.is_empty() {
        lines.push(format!("  HARD ({}, blocks af ready):", result.hard.len()));
        for f in &result.hard {
            lines.push(format!("    {:<18} L{}: {}", f.code, f.line, f.message));
            if !f.snippet.is_empty() {
                lines.push(format!("      > {}", f.snippet));
            }
        }
    }
    if !result.soft.is_empty() {
        lines.push(format!("  soft ({}, judge them):", result.soft.len()));
        for f in &result.soft {
            let where_ = if f.line != 0 { format!("L{}: ", f.line) } else { String::new() };
            lines.push(format!("    {:<18} {}{}", f.code, where_, f.message));
            if !f.snippet.is_empty() {
                lines.push(format!("      > {}", f.snippet));
            }
        }
    }
    let show = |k: &str| match result.stat(k) {
        Some(PyVal::Int(i)) => i.to_string(),
        Some(PyVal::Float(f)) => float_repr(*f),
        _ => "0".into(),
    };
    lines.push(format!(
        "  stats: {} words, {} em dashes ({}/1k), {} spaced hyphens",
        show("words"),
        show("em_dashes"),
        show("dash_rate_per_1k"),
        show("spaced_hyphens")
    ));
    if result.hard.is_empty() {
        lines.push("  ok: no hard findings".into());
    }
    lines.join("\n")
}

fn finding_val(f: &Finding) -> PyVal {
    PyVal::dict(vec![
        ("code", PyVal::s(f.code)),
        ("line", PyVal::Int(f.line as i64)),
        ("message", PyVal::s(f.message.clone())),
        ("snippet", PyVal::s(f.snippet.clone())),
    ])
}

/// The result dict as `json.dumps` sees it.
pub fn to_pyval(r: &LintResult) -> PyVal {
    let mut items = vec![
        ("user_voiced", PyVal::Bool(r.user_voiced)),
        ("hard", PyVal::List(r.hard.iter().map(finding_val).collect())),
        ("soft", PyVal::List(r.soft.iter().map(finding_val).collect())),
        ("stats", PyVal::Dict(r.stats.iter().map(|(k, v)| (k.to_string(), v.clone())).collect())),
    ];
    if let Some(p) = &r.path {
        items.push(("path", PyVal::s(p.clone())));
        items.push(("prev", r.prev.clone().map(PyVal::Str).unwrap_or(PyVal::None)));
    }
    PyVal::dict(items)
}
