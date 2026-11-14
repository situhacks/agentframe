//! The voice system concatenated into one file under
//! `system/index/voice/` so a drafting turn loads it in one read.

use crate::error::R;
use crate::fm::rx;
use crate::pyos::{self, join, join_all, py_strip};
use crate::voice::sha1_hex;

pub const READ_LIMIT: usize = 2000;
pub const REGISTERS: [&str; 3] = ["formal", "informal", "chat"];
/// `core` is still accepted and means `standard`.
pub const TIERS: [&str; 3] = ["quick", "standard", "full"];
pub const CORE_EXEMPLAR_CAP: u64 = 20_000;
/// The quick tier's excerpt: whole paragraphs up to this many words.
pub const EXCERPT_WORDS: usize = 400;
pub const DENSITY_LINE: &str = "Please remove all mannered prose: when a literal phrase is available, use it. \
Delete the clause that props up a claim already made. One idea per paragraph.";

pub fn voice_dir(root: &str) -> String {
    join_all(&[root, "workspace", "life", "voice"])
}

pub fn out_dir(root: &str) -> String {
    join_all(&[root, "system", "index", "voice"])
}

/// The tier a context asks for; every context not named here (long-form, builder-pov,
/// market-signal, slide, cover, and any unknown one) asks for full.
fn tier_by_context(c: &str) -> &'static str {
    match c {
        "message" | "chat" => "quick",
        "email" | "short-form" => "standard",
        _ => "full",
    }
}

/// `core` is the old name of `standard`.
pub fn normalize_tier(t: &str) -> &str {
    if t == "core" {
        "standard"
    } else {
        t
    }
}

pub fn context_pairs(c: &str) -> &'static [&'static str] {
    match c {
        "long-form" | "short-form" | "email" => &["plain-not-clever.md"],
        "builder-pov" => &["builder-pov.md", "plain-not-clever.md"],
        "market-signal" => &["market-signal.md"],
        "slide" | "cover" => &["slide-and-cover.md"],
        _ => &[],
    }
}

pub fn type_template(t: &str) -> Option<&'static str> {
    match t {
        "substack-essay" => Some("substack-essay.md"),
        "linkedin-copy" | "body-copy" | "linkedin-post" => Some("linkedin-body.md"),
        "carousel" | "carousel-copy" => Some("carousel-arc.md"),
        _ => None,
    }
}

pub fn type_context(t: &str) -> Option<&'static str> {
    match t {
        "substack-essay" | "linkedin-copy" | "body-copy" | "linkedin-post" => Some("long-form"),
        "carousel" | "carousel-copy" => Some("slide"),
        _ => None,
    }
}

// ---------------------------------------------------------------- recipe

fn scalar(fm: &str, key: &str) -> Option<String> {
    let re = rx(&format!(r"(?m)^\s*{}:\s*(.+?)\s*$", regex::escape(key)));
    let c = re.captures(fm)?;
    let v = py_strip(&c[1]).trim_matches(|ch| ch == '\'' || ch == '"').to_string();
    if v.is_empty() {
        None
    } else {
        Some(v)
    }
}

/// (base register, borrow registers, deliverable type) from a head's frontmatter.
pub fn recipe_from_head(path: &str) -> R<(Option<String>, Vec<String>, Option<String>)> {
    let text = pyos::read(path)?;
    let text = text.trim_start_matches('\u{feff}');
    let fm = rx(r"(?s)\A---\r?\n(.*?)\r?\n---")
        .captures(text)
        .map(|c| c[1].to_string())
        .unwrap_or_default();
    let mut base = scalar(&fm, "base_register").or_else(|| scalar(&fm, "register"));
    if let Some(b) = &base {
        if !REGISTERS.contains(&b.as_str()) {
            // the first register named inside the value, in REGISTERS order
            base = REGISTERS.iter().find(|r| b.contains(*r)).map(|r| r.to_string());
        }
    }
    let mut borrow = Vec::new();
    if let Some(c) = rx(r"(?m)^\s*borrow_from:\s*\[([^\]]*)\]").captures(&fm) {
        for b in c[1].split(',') {
            if !py_strip(b).is_empty() {
                borrow.push(py_strip(b).trim_matches(|ch| ch == '\'' || ch == '"').to_string());
            }
        }
    }
    borrow.retain(|b| REGISTERS.contains(&b.as_str()));
    Ok((base, borrow, scalar(&fm, "type")))
}

