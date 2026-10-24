//! Port of system/voice_bundle.py: the voice system concatenated into one file under
//! `system/index/voice/` so a drafting turn loads it in one read.

use crate::error::R;
use crate::fm::rx;
use crate::pyos::{self, join, join_all, py_strip};
use crate::voice::sha1_hex;

pub const READ_LIMIT: usize = 2000;
pub const REGISTERS: [&str; 2] = ["formal", "informal"];
pub const TIERS: [&str; 2] = ["core", "full"];
pub const CORE_EXEMPLAR_CAP: u64 = 20_000;
pub const DENSITY_LINE: &str = "Please remove all mannered prose: when a literal phrase is available, use it. \
Delete the clause that props up a claim already made. One idea per paragraph.";

pub fn voice_dir(root: &str) -> String {
    join_all(&[root, "workspace", "life", "voice"])
}

pub fn out_dir(root: &str) -> String {
    join_all(&[root, "system", "index", "voice"])
}

/// TIER_BY_CONTEXT; every other context defaults to full.
fn tier_by_context(c: &str) -> &'static str {
    match c {
        "email" | "short-form" => "core",
        _ => "full",
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
            // voice_bundle.py: the first register named inside the value, in REGISTERS order
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

/// core only when every task context asks for it; a long-form context anywhere means full.
pub fn default_tier(contexts: &[String]) -> &'static str {
    let long = ["long-form".to_string()];
    let cs: &[String] = if contexts.is_empty() { &long } else { contexts };
    if cs.iter().all(|c| tier_by_context(c) == "core") {
        "core"
    } else {
        "full"
    }
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

    fn contexts_or_default(&self) -> Vec<String> {
        if self.contexts.is_empty() {
            vec!["long-form".into()]
        } else {
            self.contexts.clone()
        }
    }
}

/// The ordered (label, path) list the README prescribes. Existence is checked in build().
pub fn plan(r: &Recipe, voice_dir: &str, tier: &str) -> R<Vec<(String, String)>> {
    let contexts = r.contexts_or_default();
    let register = &r.register;
    let mut parts = vec![
        ("procedure".to_string(), join(voice_dir, "README.md")),
        ("identity".to_string(), join(voice_dir, "identity.md")),
        ("profile".to_string(), join(voice_dir, "voice-profile.md")),
        ("anti-patterns".to_string(), join(voice_dir, "anti-patterns.md")),
        (format!("register {register}"), join_all(&[voice_dir, "registers", &format!("{register}.md")])),
    ];
    for b in &r.borrow {
        parts.push((format!("borrow register {b}"), join_all(&[voice_dir, "registers", &format!("{b}.md")])));
    }
    if let Some(t) = r.template.as_deref().filter(|t| !t.is_empty()) {
        parts.push((format!("template {t}"), join_all(&[voice_dir, "templates", t])));
    }
    let corpus_dir = join_all(&[voice_dir, "corpus", register]);
    if tier == "core" {
        if let Some(piece) = exemplar(&corpus_dir, CORE_EXEMPLAR_CAP)? {
            parts.push((format!("exemplar {}", pyos::basename(&piece)), piece));
        }
        return Ok(parts);
    }
    let mut pair_files: Vec<String> = vec![format!("{register}.md")];
    pair_files.extend(r.borrow.iter().map(|b| format!("{b}.md")));
    for c in &contexts {
        pair_files.extend(context_pairs(c).iter().map(|s| s.to_string()));
    }
    let mut seen: Vec<String> = Vec::new();
    for name in pair_files {
        if seen.contains(&name) {
            continue;
        }
        parts.push((format!("pairs {name}"), join_all(&[voice_dir, "pairs", &name])));
        seen.push(name);
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

/// (files, bytes, lines) a tier would bundle, without writing it.
pub fn estimate(r: &Recipe, voice_dir: &str, tier: &str) -> R<(usize, usize, usize)> {
    let mut texts = Vec::new();
    for (_, p) in plan(r, voice_dir, tier)? {
        if pyos::isfile(&p) {
            texts.push(read_part(&p)?);
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
    pub other_tier: (String, usize, usize, usize),
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
    let contexts = r.contexts_or_default();
    let r = Recipe { contexts: contexts.clone(), ..r.clone() };
    let tier = tier.unwrap_or_else(|| default_tier(&contexts)).to_string();
    let parts = plan(&r, place.voice_dir, &tier)?;
    let present: Vec<(String, String)> = parts.iter().filter(|(_, p)| pyos::isfile(p)).cloned().collect();
    let missing: Vec<(String, String)> = parts.iter().filter(|(_, p)| !pyos::isfile(p)).cloned().collect();
    pyos::makedirs(place.out_dir)?;
    let ctx_join = contexts.join("+");
    let out = join(
        place.out_dir,
        &format!("{}-{}{}.md", r.register, ctx_join, if tier == "core" { "-core" } else { "" }),
    );

    let mut digest_input = format!("{}|{}|{}|{}", tier, r.register, ctx_join, r.borrow.join(",")).into_bytes();
    let mut chunks = Vec::new();
    for (i, (label, path)) in present.iter().enumerate() {
        let body = read_part(path)?;
        let rp = rel(place.root, path)?;
        digest_input.extend_from_slice(format!("\n{rp}\n{body}").as_bytes());
        chunks.push(format!(
            "<!-- ===== voice bundle part {}/{}: {label} ({rp}) ===== -->\n\n{body}\n",
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
    let other = if tier == "core" { "full" } else { "core" };
    let (files, nbytes, nlines) = estimate(&r, place.voice_dir, other)?;
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
        other_tier: (other.to_string(), files, nbytes, nlines),
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
        "af voice bundle: {} — {} files, {} bytes, {} lines (tier {}, register {}, context {}{}; id {})",
        rel(root, &b.path)?,
        b.parts.len(),
        thousands(b.bytes),
        b.lines,
        b.tier,
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
    let (other, files, _nbytes, nlines) = &b.other_tier;
    if other == "core" {
        lines.push(format!(
            "For a short write (an email, a message, a paragraph, a note under a page) or a copyedit pass over his text, \
--tier core is {files} files, about {nlines} lines: the rules and one exemplar, no pairs."
        ));
    } else {
        lines.push(format!(
            "For a first draft or anything over a page, --tier full adds the pairs and the whole corpus: {files} files, about {nlines} lines."
        ));
    }
    lines.push(DENSITY_LINE.into());
    Ok(lines.join("\n"))
}