// ---------------------------------------------------------------- the bundle

/// The tier when `--tier` is not given, and how it was chosen. A context that asks for
/// full anywhere means full; message and chat contexts alone mean quick; any other mix
/// of email, short-form, message and chat means standard. With no context at all, the
/// chat register is quick and every other register is standard.
pub fn default_tier(contexts: &[String], register: &str) -> (&'static str, String) {
    if contexts.is_empty() {
        let tier = if register == "chat" { "quick" } else { "standard" };
        return (tier, "default, no context".to_string());
    }
    let tier = if contexts.iter().any(|c| tier_by_context(c) == "full") {
        "full"
    } else if contexts.iter().all(|c| tier_by_context(c) == "quick") {
        "quick"
    } else {
        "standard"
    };
    (tier, format!("from context {}", contexts.join("+")))
}

/// The contexts a bundle is built with. A recipe with none is long-form when the tier is
/// full (its pairs and file name), and `general` with no context pair otherwise.
pub fn effective_contexts(r: &Recipe, tier: &str) -> Vec<String> {
    if !r.contexts.is_empty() {
        r.contexts.clone()
    } else if tier == "full" {
        vec!["long-form".into()]
    } else {
        vec!["general".into()]
    }
}

/// The opening of a corpus piece for the quick tier, and its word count: frontmatter
/// dropped, whole paragraphs while the total stays at or under `EXCERPT_WORDS`, at least
/// the first paragraph, which is cut at the limit and ends ` …` when it alone is over.
pub fn excerpt_text(raw: &str) -> (String, usize) {
    let text = raw.trim_start_matches('\u{feff}');
    let body = match rx(r"\A---[ \t]*\r?\n(?:(?s:.*?)\r?\n)??---[ \t]*(?:\r?\n|\z)").find(text) {
        Some(m) => &text[m.end()..],
        None => text,
    };
    let paras: Vec<&str> =
        rx(r"\r?\n[ \t]*\r?\n").split(body.trim()).map(str::trim).filter(|p| !p.is_empty()).collect();
    let Some(first) = paras.first() else { return (String::new(), 0) };
    let first_words = first.split_whitespace().count();
    if first_words > EXCERPT_WORDS {
        let mut seen = 0;
        let mut in_word = false;
        let mut end = first.len();
        for (i, ch) in first.char_indices() {
            if ch.is_whitespace() {
                if in_word && seen == EXCERPT_WORDS {
                    end = i;
                    break;
                }
                in_word = false;
            } else if !in_word {
                in_word = true;
                seen += 1;
            }
        }
        return (format!("{} …", &first[..end]), EXCERPT_WORDS);
    }
    let mut total = first_words;
    let mut taken = vec![*first];
    for p in &paras[1..] {
        let n = p.split_whitespace().count();
        if total + n > EXCERPT_WORDS {
            break;
        }
        total += n;
        taken.push(p);
    }
    (taken.join("\n\n"), total)
}

fn corpus_pieces(corpus_dir: &str) -> R<Vec<String>> {
    if !pyos::isdir(corpus_dir) {
        return Ok(Vec::new());
    }
    Ok(pyos::sorted_listdir(corpus_dir)?
        .into_iter()
        .filter(|n| n.ends_with(".md") && n.to_lowercase() != "readme.md")
        .map(|n| join(corpus_dir, &n))
        .collect())
}

fn getsize(p: &str) -> R<u64> {
    std::fs::metadata(p).map(|m| m.len()).map_err(|e| pyos::os_error(&e, p))
}

/// The newest corpus piece at or under the cap, else the smallest piece.
pub fn exemplar(corpus_dir: &str, cap: u64) -> R<Option<String>> {
    let pieces = corpus_pieces(corpus_dir)?;
    if pieces.is_empty() {
        return Ok(None);
    }
    let mut sizes = Vec::new();
    for p in &pieces {
        sizes.push(getsize(p)?);
    }
    if let Some(i) = (0..pieces.len()).rev().find(|&i| sizes[i] <= cap) {
        return Ok(Some(pieces[i].clone()));
    }
    let mut best = 0;
    for i in 1..pieces.len() {
        if sizes[i] < sizes[best] {
            best = i;
        }
    }
    Ok(Some(pieces[best].clone()))
}

/// What a bundle is built from.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub register: String,
    pub contexts: Vec<String>,
    pub borrow: Vec<String>,
    pub template: Option<String>,
}

impl Recipe {
    pub fn new(register: &str, contexts: &[&str], borrow: &[&str], template: Option<&str>) -> Recipe {
        Recipe {
            register: register.into(),
            contexts: contexts.iter().map(|s| s.to_string()).collect(),
            borrow: borrow.iter().map(|s| s.to_string()).collect(),
            template: template.map(|s| s.to_string()),
        }
    }
}

/// The ordered (label, path) list the README prescribes. Existence is checked in build(),
/// except a register's own pair file, which is listed only when it exists.
pub fn plan(r: &Recipe, voice_dir: &str, tier: &str) -> R<Vec<(String, String)>> {
    let tier = normalize_tier(tier);
    let contexts = effective_contexts(r, tier);
    let register = &r.register;
    let reg_path = |b: &str| join_all(&[voice_dir, "registers", &format!("{b}.md")]);
    let corpus_dir = join_all(&[voice_dir, "corpus", register]);
    let mut parts = Vec::new();
    if tier == "quick" {
        parts.push(("anti-patterns".to_string(), join(voice_dir, "anti-patterns.md")));
        parts.push(("identity".to_string(), join(voice_dir, "identity.md")));
        parts.push((format!("register {register}"), reg_path(register)));
        for b in &r.borrow {
            parts.push((format!("borrow register {b}"), reg_path(b)));
        }
        if let Some(piece) = exemplar(&corpus_dir, CORE_EXEMPLAR_CAP)? {
            // a piece with nothing after its frontmatter gives no excerpt part
            if !excerpt_text(&pyos::read(&piece)?).0.is_empty() {
                parts.push((format!("excerpt {}", pyos::basename(&piece)), piece));
            }
        }
        return Ok(parts);
    }
    parts.push(("procedure".to_string(), join(voice_dir, "README.md")));
    parts.push(("identity".to_string(), join(voice_dir, "identity.md")));
    parts.push(("profile".to_string(), join(voice_dir, "voice-profile.md")));
    parts.push(("anti-patterns".to_string(), join(voice_dir, "anti-patterns.md")));
    parts.push((format!("register {register}"), reg_path(register)));
    for b in &r.borrow {
        parts.push((format!("borrow register {b}"), reg_path(b)));
    }
    if let Some(t) = r.template.as_deref().filter(|t| !t.is_empty()) {
        parts.push((format!("template {t}"), join_all(&[voice_dir, "templates", t])));
    }
    if tier == "standard" {
        if let Some(piece) = exemplar(&corpus_dir, CORE_EXEMPLAR_CAP)? {
            parts.push((format!("exemplar {}", pyos::basename(&piece)), piece));
        }
        return Ok(parts);
    }
    let mut own: Vec<String> = vec![format!("{register}.md")];
    own.extend(r.borrow.iter().map(|b| format!("{b}.md")));
    let mut pair_files: Vec<(String, bool)> = own.into_iter().map(|n| (n, true)).collect();
    for c in &contexts {
        pair_files.extend(context_pairs(c).iter().map(|s| (s.to_string(), false)));
    }
    let mut seen: Vec<String> = Vec::new();
    for (name, optional) in pair_files {
        if seen.contains(&name) {
            continue;
        }
        let path = join_all(&[voice_dir, "pairs", &name]);
        seen.push(name.clone());
        if optional && !pyos::isfile(&path) {
            continue;
        }
        parts.push((format!("pairs {name}"), path));
    }
    for piece in corpus_pieces(&corpus_dir)? {
        parts.push((format!("corpus {}", pyos::basename(&piece)), piece));
    }
    Ok(parts)
}

fn rel(root: &str, path: &str) -> R<String> {
    Ok(pyos::relpath(path, root)?.replace('\\', "/"))
}

fn read_part(path: &str) -> R<String> {
    Ok(pyos::read(path)?.trim_end_matches('\n').to_string())
}

/// What a part contributes: its file, or for an excerpt the opening of it and its word count.
fn part_body(label: &str, path: &str) -> R<(String, Option<usize>)> {
    if label.starts_with("excerpt ") {
        let (text, words) = excerpt_text(&pyos::read(path)?);
        return Ok((text, Some(words)));
    }
    Ok((read_part(path)?, None))
}

/// (files, bytes, lines) a tier would bundle, without writing it.
pub fn estimate(r: &Recipe, voice_dir: &str, tier: &str) -> R<(usize, usize, usize)> {
    let mut texts = Vec::new();
    for (l, p) in plan(r, voice_dir, tier)? {
        if pyos::isfile(&p) {
            texts.push(part_body(&l, &p)?.0);
        }
    }
    Ok((
        texts.len(),
        texts.iter().map(|t| t.len()).sum(),
        texts.iter().map(|t| t.matches('\n').count() + 3).sum::<usize>() + 4,
    ))
}

#[derive(Debug, Clone)]
pub struct Bundle {
    pub path: String,
    pub id: String,
    pub tier: String,
    pub register: String,
    pub contexts: Vec<String>,
    pub borrow: Vec<String>,
    pub parts: Vec<(String, String)>,
    pub missing: Vec<(String, String)>,
    pub bytes: usize,
    pub lines: usize,
    /// how the tier was chosen: `asked`, `default, no context` or `from context ...`
    pub tier_why: String,
    /// (tier, files, bytes, lines) for each of the other two tiers
    pub other_tiers: Vec<(String, usize, usize, usize)>,
}

impl Bundle {
    pub fn labels(&self) -> Vec<String> {
        self.parts.iter().map(|(l, _)| l.clone()).collect()
    }
}

/// Where the bundle reads from and writes to, and its clock.
pub struct Place<'a> {
    pub root: &'a str,
    pub voice_dir: &'a str,
    pub out_dir: &'a str,
    /// `datetime.now().strftime("%Y-%m-%d %H:%M")`, or a fixed stamp.
    pub stamp: Option<&'a str>,
}

pub fn build(place: &Place, r: &Recipe, head: Option<&str>, tier: Option<&str>) -> R<Bundle> {
    let (tier, tier_why) = match tier {
        Some(t) => (normalize_tier(t).to_string(), "asked".to_string()),
        None => {
            let (t, why) = default_tier(&r.contexts, &r.register);
            (t.to_string(), why)
        }
    };
    let contexts = effective_contexts(r, &tier);
    let parts = plan(r, place.voice_dir, &tier)?;
    let present: Vec<(String, String)> = parts.iter().filter(|(_, p)| pyos::isfile(p)).cloned().collect();
    let missing: Vec<(String, String)> = parts.iter().filter(|(_, p)| !pyos::isfile(p)).cloned().collect();
    pyos::makedirs(place.out_dir)?;
    let ctx_join = contexts.join("+");
    let out = join(
        place.out_dir,
        &format!(
            "{}-{}{}.md",
            r.register,
            ctx_join,
            match tier.as_str() {
                "quick" => "-quick",
                "standard" => "-standard",
                _ => "",
            }
        ),
    );

    let mut digest_input = format!("{}|{}|{}|{}", tier, r.register, ctx_join, r.borrow.join(",")).into_bytes();
    let mut chunks = Vec::new();
    for (i, (label, path)) in present.iter().enumerate() {
        let (body, excerpt_words) = part_body(label, path)?;
        let rp = rel(place.root, path)?;
        digest_input.extend_from_slice(format!("\n{rp}\n{body}").as_bytes());
        let note = excerpt_words.map(|n| format!(", an excerpt of {n} words")).unwrap_or_default();
        chunks.push(format!(
            "<!-- ===== voice bundle part {}/{}: {label} ({rp}){note} ===== -->\n\n{body}\n",
            i + 1,
            present.len()
        ));
    }
    let body_text = chunks.join("\n\n");
    let bundle_id = sha1_hex(&digest_input)[..8].to_string();
    let approx_lines = body_text.matches('\n').count() + 4;
    let stamp = place.stamp.map(|s| s.to_string()).unwrap_or_else(crate::core::stamp);
    let head_rel = match head {
        Some(h) => rel(place.root, h)?,
        None => "none".into(),
    };
    let header = format!(
        "<!-- VOICE BUNDLE id {bundle_id}, generated {stamp}. tier {tier}; register {}; context {ctx_join}; \
borrow {}; head {head_rel}. {} files, about {approx_lines} lines. Read this file whole{}. It stays loaded for the session: \
read it again only after a compaction, when the register changes, or before the back half of a draft over about 2,500 words. \
Derived cache under system/index/: never edit it; edit the files it names. -->\n<!-- {DENSITY_LINE} -->\n\n",
        r.register,
        if r.borrow.is_empty() { "none".to_string() } else { r.borrow.join(", ") },
        present.len(),
        if approx_lines > READ_LIMIT {
            format!(
                "; it is longer than the Read tool's {READ_LIMIT}-line window, so read it in two calls (offset {})",
                READ_LIMIT + 1
            )
        } else {
            String::new()
        },
    );
    let text = header + &body_text;
    pyos::write(&out, &text)?;
    let mut other_tiers = Vec::new();
    for other in TIERS.iter().filter(|t| **t != tier) {
        let (files, nbytes, nlines) = estimate(r, place.voice_dir, other)?;
        other_tiers.push((other.to_string(), files, nbytes, nlines));
    }
    let mut parts_rel = Vec::new();
    for (l, p) in &present {
        parts_rel.push((l.clone(), rel(place.root, p)?));
    }
    let mut missing_rel = Vec::new();
    for (l, p) in &missing {
        missing_rel.push((l.clone(), rel(place.root, p)?));
    }
    Ok(Bundle {
        path: out,
        id: bundle_id,
        tier,
        register: r.register.clone(),
        contexts,
        borrow: r.borrow.clone(),
        parts: parts_rel,
        missing: missing_rel,
        bytes: text.len(),
        lines: text.matches('\n').count() + 1,
        tier_why,
        other_tiers,
    })
}

/// `f"{n:,}"`
fn thousands(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn format_report(root: &str, b: &Bundle) -> R<String> {
    let mut lines = vec![format!(
        "af voice bundle: {} — {} files, {} bytes, {} lines (tier {} ({}), register {}, context {}{}; id {})",
        rel(root, &b.path)?,
        b.parts.len(),
        thousands(b.bytes),
        b.lines,
        b.tier,
        b.tier_why,
        b.register,
        b.contexts.join("+"),
        if b.borrow.is_empty() { String::new() } else { format!(", borrow {}", b.borrow.join(", ")) },
        b.id
    )];
    for (label, p) in &b.parts {
        lines.push(format!("  {label:<36} {p}"));
    }
    for (label, p) in &b.missing {
        lines.push(format!("  MISSING {label:<28} {p}"));
    }
    lines.push(String::new());
    if b.lines > READ_LIMIT {
        lines.push(format!(
            "Read it whole in two calls: the Read tool shows {READ_LIMIT} lines by default, so read again with offset {}.",
            READ_LIMIT + 1
        ));
    } else {
        lines.push("Read it whole, in one call, before writing a line.".into());
    }
    lines.push(format!(
        "Already holding bundle {} in context with no compaction since? It is current; skip the read. \
Read again after a compaction, when the register changes, or before the back half of a draft over about 2,500 words.",
        b.id
    ));
    for (other, files, _nbytes, nlines) in &b.other_tiers {
        let use_for = match other.as_str() {
            "quick" => "for a text, a direct message, a comment or a one-paragraph reply: the anti-patterns, the register and a short excerpt.",
            "standard" => "for an email, a short write under a page, or a copyedit of his text: the rules and one whole exemplar.",
            _ => "for a first draft, or anything over a page: adds the pairs and the whole corpus.",
        };
        lines.push(format!("--tier {other} is {files} files, about {nlines} lines, {use_for}"));
    }
    lines.push(DENSITY_LINE.into());
    Ok(lines.join("\n"))
}
