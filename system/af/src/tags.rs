//! `af tag`: the controlled vocabulary behind `tags:` on place, person and knowledge pages and on sources.
//!
//! The registry `workspace/life/tags.yaml` is the truth (facets, then one entry per tag with
//! its facet, lifecycle status, alt labels, scope note); `tags-log.md` is its append-only
//! change log and `tags.md` a rendered view. The verbs: `match` resolves a phrase to a usable
//! tag (exact id, alt label, singular/plural or hyphen variant; an embedding is only ever a
//! suggestion), `propose` / `promote` / `deprecate` / `drop` move a tag through its life,
//! `merge` and `rename` rewrite every page and keep the old form as an alt label, `add`
//! writes validated tags on one page or source, `facet add` declares a facet, `stats` and
//! `lint` are the weekly pass's instruments,
//! and `doctor_checks` is what `af doctor` adds: unknown or retired tags on pages, a stale
//! rendered view, and a `[[slug]]` with no page under places/ or people/.

use crate::core::{parse_iso_date, today, today_date, Ctx};
use crate::die;
use crate::error::R;
use crate::fm::{fm_list_span, fm_list_values, get_scalar, rx};
use crate::index::embed::embed_texts;
use crate::life::{vpath, TextFile};
use crate::out;
use crate::pyos::{self, py_strip};
use crate::pyyaml::{safe_load, PyVal};
use clap::Subcommand;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

pub const REGISTRY_REL: &str = "workspace/life/tags.yaml";
pub const LOG_REL: &str = "workspace/life/tags-log.md";
pub const VIEW_REL: &str = "workspace/life/tags.md";
pub const PAGE_DIRS: [&str; 2] = ["workspace/life/places", "workspace/life/people"];
pub const KNOWLEDGE_DIR: &str = "workspace/life/knowledge";
pub const SOURCES_DIR: &str = "workspace/life/sources";
/// Folders under workspace/life/ whose `[[slug]]`s are not checked: immutable inputs,
/// archives, the voice corpus, and the append-only log.
pub const LINK_SKIP_DIRS: [&str; 5] = ["sources", "voice", "_history", "_archive", "log"];
pub const STATUSES: [&str; 4] = ["proposed", "active", "deprecated", "merged"];
/// What a facet applies to. `both` is places and people; `context` is knowledge pages and sources.
pub const APPLIES: [&str; 6] = ["places", "people", "both", "knowledge", "sources", "context"];
/// A proposed tag becomes active at this many uses on this many pages.
pub const PROMOTE_USES: usize = 3;
pub const PROMOTE_PAGES: usize = 2;
/// A proposed tag with no use after this many days is dropped.
pub const PROPOSED_STALE_DAYS: i64 = 60;
/// An active tag with one use after this many days is listed for review, never auto-dropped.
pub const SINGLETON_REVIEW_DAYS: i64 = 90;
pub const PAGE_TAG_CAP: usize = 8;
/// A knowledge page or a source carries at most this many tags.
pub const DOC_TAG_CAP: usize = 3;
/// The facet whose tags a knowledge page is expected to carry.
pub const TOPIC_FACET: &str = "topic";
/// A place page (not an area) with fewer tags than this is under-tagged.
pub const PAGE_TAG_FLOOR: usize = 3;
/// Cosine at or above which an embedding neighbour is worth suggesting.
pub const SUGGEST_COSINE: f64 = 0.55;
pub const SUGGEST_N: usize = 3;

/// The starter facets `af tag init` writes; names only, no instance vocabulary.
pub const DEFAULT_FACETS: [(&str, &str, bool); 12] = [
    ("cuisine", "places", false),
    ("music", "places", false),
    ("vibe", "places", false),
    ("occasion", "places", false),
    ("crowd", "places", false),
    ("activity", "places", false),
    ("price", "places", true),
    ("practical", "places", false),
    ("interest", "people", false),
    ("context", "people", false),
    ("closeness", "people", true),
    ("topic", "context", false),
];

/// Does a facet whose `applies` is this value apply to a page on this side? The one place the
/// two are compared.
pub fn facet_applies(applies: &str, side: &str) -> bool {
    match applies {
        "both" => side == "places" || side == "people",
        "context" => side == "knowledge" || side == "sources",
        a => a == side,
    }
}

/// The most tags a page on this side carries.
pub fn tag_cap(side: &str) -> usize {
    if side == "knowledge" || side == "sources" {
        DOC_TAG_CAP
    } else {
        PAGE_TAG_CAP
    }
}

#[derive(Subcommand, Debug)]
pub enum TagCmd {
    /// Write a registry with the starter facets and no tags
    Init,
    /// Resolve a phrase to a usable tag: exact id, alt label, or a plural/hyphen variant
    Match {
        phrase: String,
        #[arg(long)]
        facet: Option<String>,
        /// On no match, suggest the nearest tags by embedding (a suggestion to confirm, never a match)
        #[arg(long)]
        suggest: bool,
    },
    /// Add a new tag as `proposed`
    Propose {
        slug: String,
        #[arg(long)]
        facet: String,
        #[arg(long, default_value = "")]
        scope: String,
        /// Alt labels, comma-separated
        #[arg(long, default_value = "")]
        alt: String,
    },
    /// proposed -> active
    Promote { slug: String },
    /// active or proposed -> deprecated (still resolves, no new uses)
    Deprecate {
        slug: String,
        #[arg(long, default_value = "")]
        reason: String,
    },
    /// Remove a proposed tag that no page uses
    Drop { slug: String },
    /// Rewrite <from> to <into> on every page; <from> becomes an alt label of <into>
    Merge {
        from: String,
        #[arg(long)]
        into: String,
    },
    /// Give a tag a new id; pages are rewritten and the old id stays as an alt label
    Rename { old: String, new: String },
    /// Declare a new facet in the registry
    Facet {
        #[command(subcommand)]
        cmd: FacetCmd,
    },
    /// Write validated tags on one page or source (alt labels resolve; unknown and retired tags refuse)
    Add {
        page: String,
        #[arg(required = true)]
        tags: Vec<String>,
    },
    /// The vocabulary, optionally one facet or one status
    List {
        #[arg(long)]
        facet: Option<String>,
        #[arg(long)]
        status: Option<String>,
    },
    /// Counts by facet and status, use counts per tag, and the health metrics
    Stats,
    /// The weekly findings: drift on pages, promotions, drops, near-duplicates, overloads, under-tagged pages
    Lint {
        /// Apply the mechanical items: promotions and drops
        #[arg(long)]
        fix: bool,
        /// Add embedding near-duplicate pairs (needs the local model)
        #[arg(long)]
        embed: bool,
    },
    /// Rewrite workspace/life/tags.md from the registry
    Render,
}

#[derive(Subcommand, Debug)]
pub enum FacetCmd {
    /// Add a facet to the registry
    Add {
        name: String,
        /// places, people, both, knowledge, sources or context (knowledge pages and sources)
        #[arg(long)]
        applies: String,
        /// One tag of this facet per page
        #[arg(long)]
        single: bool,
    },
}

// ------------------------------------------------------------------ registry

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facet {
    pub name: String,
    /// places | people | both | knowledge | sources | context
    pub applies: String,
    pub single: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Tag {
    pub id: String,
    pub facet: String,
    pub status: String,
    pub alt: Vec<String>,
    pub scope: String,
    pub broader: Option<String>,
    pub since: Option<String>,
    pub merged_into: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Registry {
    pub facets: Vec<Facet>,
    pub tags: Vec<Tag>,
}

pub fn valid_slug(s: &str) -> bool {
    rx(r"\A[a-z0-9][a-z0-9-]{0,39}\z").is_match(s) && !s.ends_with('-') && !s.contains("--")
}

/// A phrase as a slug: lower case, every run of non-alphanumerics one hyphen.
pub fn normalize(phrase: &str) -> String {
    let low = phrase.to_lowercase();
    let mut out = String::new();
    let mut dash = false;
    for c in low.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

/// Singular/plural and hyphen variants of a slug, the slug itself first.
pub fn variants(slug: &str) -> Vec<String> {
    let mut v = vec![slug.to_string()];
    let mut push = |s: String| {
        if !s.is_empty() && !v.contains(&s) {
            v.push(s);
        }
    };
    if let Some(stem) = slug.strip_suffix("ies") {
        push(format!("{stem}y"));
    }
    if let Some(stem) = slug.strip_suffix("es") {
        push(stem.to_string());
    }
    if let Some(stem) = slug.strip_suffix('s') {
        push(stem.to_string());
    }
    if let Some(stem) = slug.strip_suffix('y') {
        push(format!("{stem}ies"));
    }
    push(format!("{slug}s"));
    push(format!("{slug}es"));
    push(slug.replace('-', ""));
    v
}

fn s_of(v: Option<&PyVal>) -> String {
    match v {
        None | Some(PyVal::None) => String::new(),
        Some(x) => py_strip(&x.py_str()).to_string(),
    }
}

fn opt_of(v: Option<&PyVal>) -> Option<String> {
    let s = s_of(v);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn list_of(v: Option<&PyVal>) -> Vec<String> {
    match v {
        Some(PyVal::List(items)) => items.iter().map(|i| py_strip(&i.py_str()).to_string()).filter(|s| !s.is_empty()).collect(),
        Some(PyVal::Str(s)) => s.split(',').map(|x| py_strip(x).to_string()).filter(|s| !s.is_empty()).collect(),
        _ => Vec::new(),
    }
}

impl Registry {
    pub fn path(ctx: &Ctx) -> String {
        vpath(ctx, REGISTRY_REL)
    }

    /// None when there is no registry yet; a refusal when it does not parse.
    pub fn load(ctx: &Ctx) -> R<Option<Registry>> {
        let path = Registry::path(ctx);
        let file = match TextFile::load(&path)? {
            Some(f) => f,
            None => return Ok(None),
        };
        Registry::parse(&file.text).map(Some)
    }

    /// The registry, or the refusal that names how to create one.
    pub fn require(ctx: &Ctx) -> R<Registry> {
        match Registry::load(ctx)? {
            Some(r) => Ok(r),
            None => die!("{REGISTRY_REL} does not exist; af tag init writes one with the starter facets"),
        }
    }

    pub fn parse(text: &str) -> R<Registry> {
        let doc = match safe_load(text, REGISTRY_REL, false) {
            Ok(d) => d,
            Err(e) => die!("{REGISTRY_REL} does not parse: {}", e.lines().next().unwrap_or("")),
        };
        let mut reg = Registry::default();
        if let Some(PyVal::List(items)) = doc.get("facets") {
            for item in items {
                let name = s_of(item.get("name"));
                if !valid_slug(&name) {
                    die!("{REGISTRY_REL}: facet name '{name}' is not a slug");
                }
                let applies = opt_of(item.get("applies")).unwrap_or_else(|| "places".into());
                if !APPLIES.contains(&applies.as_str()) {
                    die!("{REGISTRY_REL}: facet '{name}' applies '{applies}' is not one of {}", APPLIES.join(", "));
                }
                let single = s_of(item.get("cardinality")) == "single";
                reg.facets.push(Facet { name, applies, single });
            }
        }
        if let Some(PyVal::List(items)) = doc.get("tags") {
            for item in items {
                let id = s_of(item.get("id"));
                if !valid_slug(&id) {
                    die!("{REGISTRY_REL}: tag id '{id}' is not a slug (lower case, digits, single hyphens)");
                }
                let status = opt_of(item.get("status")).unwrap_or_else(|| "active".into());
                if !STATUSES.contains(&status.as_str()) {
                    die!("{REGISTRY_REL}: tag '{id}' status '{status}' is not one of {}", STATUSES.join(", "));
                }
                let facet = s_of(item.get("facet"));
                if !reg.facets.iter().any(|f| f.name == facet) {
                    die!("{REGISTRY_REL}: tag '{id}' names facet '{facet}', which is not declared under facets");
                }
                if reg.tags.iter().any(|t| t.id == id) {
                    die!("{REGISTRY_REL}: tag '{id}' is listed twice");
                }
                reg.tags.push(Tag {
                    id,
                    facet,
                    status,
                    alt: list_of(item.get("alt")),
                    scope: s_of(item.get("scope")),
                    broader: opt_of(item.get("broader")),
                    since: opt_of(item.get("since")),
                    merged_into: opt_of(item.get("merged_into")),
                });
            }
        }
        Ok(reg)
    }

    pub fn facet(&self, name: &str) -> Option<&Facet> {
        self.facets.iter().find(|f| f.name == name)
    }

    pub fn get(&self, id: &str) -> Option<&Tag> {
        self.tags.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Tag> {
        self.tags.iter_mut().find(|t| t.id == id)
    }

    /// The tag that lists `label` as an alt label.
    pub fn by_alt(&self, label: &str) -> Option<&Tag> {
        self.tags.iter().find(|t| t.alt.iter().any(|a| a == label))
    }

    /// Resolve a phrase: exact id, then alt label, then the plural and hyphen variants of
    /// both; a merged tag resolves to its target. `via` says how it matched.
    pub fn resolve(&self, phrase: &str, facet: Option<&str>) -> Resolved<'_> {
        let norm = normalize(phrase);
        if norm.is_empty() {
            return Resolved::None;
        }
        let in_facet = |t: &Tag| facet.map(|f| t.facet == f).unwrap_or(true);
        let mut found: Option<(&Tag, String)> = None;
        for (i, v) in variants(&norm).iter().enumerate() {
            if let Some(t) = self.tags.iter().find(|t| t.id == *v && in_facet(t)) {
                found = Some((t, if i == 0 { "id".to_string() } else { format!("variant of '{v}'") }));
                break;
            }
            if let Some(t) = self.tags.iter().find(|t| t.alt.iter().any(|a| a == v) && in_facet(t)) {
                found = Some((t, format!("alt label '{v}'")));
                break;
            }
        }
        let (mut tag, mut via) = match found {
            Some(f) => f,
            None => return Resolved::None,
        };
        let mut hops = 0;
        while tag.status == "merged" {
            let target = match tag.merged_into.as_deref().and_then(|m| self.get(m)) {
                Some(t) => t,
                None => return Resolved::None,
            };
            via = format!("merged from '{}'", tag.id);
            tag = target;
            hops += 1;
            if hops > 8 {
                return Resolved::None;
            }
        }
        if tag.status == "deprecated" {
            return Resolved::Deprecated(tag);
        }
        Resolved::Use(tag, via)
    }

    /// Canonical YAML: a header, the facets, then the tags ordered by facet then id.
    pub fn render_yaml(&self) -> String {
        let mut s = String::new();
        s.push_str("# The controlled vocabulary for `tags:` on place, person and knowledge pages and on sources. Edit it through `af tag`\n");
        s.push_str("# (match, propose, promote, merge, rename, deprecate); tags.md is the rendered view.\n");
        s.push_str("# facet: applies is places | people | both | knowledge | sources | context; cardinality single means one tag per page.\n");
        s.push_str("# tag: status is proposed | active | deprecated | merged; alt labels resolve in af tag match.\n");
        s.push_str("facets:\n");
        for f in &self.facets {
            s.push_str(&format!("  - name: {}\n    applies: {}\n    cardinality: {}\n", yq(&f.name), f.applies, if f.single { "single" } else { "multi" }));
        }
        s.push_str("tags:\n");
        let order: HashMap<&str, usize> = self.facets.iter().enumerate().map(|(i, f)| (f.name.as_str(), i)).collect();
        let mut tags: Vec<&Tag> = self.tags.iter().collect();
        tags.sort_by(|a, b| {
            let fa = order.get(a.facet.as_str()).copied().unwrap_or(usize::MAX);
            let fb = order.get(b.facet.as_str()).copied().unwrap_or(usize::MAX);
            fa.cmp(&fb).then_with(|| a.id.cmp(&b.id))
        });
        for t in tags {
            s.push_str(&format!("  - id: {}\n    facet: {}\n    status: {}\n", yq(&t.id), yq(&t.facet), t.status));
            let alts: Vec<String> = t.alt.iter().map(|a| yq(a)).collect();
            s.push_str(&format!("    alt: [{}]\n", alts.join(", ")));
            s.push_str(&format!("    scope: {}\n", yq(&t.scope)));
            if let Some(b) = &t.broader {
                s.push_str(&format!("    broader: {}\n", yq(b)));
            }
            if let Some(d) = &t.since {
                s.push_str(&format!("    since: {d}\n"));
            }
            if let Some(m) = &t.merged_into {
                s.push_str(&format!("    merged_into: {}\n", yq(m)));
            }
        }
        s
    }

    pub fn save(&self, ctx: &Ctx) -> R<()> {
        let path = Registry::path(ctx);
        let mut file = TextFile::load(&path)?.unwrap_or_else(|| TextFile::new(""));
        file.text = self.render_yaml();
        file.save(&path)
    }
}

pub enum Resolved<'a> {
    Use(&'a Tag, String),
    Deprecated(&'a Tag),
    None,
}

/// A YAML scalar: bare when it is a plain slug, double-quoted otherwise (empty included).
fn yq(s: &str) -> String {
    let bare = rx(r"\A[a-z][a-z0-9-]*\z").is_match(s) && !matches!(s, "y" | "n" | "no" | "on" | "off" | "yes" | "true" | "false" | "null");
    if bare {
        return s.to_string();
    }
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

fn append_log(ctx: &Ctx, verb: &str, detail: &str) -> R<()> {
    let path = vpath(ctx, LOG_REL);
    let mut file = TextFile::load(&path)?.unwrap_or_else(|| {
        TextFile::new("# Tag changes\n\nAppend-only, one line per registry change, written by `af tag`.\n\n")
    });
    file.push_line(&format!("- {} · {verb} · {detail}", today()));
    file.save(&path)
}

// ------------------------------------------------------------------ pages

#[derive(Clone, Debug)]
pub struct Page {
    /// Vault-relative, forward slashes.
    pub rel: String,
    pub path: String,
    pub tags: Vec<String>,
    pub kind: Option<String>,
    /// places | people | knowledge | sources, by folder.
    pub side: &'static str,
    pub has_tags_field: bool,
}

impl Page {
    pub fn is_area(&self) -> bool {
        self.kind.as_deref() == Some("area")
    }
}

/// Where the frontmatter of a text sits. `start` is the first byte after the opening `---` line,
/// `close` the first byte of the closing `---` line, `eol` the opening line's line ending.
struct FmBounds {
    eol: &'static str,
    start: usize,
    close: usize,
}

/// The frontmatter block of `raw`: an opening `---` line (after an optional BOM) and the first later
/// line that is `---` alone. None when there is no block or it is never closed. Works on the bytes as
/// they are, so a CRLF file's offsets are its own.
fn fm_bounds(raw: &str) -> Option<FmBounds> {
    let bom = if raw.starts_with('\u{feff}') { '\u{feff}'.len_utf8() } else { 0 };
    let rest = &raw[bom..];
    let eol = if rest.starts_with("---\r\n") {
        "\r\n"
    } else if rest.starts_with("---\n") {
        "\n"
    } else {
        return None;
    };
    let start = bom + 3 + eol.len();
    let mut pos = start;
    while pos < raw.len() {
        let end = raw[pos..].find('\n').map(|i| pos + i).unwrap_or(raw.len());
        if raw[pos..end].trim_end_matches('\r').trim_end_matches([' ', '\t']) == "---" {
            return Some(FmBounds { eol, start, close: pos });
        }
        pos = end + 1;
    }
    None
}

/// An optionally quoted key of letters, digits, `_` or `-`, then `:` and a space or the end of the line.
fn is_key_line(line: &str) -> bool {
    rx(r#"^(?:"[A-Za-z0-9_-]+"|'[A-Za-z0-9_-]+'|[A-Za-z0-9_-]+)[ \t]*:(?:[ \t]|$)"#).is_match(line.trim_end_matches('\r'))
}

/// Does a block between two `---` lines read as frontmatter? Every line is blank, a comment, indented,
/// a list item, the closing bracket of a wrapped list, or a key line, and some line is a key (an
/// empty block counts). Prose that happens to sit between two rules is not.
fn looks_like_frontmatter(block: &str) -> bool {
    let mut keys = 0;
    for l in block.lines() {
        let l = l.trim_end_matches('\r');
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') || l.starts_with(' ') || l.starts_with('\t') || l.starts_with(']') || l.starts_with('}') {
            continue;
        }
        if rx(r"\A-([ \t]|$)").is_match(l) {
            continue;
        }
        if is_key_line(l) {
            keys += 1;
            continue;
        }
        return false;
    }
    keys > 0 || block.trim().is_empty()
}

/// The knowledge pages and sources are free text that may open with a rule, so their frontmatter
/// must also look like frontmatter; a place or person page is a block between two rules, as ever.
fn strict(side: &str) -> bool {
    side == "knowledge" || side == "sources"
}

/// `fm_bounds` for a page on this side.
fn fm_bounds_for(raw: &str, side: &str) -> Option<FmBounds> {
    fm_bounds(raw).filter(|b| !strict(side) || looks_like_frontmatter(&raw[b.start..b.close]))
}

/// The folder each side reads, and whether a text there must carry frontmatter to count.
fn side_dir(side: &str) -> &'static str {
    match side {
        "places" => PAGE_DIRS[0],
        "people" => PAGE_DIRS[1],
        "knowledge" => KNOWLEDGE_DIR,
        _ => SOURCES_DIR,
    }
}

/// Every .md under places/ and people/ (recursive), with its tags and kind. This is the set
/// `af place` and `af person` work on; the knowledge pages and sources are `scan_tagged`.
pub fn scan_pages(ctx: &Ctx) -> R<Vec<Page>> {
    scan_sides(ctx, &["places", "people"])
}

/// Every page that can carry a tag: places, people, the live knowledge pages, and the sources
/// that have frontmatter (not `INDEX.md`, not `inbox/`).
pub fn scan_tagged(ctx: &Ctx) -> R<Vec<Page>> {
    scan_sides(ctx, &["places", "people", "knowledge", "sources"])
}

fn scan_sides(ctx: &Ctx, sides: &[&'static str]) -> R<Vec<Page>> {
    let mut pages = Vec::new();
    for &side in sides {
        let top = vpath(ctx, side_dir(side));
        if !pyos::isdir(&top) {
            continue;
        }
        let mut files: Vec<String> = Vec::new();
        pyos::walk(&top, &mut |dirpath: &str, dirnames: &mut Vec<String>, filenames: &[String]| {
            dirnames.retain(|d| {
                !d.starts_with('.')
                    && if side == "sources" { !(d == "inbox" && dirpath == top) } else { d != "_archive" && d != "_history" }
            });
            for f in filenames {
                if !f.to_lowercase().ends_with(".md") {
                    continue;
                }
                let index_like = f.eq_ignore_ascii_case("INDEX.md") || (side == "knowledge" && f.eq_ignore_ascii_case("README.md"));
                if (side == "sources" || side == "knowledge") && index_like {
                    continue;
                }
                files.push(pyos::join(dirpath, f));
            }
        });
        files.sort();
        for path in files {
            let rel = ctx.rel(&path)?;
            let text = if side == "places" || side == "people" {
                match TextFile::load(&path)? {
                    Some(f) => f.text,
                    None => continue,
                }
            } else {
                // Read as the bytes are; a source that is not UTF-8 has no frontmatter this reads.
                match std::fs::read(&path) {
                    Ok(b) => String::from_utf8_lossy(&b).into_owned(),
                    Err(e) => die!("{rel} could not be read ({}); close whatever holds it, then rerun", io_reason(&e)),
                }
            };
            let bounds = fm_bounds_for(&text, side);
            if side == "sources" && bounds.is_none() {
                continue;
            }
            let (has_tags_field, tags, kind) = match bounds {
                Some(b) => {
                    let fm = text[b.start..b.close].replace("\r\n", "\n");
                    (rx(r"(?m)^tags:").is_match(&fm), fm_list_values(&fm, "tags"), get_scalar(&fm, "kind").filter(|k| !k.is_empty()))
                }
                None => (false, Vec::new(), None),
            };
            pages.push(Page { rel, path, tags, kind, side, has_tags_field });
        }
    }
    Ok(pages)
}

/// The comment after the value on a `tags:` line (`tags: [a] # why`, or `tags: # why` above a block
/// list), with the space before it; empty when there is none.
fn trailing_comment(first_line: &str) -> &str {
    let value = first_line.strip_prefix("tags:").unwrap_or("");
    if value.trim_start().starts_with('#') {
        return value;
    }
    match value.find(']') {
        Some(i) if value[i + 1..].trim_start().starts_with('#') => &value[i + 1..],
        _ => "",
    }
}

/// The mapping a frontmatter block reads as with the crate's YAML reader, if it reads as one.
fn fm_mapping(fm: &str) -> Option<Vec<(PyVal, PyVal)>> {
    match safe_load(&fm.replace("\r\n", "\n"), "frontmatter", false) {
        Ok(PyVal::Dict(m)) => Some(m),
        _ => None,
    }
}

/// Is `new` the mapping `old` is, but for `tags`, which is the wanted list? An `old` that does not
/// read as a mapping (an unquoted colon in a summary, say) cannot be checked and passes. A tag the
/// reader turns into another type (`yes`, `42`) is not held against the list.
fn same_but_tags(old: &str, new: &str, tags: &[String]) -> bool {
    let Some(old) = fm_mapping(old) else { return true };
    let Some(new) = fm_mapping(new) else { return false };
    let rest = |m: &[(PyVal, PyVal)]| {
        let mut v: Vec<(String, String)> =
            m.iter().filter(|(k, _)| k.as_str() != Some("tags")).map(|(k, v)| (format!("{k:?}"), format!("{v:?}"))).collect();
        v.sort();
        v
    };
    if rest(&old) != rest(&new) {
        return false;
    }
    match new.iter().find(|(k, _)| k.as_str() == Some("tags")).map(|(_, v)| v) {
        Some(PyVal::List(items)) => {
            items.len() == tags.len() && items.iter().zip(tags).all(|(i, t)| if let PyVal::Str(s) = i { s == t } else { true })
        }
        _ => false,
    }
}

/// A first line of `---` (after an optional BOM) whose next non-blank line is a key: a frontmatter
/// block that was opened and never closed. Followed by anything else the `---` is a horizontal rule.
fn unclosed_frontmatter(raw: &str) -> bool {
    let rest = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    let mut lines = rest.lines();
    lines.next().is_some_and(|l| l.trim_end() == "---") && lines.find(|l| !l.trim().is_empty()).is_some_and(is_key_line)
}

/// `raw` with its `tags:` field set to an inline list and not one other byte changed: the line
/// endings, the BOM, the body (trailing blank lines, a `---` line of its own, a missing final
/// newline) and every other frontmatter line stay as they are. A `tags:` field already there goes as
/// one unit, a block list or a flow list wrapped over lines included, so no item line is left behind;
/// a comment after the value on the first line stays (one on the last line of a wrapped list is lost).
/// A field this cannot own is refused and the text left alone: an indented line or list item after
/// it (blank and comment lines skipped) that is not part of it, two `tags` keys, or one spelled
/// another way (`"tags":`, `tags :`). No field yet: the line is added at the end of the frontmatter.
/// On a knowledge page or source the block must also look like frontmatter (`looks_like_frontmatter`)
/// and, when the old block reads as a YAML mapping, the new one must read as the same mapping with
/// the wanted tags; otherwise the text is refused untouched. No frontmatter: refused, except on a
/// knowledge page, which gets a block of its own above the text; a block opened with `---` and
/// never closed, whose next line is a key, is refused. `label` names the page in a refusal.
pub fn rewrite_tags(raw: &str, tags: &[String], label: &str, side: &str) -> R<String> {
    let line = format!("tags: [{}]", tags.join(", "));
    let b = match fm_bounds_for(raw, side) {
        Some(b) => b,
        None if side == "knowledge" => {
            if fm_bounds(raw).is_none() && unclosed_frontmatter(raw) {
                die!("{label}: the frontmatter is opened with --- and never closed; close it by hand, then rerun");
            }
            let bom = if raw.starts_with('\u{feff}') { '\u{feff}'.len_utf8() } else { 0 };
            let eol = if raw[bom..].find('\n').is_some_and(|i| raw[bom..][..i].ends_with('\r')) { "\r\n" } else { "\n" };
            return Ok(format!("{}---{eol}{line}{eol}---{eol}{}", &raw[..bom], &raw[bom..]));
        }
        None => die!("{label} has no frontmatter block"),
    };
    let fm = &raw[b.start..b.close];
    let keys = rx(r#"(?m)^["']?tags["']?[ \t]*:"#).find_iter(fm).count();
    let span = fm_list_span(fm, "tags");
    if keys > 1 {
        die!("{label}: the frontmatter has {keys} tags keys; fix the frontmatter by hand, then rerun");
    }
    if keys == 1 && span.is_none() {
        die!("{label}: the frontmatter has a tags key that is not written `tags:`; fix the frontmatter by hand, then rerun");
    }
    let new_fm = match span {
        Some((s, e)) => {
            let after = &fm[e..];
            let next = after
                .strip_prefix('\n')
                .unwrap_or(after)
                .lines()
                .find(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
                .unwrap_or("");
            if next.starts_with(' ') || next.starts_with('\t') || rx(r"\A-([ \t]|$)").is_match(next) {
                die!("{label}: the tags: field is followed by an indented line that is not part of it ({next:?}); fix the frontmatter by hand, then rerun");
            }
            let cr = if fm[s..e].ends_with('\r') { "\r" } else { "" };
            let comment = trailing_comment(fm[s..e].split('\n').next().unwrap_or("").trim_end_matches('\r'));
            format!("{}{line}{comment}{cr}{}", &fm[..s], &fm[e..])
        }
        None => format!("{fm}{line}{}", b.eol),
    };
    if strict(side) && !same_but_tags(fm, &new_fm, tags) {
        die!("{label}: the result would not be the same frontmatter with new tags; fix the frontmatter by hand, then rerun");
    }
    Ok(format!("{}{new_fm}{}", &raw[..b.start], &raw[b.close..]))
}

/// Why a file operation failed: a permission or sharing error is "read-only or open in another
/// program"; anything else is the system's own text.
fn io_reason(e: &std::io::Error) -> String {
    if e.kind() == std::io::ErrorKind::PermissionDenied || matches!(e.raw_os_error(), Some(5 | 32 | 33)) {
        "it is read-only or open in another program".to_string()
    } else {
        e.to_string()
    }
}

/// A page's text exactly as it lies on disk; a refusal, not a crash, when it cannot be read.
fn read_raw(path: &str, label: &str) -> R<String> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => die!("{label}: no such page"),
        Err(e) => die!("{label} could not be read ({}); nothing was written", io_reason(&e)),
    };
    match String::from_utf8(bytes) {
        Ok(s) => Ok(s),
        Err(_) => die!("{label} is not UTF-8 text; nothing was written"),
    }
}

/// Rewrite the `tags:` field on one page; see `rewrite_tags`. The page is the only thing written
/// (`af tag add` leaves the registry and the log alone), through a temp file and a rename, so a
/// failure leaves the page as it was and ends in a refusal that names it (`label`, the vault path).
pub fn write_page_tags(path: &str, label: &str, tags: &[String], side: &str) -> R<()> {
    let raw = read_raw(path, label)?;
    let new = rewrite_tags(&raw, tags, label, side)?;
    if new != raw {
        if std::fs::metadata(path).map(|m| m.permissions().readonly()).unwrap_or(false) {
            die!("{label} is read-only; nothing was written");
        }
        if let Err(why) = try_write_raw(path, &new) {
            die!("{label} could not be written ({why}); nothing was changed");
        }
    }
    Ok(())
}

/// Write a text byte for byte through a temp file and a rename, so a reader sees the old file or the
/// new one; the rename is retried briefly while another process has the target open. The reason on
/// failure, for a caller that turns it into its own refusal.
fn try_write_raw(path: &str, text: &str) -> Result<(), String> {
    let tmp = format!("{path}.tmp{}", std::process::id());
    if let Err(e) = std::fs::write(&tmp, text.as_bytes()) {
        let _ = std::fs::remove_file(&tmp);
        return Err(io_reason(&e));
    }
    let mut tries = 0;
    loop {
        match std::fs::rename(&tmp, path) {
            Ok(()) => return Ok(()),
            Err(_) if tries < crate::life::RENAME_TRIES => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(crate::life::RENAME_PAUSE_MS));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(io_reason(&e));
            }
        }
    }
}

/// A change to the tag lists of several pages. `plan` makes every new text first, and refuses a page
/// that cannot be rewritten (a tags field it cannot own, a text that is not UTF-8, a read-only
/// file) before any file is written. `commit` then writes the pages in order, each through a temp
/// file and a rename; each page's original text is kept, so a write that fails puts back every
/// page already written and ends in a refusal that names the failing page. `rollback` does the
/// same for a caller whose next step (the registry) failed after the pages were written.
struct Rewrite {
    /// (path, vault-relative path, original text, new text)
    pages: Vec<(String, String, String, String)>,
    written: Vec<usize>,
}

impl Rewrite {
    fn plan(used: &[&Page], f: &dyn Fn(&[String]) -> Vec<String>) -> R<Rewrite> {
        let mut pages = Vec::new();
        for p in used {
            let raw = read_raw(&p.path, &p.rel)?;
            let new = rewrite_tags(&raw, &f(&p.tags), &p.rel, p.side)?;
            if new != raw && std::fs::metadata(&p.path).map(|m| m.permissions().readonly()).unwrap_or(false) {
                die!("{} is read-only; nothing was written", p.rel);
            }
            pages.push((p.path.clone(), p.rel.clone(), raw, new));
        }
        Ok(Rewrite { pages, written: Vec::new() })
    }

    fn rels(&self) -> Vec<String> {
        self.pages.iter().map(|p| p.1.clone()).collect()
    }

    /// Put back every page written so far; the pages that could not be put back.
    fn rollback(&mut self) -> Vec<String> {
        let mut stuck = Vec::new();
        for &i in self.written.iter().rev() {
            let (path, rel, raw, _) = &self.pages[i];
            if try_write_raw(path, raw).is_err() {
                stuck.push(rel.clone());
            }
        }
        self.written.clear();
        stuck
    }

    fn commit(&mut self) -> R<()> {
        for i in 0..self.pages.len() {
            if self.pages[i].2 == self.pages[i].3 {
                continue;
            }
            if let Err(why) = try_write_raw(&self.pages[i].0, &self.pages[i].3) {
                let rel = self.pages[i].1.clone();
                let stuck = self.rollback();
                if stuck.is_empty() {
                    die!("{rel} could not be written ({why}); every page already written was put back, and the registry did not change");
                }
                die!(
                    "{rel} could not be written ({why}), and putting back failed for {}: they now hold the new tag while the registry does not; make them writable and rerun the same command",
                    stuck.join(", ")
                );
            }
            self.written.push(i);
        }
        Ok(())
    }
}

/// Replace `from` with `to` in a tag list, keeping order and dropping the duplicate.
fn swap_tag(tags: &[String], from: &str, to: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in tags {
        let v = if t == from { to.to_string() } else { t.clone() };
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// Does this page carry a tag of the `topic` facet?
fn has_topic(reg: &Registry, p: &Page) -> bool {
    p.tags.iter().any(|t| reg.get(t).is_some_and(|x| x.facet == TOPIC_FACET))
}

fn uses_of<'a>(pages: &'a [Page], tag: &str) -> Vec<&'a Page> {
    pages.iter().filter(|p| p.tags.iter().any(|t| t == tag)).collect()
}

fn days_since(date: Option<&str>) -> Option<i64> {
    parse_iso_date(date).map(|d| (today_date() - d).num_days())
}

// ------------------------------------------------------------------ embeddings

fn cosine(a: &[f64], b: &[f64]) -> f64 {
    let dot: f64 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let na: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
    let nb: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na * nb)
    }
}

fn tag_text(t: &Tag) -> String {
    if t.scope.is_empty() {
        t.id.replace('-', " ")
    } else {
        format!("{}: {}", t.id.replace('-', " "), t.scope)
    }
}

/// The nearest usable tags to a phrase by embedding, best first; Err is the backend's class.
fn suggest(reg: &Registry, phrase: &str) -> Result<Vec<(String, f64)>, String> {
    let usable: Vec<&Tag> = reg.tags.iter().filter(|t| t.status == "active" || t.status == "proposed").collect();
    if usable.is_empty() {
        return Ok(Vec::new());
    }
    let mut texts = vec![phrase.to_string()];
    texts.extend(usable.iter().map(|t| tag_text(t)));
    let vecs = embed_texts(&texts).map_err(|e| format!("{}: {}", e.class, e.msg))?;
    let mut scored: Vec<(String, f64)> =
        usable.iter().zip(vecs.iter().skip(1)).map(|(t, v)| (t.id.clone(), cosine(&vecs[0], v))).collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored.retain(|(_, c)| *c >= SUGGEST_COSINE);
    scored.truncate(SUGGEST_N);
    Ok(scored)
}

// ------------------------------------------------------------------ the verbs

fn require_tag<'a>(reg: &'a Registry, slug: &str) -> R<&'a Tag> {
    match reg.get(slug) {
        Some(t) => Ok(t),
        None => match reg.by_alt(slug) {
            Some(o) => die!("'{slug}' is an alt label of '{}', not a tag; use {}", o.id, o.id),
            None => die!("no tag '{slug}' in {REGISTRY_REL}; af tag match {slug} finds the nearest, af tag propose adds it"),
        },
    }
}

pub fn cmd_init(ctx: &Ctx) -> R<i32> {
    let path = Registry::path(ctx);
    if pyos::isfile(&path) {
        die!("{REGISTRY_REL} already exists; edit it through af tag propose and friends");
    }
    let reg = Registry {
        facets: DEFAULT_FACETS.iter().map(|(n, a, s)| Facet { name: n.to_string(), applies: a.to_string(), single: *s }).collect(),
        tags: Vec::new(),
    };
    reg.save(ctx)?;
    append_log(ctx, "init", &format!("registry created with {} facets", reg.facets.len()))?;
    render_view(ctx, &reg)?;
    out::line(&format!("af tag init: wrote {REGISTRY_REL} with {} facets and no tags; af tag propose adds the first", reg.facets.len()));
    Ok(0)
}

pub fn cmd_match(ctx: &Ctx, phrase: &str, facet: Option<&str>, suggest_near: bool) -> R<i32> {
    let reg = Registry::require(ctx)?;
    if let Some(f) = facet {
        if reg.facet(f).is_none() {
            die!("no facet '{f}' in {REGISTRY_REL}; facets: {}", reg.facets.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", "));
        }
    }
    match reg.resolve(phrase, facet) {
        Resolved::Use(t, via) => {
            out::line(&format!("af tag match: {} · {} · {} · via {via}", t.id, t.status, t.facet));
            if !t.scope.is_empty() {
                out::line(&format!("  scope: {}", t.scope));
            }
            Ok(0)
        }
        Resolved::Deprecated(t) => {
            out::line(&format!(
                "af tag match: none usable for {} · '{}' is deprecated ({}); do not write it",
                pyos::py_repr_str(phrase),
                t.id,
                if t.scope.is_empty() { "no scope note" } else { t.scope.as_str() }
            ));
            Ok(0)
        }
        Resolved::None => {
            let slug = normalize(phrase);
            out::line(&format!(
                "af tag match: none for {} · if it is genuinely new: af tag propose {slug} --facet <{}> --scope \"<one line>\"",
                pyos::py_repr_str(phrase),
                facet.unwrap_or("facet")
            ));
            if suggest_near {
                match suggest(&reg, phrase) {
                    Ok(near) if near.is_empty() => out::line("  suggestions: none above the similarity floor"),
                    Ok(near) => {
                        let items: Vec<String> = near.iter().map(|(id, c)| format!("{id} ({c:.2})")).collect();
                        out::line(&format!("  suggestions (confirm before use, never a match): {}", items.join(", ")));
                    }
                    Err(e) => out::line(&format!("  suggestions unavailable ({e}); exact and alt labels only this run")),
                }
            }
            Ok(0)
        }
    }
}

pub fn cmd_propose(ctx: &Ctx, slug: &str, facet: &str, scope: &str, alt: &str) -> R<i32> {
    let mut reg = Registry::require(ctx)?;
    if !valid_slug(slug) {
        die!("'{slug}' is not a tag slug: lower case letters, digits, single hyphens, 40 characters at most");
    }
    if reg.facet(facet).is_none() {
        die!("no facet '{facet}' in {REGISTRY_REL}; facets: {}", reg.facets.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", "));
    }
    if let Some(t) = reg.get(slug) {
        die!("tag '{slug}' already exists ({}, {}); use it, or af tag promote {slug} if it is proposed", t.status, t.facet);
    }
    if let Some(o) = reg.by_alt(slug) {
        die!("'{slug}' is an alt label of '{}'; write {} instead", o.id, o.id);
    }
    let mut alts: Vec<String> = Vec::new();
    for a in alt.split(',').map(|a| normalize(a)).filter(|a| !a.is_empty()) {
        if a == slug {
            continue;
        }
        if let Some(t) = reg.get(&a) {
            die!("alt label '{a}' is already the tag '{}'", t.id);
        }
        if let Some(o) = reg.by_alt(&a) {
            die!("alt label '{a}' already belongs to '{}'", o.id);
        }
        if !alts.contains(&a) {
            alts.push(a);
        }
    }
    reg.tags.push(Tag {
        id: slug.to_string(),
        facet: facet.to_string(),
        status: "proposed".to_string(),
        alt: alts,
        scope: py_strip(scope).to_string(),
        broader: None,
        since: Some(today()),
        merged_into: None,
    });
    reg.save(ctx)?;
    append_log(ctx, "propose", &format!("{slug} · {facet} · {}", if scope.is_empty() { "(no scope)" } else { scope }))?;
    render_view(ctx, &reg)?;
    out::line(&format!(
        "af tag propose: {slug} · proposed · {facet}; it becomes active at {PROMOTE_USES} uses on {PROMOTE_PAGES} pages or af tag promote {slug}"
    ));
    Ok(0)
}

pub fn cmd_promote(ctx: &Ctx, slug: &str) -> R<i32> {
    let mut reg = Registry::require(ctx)?;
    let t = require_tag(&reg, slug)?;
    if t.status != "proposed" {
        die!("tag '{slug}' is {}, only a proposed tag is promoted", t.status);
    }
    reg.get_mut(slug).unwrap().status = "active".to_string();
    reg.save(ctx)?;
    append_log(ctx, "promote", slug)?;
    render_view(ctx, &reg)?;
    out::line(&format!("af tag promote: {slug} · active"));
    Ok(0)
}

pub fn cmd_deprecate(ctx: &Ctx, slug: &str, reason: &str) -> R<i32> {
    let mut reg = Registry::require(ctx)?;
    let t = require_tag(&reg, slug)?;
    if t.status == "merged" || t.status == "deprecated" {
        die!("tag '{slug}' is already {}", t.status);
    }
    reg.get_mut(slug).unwrap().status = "deprecated".to_string();
    reg.save(ctx)?;
    append_log(ctx, "deprecate", &format!("{slug}{}", if reason.is_empty() { String::new() } else { format!(" · {reason}") }))?;
    render_view(ctx, &reg)?;
    let pages = scan_tagged(ctx)?;
    let still: Vec<String> = uses_of(&pages, slug).iter().map(|p| p.rel.clone()).collect();
    out::line(&format!("af tag deprecate: {slug} · deprecated; no new uses"));
    if !still.is_empty() {
        out::line(&format!("  still on {} page(s): {} (af tag merge {slug} --into <replacement> rewrites them)", still.len(), still.join(", ")));
    }
    Ok(0)
}

pub fn cmd_drop(ctx: &Ctx, slug: &str) -> R<i32> {
    let mut reg = Registry::require(ctx)?;
    let t = require_tag(&reg, slug)?;
    if t.status != "proposed" {
        die!("tag '{slug}' is {}; only a proposed tag is dropped (af tag deprecate or af tag merge for an active one)", t.status);
    }
    let pages = scan_tagged(ctx)?;
    let used = uses_of(&pages, slug);
    if !used.is_empty() {
        die!("tag '{slug}' is on {} page(s) ({}); af tag merge {slug} --into <id> or af tag promote {slug}", used.len(), used.iter().map(|p| p.rel.as_str()).collect::<Vec<_>>().join(", "));
    }
    reg.tags.retain(|t| t.id != slug);
    reg.save(ctx)?;
    append_log(ctx, "drop", &format!("{slug} · proposed, unused"))?;
    render_view(ctx, &reg)?;
    out::line(&format!("af tag drop: {slug} removed (it was proposed and on no page)"));
    Ok(0)
}

/// Save the registry after the pages were written; if that fails, put the pages back and refuse.
fn save_or_put_back(ctx: &Ctx, reg: &Registry, rewrite: &mut Rewrite) -> R<()> {
    if let Err(e) = reg.save(ctx) {
        let stuck = rewrite.rollback();
        if !stuck.is_empty() {
            die!("{REGISTRY_REL} could not be saved ({e}), and putting back failed for {}: they hold the new tag while the registry does not; make them writable and rerun the same command", stuck.join(", "));
        }
        die!("{REGISTRY_REL} could not be saved ({e}); every page was put back, nothing changed");
    }
    Ok(())
}

/// After the registry is saved the change has happened, whatever the log and the view report: the
/// parts that could not be written, named for the refusal that ends the command.
fn write_log_and_view(ctx: &Ctx, reg: &Registry, verb: &str, detail: &str) -> Vec<String> {
    let mut missed = Vec::new();
    if let Err(e) = append_log(ctx, verb, detail) {
        missed.push(format!("{LOG_REL} ({e}); its line for this {verb} is missing"));
    }
    if let Err(e) = render_view(ctx, reg) {
        missed.push(format!("{VIEW_REL} ({e}); af tag render writes it"));
    }
    missed
}

/// The refusal for a change that is saved but whose log or view could not be written.
fn saved_but(verb: &str, missed: &[String]) -> R<i32> {
    die!(
        "the {verb} is saved: the pages and {REGISTRY_REL} agree. It could not write {}. Make the file writable, then run af tag render; do not rerun the {verb}",
        missed.join(" and ")
    )
}

/// `merge <from> --into <to>`: `from` is a tag (it becomes merged), an alt label of `to`, or
/// a label on pages that no tag owns; every page is rewritten and `to` gains the alt label.
pub fn cmd_merge(ctx: &Ctx, from: &str, into: &str) -> R<i32> {
    let mut reg = Registry::require(ctx)?;
    let from = normalize(from);
    let to = match reg.get(into) {
        Some(t) => t.clone(),
        None => match reg.by_alt(into) {
            Some(o) => die!("'{into}' is an alt label of '{}'; merge into {}", o.id, o.id),
            None => die!("no tag '{into}' in {REGISTRY_REL}; af tag propose {into} --facet <f> first"),
        },
    };
    if from == to.id {
        die!("'{from}' is already the tag '{}'", to.id);
    }
    if to.status == "merged" || to.status == "deprecated" {
        die!("tag '{}' is {}; merge into a live tag", to.id, to.status);
    }
    let pages = scan_tagged(ctx)?;
    let used: Vec<&Page> = uses_of(&pages, &from);
    let kind = if let Some(f) = reg.get(&from) {
        if f.status == "merged" && f.merged_into.as_deref() != Some(to.id.as_str()) {
            die!("tag '{from}' is already merged into '{}'", f.merged_into.clone().unwrap_or_default());
        }
        if f.facet != to.facet {
            die!(
                "'{from}' is in facet '{}' and '{}' in facet '{}'; a merge stays inside one facet (af tag deprecate {from}, then af tag add the pages)",
                f.facet, to.id, to.facet
            );
        }
        "tag"
    } else if let Some(o) = reg.by_alt(&from) {
        if o.id != to.id {
            die!("'{from}' is an alt label of '{}', not of '{}'", o.id, to.id);
        }
        "alt label"
    } else if used.is_empty() {
        die!("'{from}' is neither a tag nor on any page; nothing to merge");
    } else {
        "label"
    };
    let mut rewrite = Rewrite::plan(&used, &|tags| swap_tag(tags, &from, &to.id))?;
    rewrite.commit()?;
    let rewritten = rewrite.rels();
    let mut moved_alts: Vec<String> = vec![from.clone()];
    if let Some(f) = reg.get_mut(&from) {
        moved_alts.extend(f.alt.drain(..));
        f.status = "merged".to_string();
        f.merged_into = Some(to.id.clone());
    }
    {
        let target = reg.get_mut(&to.id).unwrap();
        for a in moved_alts {
            if a != target.id && !target.alt.contains(&a) {
                target.alt.push(a);
            }
        }
    }
    // Order: the pages, then the registry, the log and the view. A registry that cannot be saved
    // puts the pages back; once it is saved the pages and the registry agree.
    save_or_put_back(ctx, &reg, &mut rewrite)?;
    let missed = write_log_and_view(ctx, &reg, "merge", &format!("{from} -> {} · {} page(s) rewritten", to.id, rewritten.len()));
    out::line(&format!("af tag merge: {from} ({kind}) -> {} · {} page(s) rewritten", to.id, rewritten.len()));
    for r in &rewritten {
        out::line(&format!("  {r}"));
    }
    if !missed.is_empty() {
        return saved_but("merge", &missed);
    }
    Ok(0)
}

pub fn cmd_rename(ctx: &Ctx, old: &str, new: &str) -> R<i32> {
    let mut reg = Registry::require(ctx)?;
    let t = require_tag(&reg, old)?.clone();
    if t.status == "merged" {
        die!("tag '{old}' is merged into '{}'; rename that one", t.merged_into.clone().unwrap_or_default());
    }
    if !valid_slug(new) {
        die!("'{new}' is not a tag slug: lower case letters, digits, single hyphens, 40 characters at most");
    }
    if reg.get(new).is_some() {
        die!("tag '{new}' already exists; af tag merge {old} --into {new} instead");
    }
    if let Some(o) = reg.by_alt(new) {
        die!("'{new}' is an alt label of '{}'", o.id);
    }
    let mut alts = t.alt.clone();
    alts.push(old.to_string());
    reg.tags.push(Tag { id: new.to_string(), alt: alts, since: Some(today()), merged_into: None, ..t.clone() });
    {
        let o = reg.get_mut(old).unwrap();
        o.alt.clear();
        o.status = "merged".to_string();
        o.merged_into = Some(new.to_string());
    }
    let pages = scan_tagged(ctx)?;
    let mut rewrite = Rewrite::plan(&uses_of(&pages, old), &|tags| swap_tag(tags, old, new))?;
    rewrite.commit()?;
    let rewritten = rewrite.rels().len();
    save_or_put_back(ctx, &reg, &mut rewrite)?;
    let missed = write_log_and_view(ctx, &reg, "rename", &format!("{old} -> {new} · {rewritten} page(s) rewritten"));
    out::line(&format!("af tag rename: {old} -> {new} · {} · {rewritten} page(s) rewritten; {old} stays an alt label", t.status));
    if !missed.is_empty() {
        return saved_but("rename", &missed);
    }
    Ok(0)
}

/// Where a page path points, as a vault-relative page that can carry tags.
fn page_of<'a>(ctx: &Ctx, pages: &'a [Page], given: &str) -> R<&'a Page> {
    let abs = if pyos::isabs(given) { given.to_string() } else { vpath(ctx, &given.replace('\\', "/")) };
    let rel = match pyos::relpath(&abs, &ctx.root) {
        Ok(r) => r.replace('\\', "/"),
        Err(_) => given.replace('\\', "/"),
    };
    let rel = rel.trim_start_matches("./").to_string();
    match pages.iter().find(|p| p.rel == rel) {
        Some(p) => Ok(p),
        None => {
            let sources = format!("{SOURCES_DIR}/");
            if rel.starts_with(&sources) && rel.ends_with(".md") {
                if rel.starts_with(&format!("{SOURCES_DIR}/inbox/")) {
                    die!("{given} is in sources/inbox/, a staging folder; file it with af source add, then tag the filed source");
                }
                if pyos::isfile(&abs) && !rel.ends_with("/INDEX.md") {
                    die!("{given} has no frontmatter block, so it carries no tags; af source add --tag gives a new source its tags");
                }
            }
            die!("{given} is not a page under {}, {}, {} or {}", PAGE_DIRS[0], PAGE_DIRS[1], KNOWLEDGE_DIR, SOURCES_DIR)
        }
    }
}

/// The tags a page on `side` would carry after adding `wanted` to `current`: each phrase resolved
/// through the registry, checked against the facets that apply to the side, the single-valued
/// facets and the cap. Returns the new list and a note per phrase that was not an id. `what` names
/// the page in a refusal. Nothing is written here.
pub fn vet_tags(reg: &Registry, wanted: &[String], current: &[String], side: &str, what: &str) -> R<(Vec<String>, Vec<String>)> {
    let mut tags = current.to_vec();
    let mut notes: Vec<String> = Vec::new();
    for w in wanted {
        let id = match reg.resolve(w, None) {
            Resolved::Use(t, via) => {
                if via != "id" {
                    notes.push(format!("'{w}' written as {} ({via})", t.id));
                }
                t.clone()
            }
            Resolved::Deprecated(t) => die!("tag '{}' is deprecated; pick a live tag (af tag match {w})", t.id),
            Resolved::None => {
                die!("no tag matches '{w}' in {REGISTRY_REL}; af tag propose {} --facet <f> --scope \"<one line>\" first", normalize(w))
            }
        };
        if tags.contains(&id.id) {
            continue;
        }
        if let Some(f) = reg.facet(&id.facet) {
            if !facet_applies(&f.applies, side) {
                die!("tag '{}' is in facet '{}', a {} facet; this is a {side} page", id.id, f.name, f.applies);
            }
            if f.single {
                if let Some(other) = tags.iter().find(|t| reg.get(t).map(|x| x.facet == f.name).unwrap_or(false)) {
                    die!("{what} already carries '{other}' in the single-valued facet '{}'; af tag add after removing it, or keep {other}", f.name);
                }
            }
        }
        tags.push(id.id.clone());
    }
    let cap = tag_cap(side);
    if tags.len() > cap {
        die!("{what} would carry {} tags, over the cap of {cap}; keep the ones a search would use", tags.len());
    }
    Ok((tags, notes))
}

pub fn cmd_add(ctx: &Ctx, page: &str, wanted: &[String]) -> R<i32> {
    let reg = Registry::require(ctx)?;
    let pages = scan_tagged(ctx)?;
    let p = page_of(ctx, &pages, page)?;
    let (tags, notes) = vet_tags(&reg, wanted, &p.tags, p.side, &p.rel)?;
    let added = tags.len() - p.tags.len();
    // A knowledge page may start at its heading; the block of frontmatter is made for it.
    write_page_tags(&p.path, &p.rel, &tags, p.side)?;
    out::line(&format!("af tag add: {} · +{added} · tags: [{}]", p.rel, tags.join(", ")));
    for n in notes {
        out::line(&format!("  {n}"));
    }
    Ok(0)
}

pub fn cmd_facet_add(ctx: &Ctx, name: &str, applies: &str, single: bool) -> R<i32> {
    let mut reg = Registry::require(ctx)?;
    if !valid_slug(name) {
        die!("'{name}' is not a facet name: lower case letters, digits, single hyphens, 40 characters at most");
    }
    if !APPLIES.contains(&applies) {
        die!("--applies '{applies}' is not one of {}", APPLIES.join(", "));
    }
    if let Some(f) = reg.facet(name) {
        die!("facet '{name}' already exists ({}, {})", f.applies, if f.single { "one per page" } else { "multi" });
    }
    reg.facets.push(Facet { name: name.to_string(), applies: applies.to_string(), single });
    reg.save(ctx)?;
    append_log(ctx, "facet", &format!("{name} · {applies} · {}", if single { "single" } else { "multi" }))?;
    render_view(ctx, &reg)?;
    out::line(&format!("af tag facet add: {name} · {applies} · {}", if single { "one per page" } else { "multi" }));
    Ok(0)
}

pub fn cmd_list(ctx: &Ctx, facet: Option<&str>, status: Option<&str>) -> R<i32> {
    let reg = Registry::require(ctx)?;
    let pages = scan_tagged(ctx)?;
    let mut n = 0;
    for f in &reg.facets {
        if facet.map(|x| x != f.name).unwrap_or(false) {
            continue;
        }
        let mut tags: Vec<&Tag> = reg.tags.iter().filter(|t| t.facet == f.name && status.map(|s| t.status == s).unwrap_or(t.status != "merged")).collect();
        tags.sort_by(|a, b| a.id.cmp(&b.id));
        if tags.is_empty() {
            continue;
        }
        out::line(&format!("{} ({}, {}):", f.name, f.applies, if f.single { "one per page" } else { "multi" }));
        for t in tags {
            let uses = uses_of(&pages, &t.id).len();
            let alt = if t.alt.is_empty() { String::new() } else { format!(" · alt: {}", t.alt.join(", ")) };
            let scope = if t.scope.is_empty() { String::new() } else { format!(" · {}", t.scope) };
            out::line(&format!("  {} · {} · {uses} use(s){alt}{scope}", t.id, t.status));
            n += 1;
        }
    }
    if n == 0 {
        out::line("af tag list: no tags match");
    }
    Ok(0)
}

pub fn cmd_render(ctx: &Ctx) -> R<i32> {
    let reg = Registry::require(ctx)?;
    render_view(ctx, &reg)?;
    out::line(&format!("af tag render: wrote {VIEW_REL} from {} tags", reg.tags.len()));
    Ok(0)
}

/// `tags.md`: the vocabulary by side and facet, live tags only, retired ones in one line each.
pub fn render_view(ctx: &Ctx, reg: &Registry) -> R<()> {
    let mut s = String::new();
    s.push_str("---\nprivate: true\ntype: registry\n---\n# Tags\n\n");
    s.push_str(&format!(
        "A view of `tags.yaml`, rewritten by `af tag`; edit the vocabulary with `af tag`, never here. Before writing `tags:` on a place, person or knowledge page or a source: `af tag match \"<phrase>\"` per candidate, reuse the id it returns, `af tag propose` only for a genuinely new one; one tag per single-valued facet, {PAGE_TAG_FLOOR} to {PAGE_TAG_CAP} tags on a place or person page, one to {DOC_TAG_CAP} on a knowledge page or source, and nothing that is already a field (kind, area, status, groups). Search: `af search --tag <id>`.\n\n"
    ));
    let sections: [(&[&str], &str); 4] = [
        (&["places"], "Places"),
        (&["people"], "People"),
        (&["both"], "Places and people"),
        (&["knowledge", "sources", "context"], "Knowledge and sources"),
    ];
    for (sides, title) in sections {
        let facets: Vec<&Facet> = reg.facets.iter().filter(|f| sides.contains(&f.applies.as_str())).collect();
        if facets.is_empty() {
            continue;
        }
        s.push_str(&format!("## {title}\n\n"));
        for f in facets {
            let mut active: Vec<&str> = reg.tags.iter().filter(|t| t.facet == f.name && t.status == "active").map(|t| t.id.as_str()).collect();
            let mut proposed: Vec<&str> = reg.tags.iter().filter(|t| t.facet == f.name && t.status == "proposed").map(|t| t.id.as_str()).collect();
            active.sort();
            proposed.sort();
            let only = match f.applies.as_str() {
                "knowledge" => " (knowledge pages only)",
                "sources" => " (sources only)",
                _ => "",
            };
            let mut line = format!("- **{}**{only}{}: {}", f.name, if f.single { " (one per page)" } else { "" }, active.join(", "));
            if !proposed.is_empty() {
                line.push_str(&format!(" · proposed: {}", proposed.join(", ")));
            }
            s.push_str(&line);
            s.push('\n');
        }
        s.push('\n');
    }
    let mut retired: Vec<String> = Vec::new();
    for t in &reg.tags {
        match t.status.as_str() {
            "merged" => retired.push(format!("{} -> {}", t.id, t.merged_into.clone().unwrap_or_default())),
            "deprecated" => retired.push(format!("{} (deprecated)", t.id)),
            _ => {}
        }
    }
    if !retired.is_empty() {
        retired.sort();
        s.push_str(&format!("## Retired\n\n{}\n", retired.join(" · ")));
    }
    let path = vpath(ctx, VIEW_REL);
    let mut file = TextFile::load(&path)?.unwrap_or_else(|| TextFile::new(""));
    file.text = s;
    file.save(&path)
}

// ------------------------------------------------------------------ stats and lint

pub fn cmd_stats(ctx: &Ctx) -> R<i32> {
    let reg = Registry::require(ctx)?;
    let pages = scan_tagged(ctx)?;
    let count = |status: &str| reg.tags.iter().filter(|t| t.status == status).count();
    out::line(&format!(
        "af tag stats: {} tags · {} active · {} proposed · {} deprecated · {} merged · {} pages ({} places, {} people)",
        reg.tags.len(),
        count("active"),
        count("proposed"),
        count("deprecated"),
        count("merged"),
        pages.iter().filter(|p| p.side == "places" || p.side == "people").count(),
        pages.iter().filter(|p| p.side == "places").count(),
        pages.iter().filter(|p| p.side == "people").count()
    ));
    for f in &reg.facets {
        let mut tags: Vec<&Tag> = reg.tags.iter().filter(|t| t.facet == f.name && t.status != "merged").collect();
        tags.sort_by(|a, b| a.id.cmp(&b.id));
        if tags.is_empty() {
            continue;
        }
        let items: Vec<String> = tags
            .iter()
            .map(|t| {
                let uses = uses_of(&pages, &t.id).len();
                let mark = match t.status.as_str() {
                    "proposed" => "?",
                    "deprecated" => "!",
                    _ => "",
                };
                format!("{}{mark} {uses}", t.id)
            })
            .collect();
        out::line(&format!("  {} ({}): {}", f.name, f.applies, items.join(" · ")));
    }
    let active: Vec<&Tag> = reg.tags.iter().filter(|t| t.status == "active").collect();
    let singletons = active.iter().filter(|t| uses_of(&pages, &t.id).len() <= 1).count();
    let oldest_proposed = reg.tags.iter().filter(|t| t.status == "proposed").filter_map(|t| days_since(t.since.as_deref())).max();
    let venues: Vec<&Page> = pages.iter().filter(|p| p.side == "places" && !p.is_area()).collect();
    let covered = |facet: &str| -> String {
        if venues.is_empty() {
            return "n/a".into();
        }
        let n = venues.iter().filter(|p| p.tags.iter().any(|t| reg.get(t).map(|x| x.facet == facet).unwrap_or(false))).count();
        format!("{n}/{}", venues.len())
    };
    out::line(&format!(
        "  singleton share: {singletons}/{} active tags on one page or none (target under 15%)",
        active.len()
    ));
    out::line(&format!(
        "  proposed backlog: {} tag(s), oldest {} day(s)",
        count("proposed"),
        oldest_proposed.map(|d| d.to_string()).unwrap_or_else(|| "0".into())
    ));
    let facet_names: Vec<&str> = reg.facets.iter().filter(|f| f.applies == "places").take(3).map(|f| f.name.as_str()).collect();
    let coverage: Vec<String> = facet_names.iter().map(|f| format!("{f} {}", covered(f))).collect();
    out::line(&format!("  venue pages with a tag in: {}", coverage.join(" · ")));
    out::line(&format!(
        "  people pages tagged: {}/{}",
        pages.iter().filter(|p| p.side == "people" && !p.tags.is_empty()).count(),
        pages.iter().filter(|p| p.side == "people").count()
    ));
    for (side, label) in [("knowledge", "knowledge pages"), ("sources", "sources")] {
        let of_side: Vec<&Page> = pages.iter().filter(|p| p.side == side).collect();
        out::line(&format!("  {label} with a topic: {}/{}", of_side.iter().filter(|p| has_topic(&reg, p)).count(), of_side.len()));
    }
    Ok(0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    /// issue | note | promote | drop | review | near-duplicate | overload | under-tagged | scope
    pub class: &'static str,
    pub text: String,
}

/// Every lint finding, in a stable order. Pure: applies nothing.
pub fn lint_findings(reg: &Registry, pages: &[Page]) -> Vec<Finding> {
    let mut out: Vec<Finding> = Vec::new();
    let mut push = |class: &'static str, text: String| out.push(Finding { class, text });

    // --- drift on pages ---
    for p in pages {
        let mut seen_single: BTreeMap<&str, &str> = BTreeMap::new();
        for t in &p.tags {
            match reg.get(t) {
                None => match reg.by_alt(t) {
                    Some(o) => push("issue", format!("{}: tag '{t}' is an alt label of '{}'; af tag merge {t} --into {}", p.rel, o.id, o.id)),
                    None => push(
                        "issue",
                        format!("{}: tag '{t}' is not in {REGISTRY_REL}; af tag match {t}, then af tag merge {t} --into <id> or af tag propose {t} --facet <f> --scope \"<one line>\"", p.rel),
                    ),
                },
                Some(x) if x.status == "merged" => push(
                    "issue",
                    format!("{}: tag '{t}' was merged into '{}'; rerun af tag merge {t} --into {}", p.rel, x.merged_into.clone().unwrap_or_default(), x.merged_into.clone().unwrap_or_default()),
                ),
                Some(x) if x.status == "deprecated" => {
                    push("note", format!("{}: tag '{t}' is deprecated; remove it or af tag merge {t} --into <replacement>", p.rel))
                }
                Some(x) => {
                    if let Some(f) = reg.facet(&x.facet) {
                        if !facet_applies(&f.applies, p.side) {
                            push("note", format!("{}: tag '{t}' is in '{}', a {} facet, on a {} page", p.rel, f.name, f.applies, p.side));
                        }
                        if f.single {
                            if let Some(prev) = seen_single.get(f.name.as_str()) {
                                push("issue", format!("{}: two '{}' tags ({prev}, {t}); the facet takes one per page", p.rel, f.name));
                            } else {
                                seen_single.insert(f.name.as_str(), t.as_str());
                            }
                        }
                    }
                }
            }
        }
        if p.tags.len() > tag_cap(p.side) {
            push("issue", format!("{}: {} tags, over the cap of {}; keep the ones a search would use", p.rel, p.tags.len(), tag_cap(p.side)));
        }
    }

    // --- lifecycle ---
    let mut sorted: Vec<&Tag> = reg.tags.iter().collect();
    sorted.sort_by(|a, b| a.id.cmp(&b.id));
    for t in &sorted {
        let used = uses_of(pages, &t.id);
        let distinct_pages = used.len();
        let age = days_since(t.since.as_deref());
        match t.status.as_str() {
            "proposed" => {
                if distinct_pages >= PROMOTE_PAGES && distinct_pages >= PROMOTE_USES {
                    push("promote", format!("'{}' (proposed) is on {distinct_pages} pages; af tag promote {}", t.id, t.id));
                } else if distinct_pages == 0 && age.map(|d| d >= PROPOSED_STALE_DAYS).unwrap_or(false) {
                    push("drop", format!("'{}' (proposed {} days, no page); af tag drop {}", t.id, age.unwrap_or(0), t.id));
                }
            }
            "active" => {
                if distinct_pages <= 1 && age.map(|d| d >= SINGLETON_REVIEW_DAYS).unwrap_or(false) {
                    push(
                        "review",
                        format!("'{}' (active, {} days) is on {distinct_pages} page(s); keep it if the facet value is real, else af tag deprecate {}", t.id, age.unwrap_or(0), t.id),
                    );
                }
                if t.scope.is_empty() {
                    push("scope", format!("'{}' has no scope note; one line in {REGISTRY_REL} says what it covers and what it does not", t.id));
                }
            }
            _ => {}
        }
    }

    // --- near-duplicates: string variants, then co-occurrence ---
    let live: Vec<&Tag> = sorted.iter().copied().filter(|t| t.status == "active" || t.status == "proposed").collect();
    let mut paired: BTreeSet<(String, String)> = BTreeSet::new();
    for (i, a) in live.iter().enumerate() {
        for b in live.iter().skip(i + 1) {
            let key = (a.id.clone(), b.id.clone());
            let va = variants(&a.id);
            let vb = variants(&b.id);
            let variant = va.iter().skip(1).any(|v| *v == b.id) || vb.iter().skip(1).any(|v| *v == a.id);
            if variant {
                push("near-duplicate", format!("'{}' and '{}' are singular/plural or hyphen variants; af tag merge one --into the other", a.id, b.id));
                paired.insert(key);
                continue;
            }
            let ua: HashSet<&str> = uses_of(pages, &a.id).iter().map(|p| p.rel.as_str()).collect();
            let ub: HashSet<&str> = uses_of(pages, &b.id).iter().map(|p| p.rel.as_str()).collect();
            let both = ua.intersection(&ub).count();
            if both >= PROMOTE_USES && both == ua.len() && both == ub.len() {
                push("near-duplicate", format!("'{}' and '{}' always appear together ({both} pages); one tag, or one is broader", a.id, b.id));
                paired.insert(key);
            }
        }
    }

    // --- overload: one tag spread over kinds with no majority ---
    for t in &live {
        // Kinds are the place kinds and person types; a topic spread over knowledge pages and sources is not an overload.
        let used: Vec<&Page> = uses_of(pages, &t.id).into_iter().filter(|p| p.side == "places" || p.side == "people").collect();
        if used.len() < 2 * PROMOTE_USES {
            continue;
        }
        let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
        for p in &used {
            *kinds.entry(p.kind.clone().unwrap_or_else(|| p.side.to_string())).or_insert(0) += 1;
        }
        let top = kinds.values().max().copied().unwrap_or(0);
        if kinds.len() >= 2 && top * 10 < used.len() * 7 {
            let spread: Vec<String> = kinds.iter().map(|(k, n)| format!("{k} {n}")).collect();
            push("overload", format!("'{}' spans {} kinds ({}); consider narrower tags with '{}' as broader", t.id, kinds.len(), spread.join(", "), t.id));
        }
    }

    // --- under-tagged pages ---
    for p in pages {
        if p.side == "places" && !p.is_area() && p.tags.len() < PAGE_TAG_FLOOR {
            push("under-tagged", format!("{} ({}) has {} tag(s), floor {PAGE_TAG_FLOOR}; read it and af tag add what a search would use", p.rel, p.kind.clone().unwrap_or_else(|| "no kind".into()), p.tags.len()));
        }
        if p.side == "people" && p.tags.is_empty() && p.has_tags_field {
            push("under-tagged", format!("{} has no tags; af tag add an interest or closeness if the page shows one", p.rel));
        }
    }
    // A live knowledge page with no topic. A source with none is not a finding: old sources are tagged once, by a worker.
    let knowledge: Vec<&Page> = pages.iter().filter(|p| p.side == "knowledge").collect();
    if reg.facet(TOPIC_FACET).is_none() {
        if !knowledge.is_empty() {
            push(
                "under-tagged",
                format!("{} knowledge page(s) cannot carry a topic: {REGISTRY_REL} has no '{TOPIC_FACET}' facet; af tag facet add {TOPIC_FACET} --applies context", knowledge.len()),
            );
        }
    } else {
        for p in knowledge.iter().filter(|p| !has_topic(reg, p)) {
            push(
                "under-tagged",
                format!("{} has no {TOPIC_FACET} tag; read it and af tag add one to {DOC_TAG_CAP} (af tag match \"<phrase>\" --facet {TOPIC_FACET})", p.rel),
            );
        }
    }
    out
}

pub fn cmd_lint(ctx: &Ctx, fix: bool, embed: bool) -> R<i32> {
    let reg = Registry::require(ctx)?;
    let pages = scan_tagged(ctx)?;
    let mut findings = lint_findings(&reg, &pages);
    if embed {
        let live: Vec<&Tag> = reg.tags.iter().filter(|t| t.status == "active" || t.status == "proposed").collect();
        if live.len() >= 2 {
            let texts: Vec<String> = live.iter().map(|t| tag_text(t)).collect();
            match embed_texts(&texts) {
                Ok(vecs) => {
                    for i in 0..live.len() {
                        for j in i + 1..live.len() {
                            let c = cosine(&vecs[i], &vecs[j]);
                            if c >= 0.85 {
                                findings.push(Finding {
                                    class: "near-duplicate",
                                    text: format!("'{}' and '{}' embed alike ({c:.2}); a suggestion to judge, not a match", live[i].id, live[j].id),
                                });
                            }
                        }
                    }
                }
                Err(e) => findings.push(Finding { class: "note", text: format!("embedding pass skipped ({}: {})", e.class, e.msg) }),
            }
        }
    }
    // What was read, by side: pages is places and people, as it always was.
    let side = |s: &str| pages.iter().filter(|p| p.side == s).count();
    let plural = |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let counted = format!(
        "{} · {} · {}",
        plural(side("places") + side("people"), "page", "pages"),
        plural(side("knowledge"), "knowledge page", "knowledge pages"),
        plural(side("sources"), "source", "sources")
    );
    if findings.is_empty() {
        out::line(&format!("af tag lint: clean · {} tags · {counted}", reg.tags.len()));
        return Ok(0);
    }
    out::line(&format!("af tag lint: {} finding(s) · {} tags · {counted}", findings.len(), reg.tags.len()));
    for f in &findings {
        out::line(&format!("  {} · {}", f.class, f.text));
    }
    if fix {
        let mut applied = 0;
        let ids: Vec<(&'static str, String)> = findings
            .iter()
            .filter(|f| f.class == "promote" || f.class == "drop")
            .filter_map(|f| rx(r"\A'([^']+)'").captures(&f.text).map(|c| (f.class, c[1].to_string())))
            .collect();
        for (class, id) in ids {
            let code = if class == "promote" { cmd_promote(ctx, &id)? } else { cmd_drop(ctx, &id)? };
            if code == 0 {
                applied += 1;
            }
        }
        out::line(&format!("af tag lint: applied {applied} mechanical item(s); merges, splits and deprecations stay yours"));
    }
    Ok(0)
}

// ------------------------------------------------------------------ doctor

fn wikilink_re() -> regex::Regex {
    rx(r"\[\[([^\]|#\n]+)(?:[#|][^\]\n]*)?\]\]")
}

/// Slugs that resolve: every .md stem under places/ and people/ (recursive).
fn node_slugs(ctx: &Ctx) -> HashSet<String> {
    let mut slugs = HashSet::new();
    for dir in PAGE_DIRS {
        let top = vpath(ctx, dir);
        pyos::walk(&top, &mut |_d: &str, dirnames: &mut Vec<String>, filenames: &[String]| {
            dirnames.retain(|d| !d.starts_with('.'));
            for f in filenames {
                if f.to_lowercase().ends_with(".md") {
                    slugs.insert(pyos::splitext(f).0);
                }
            }
        });
    }
    slugs
}

/// `[[slug]]`s under workspace/life/ (minus the skipped folders) with no page to land on.
pub fn dangling_link_issues(ctx: &Ctx) -> R<Vec<String>> {
    let life = vpath(ctx, "workspace/life");
    if !pyos::isdir(&life) {
        return Ok(Vec::new());
    }
    let slugs = node_slugs(ctx);
    let mut files: Vec<String> = Vec::new();
    pyos::walk(&life, &mut |dirpath: &str, dirnames: &mut Vec<String>, filenames: &[String]| {
        dirnames.retain(|d| !d.starts_with('.') && !LINK_SKIP_DIRS.contains(&d.as_str()));
        for f in filenames {
            if f.to_lowercase().ends_with(".md") {
                files.push(pyos::join(dirpath, f));
            }
        }
    });
    files.sort();
    let mut issues = Vec::new();
    for path in files {
        let text = match TextFile::load(&path)? {
            Some(f) => f.text,
            None => continue,
        };
        let text = rx(r"(?s)```.*?```").replace_all(&text, "").into_owned();
        let text = rx(r"`[^`\n]*`").replace_all(&text, "").into_owned();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for c in wikilink_re().captures_iter(&text) {
            // A table cell writes an alias `[[slug\|alias]]`; the backslash is not the slug's.
            let slug = py_strip(&c[1]).trim_end_matches('\\').to_string();
            if slug.is_empty() || slugs.contains(&slug) || !seen.insert(slug.clone()) {
                continue;
            }
            issues.push(format!(
                "{}: [[{slug}]] has no page under workspace/life/places/ or people/; create it from its schema or fix the slug",
                ctx.rel(&path)?
            ));
        }
    }
    Ok(issues)
}

/// What `af doctor` adds for tags: (issues, notes).
pub fn doctor_checks(ctx: &Ctx) -> R<(Vec<String>, Vec<String>)> {
    let (mut issues, mut notes) = (Vec::new(), Vec::new());
    let pages = scan_tagged(ctx)?;
    let reg_path = Registry::path(ctx);
    match crate::error::soft(Registry::load(ctx))? {
        None => issues.push(format!("{REGISTRY_REL} does not parse; see the af tag error above")),
        Some(None) => {
            if pages.iter().any(|p| !p.tags.is_empty()) {
                notes.push(format!(
                    "{REGISTRY_REL} missing while {} page(s) carry tags; af tag init, then af tag propose each tag with its facet",
                    pages.iter().filter(|p| !p.tags.is_empty()).count()
                ));
            }
        }
        Some(Some(reg)) => {
            for f in lint_findings(&reg, &pages) {
                match f.class {
                    "issue" => issues.push(f.text),
                    "note" => notes.push(f.text),
                    _ => {}
                }
            }
            let view = vpath(ctx, VIEW_REL);
            if !pyos::isfile(&view) || pyos::getmtime(&view)? < pyos::getmtime(&reg_path)? {
                notes.push(format!("{VIEW_REL} is older than {REGISTRY_REL}; af tag render"));
            }
        }
    }
    issues.extend(dangling_link_issues(ctx)?);
    Ok((issues, notes))
}

// ------------------------------------------------------------------ dispatch

pub fn run(ctx: &Ctx, cmd: TagCmd) -> R<i32> {
    match cmd {
        TagCmd::Init => cmd_init(ctx),
        TagCmd::Match { phrase, facet, suggest } => cmd_match(ctx, &phrase, facet.as_deref(), suggest),
        TagCmd::Propose { slug, facet, scope, alt } => cmd_propose(ctx, &slug, &facet, &scope, &alt),
        TagCmd::Promote { slug } => cmd_promote(ctx, &slug),
        TagCmd::Deprecate { slug, reason } => cmd_deprecate(ctx, &slug, &reason),
        TagCmd::Drop { slug } => cmd_drop(ctx, &slug),
        TagCmd::Merge { from, into } => cmd_merge(ctx, &from, &into),
        TagCmd::Rename { old, new } => cmd_rename(ctx, &old, &new),
        TagCmd::Facet { cmd: FacetCmd::Add { name, applies, single } } => cmd_facet_add(ctx, &name, &applies, single),
        TagCmd::Add { page, tags } => cmd_add(ctx, &page, &tags),
        TagCmd::List { facet, status } => cmd_list(ctx, facet.as_deref(), status.as_deref()),
        TagCmd::Stats => cmd_stats(ctx),
        TagCmd::Lint { fix, embed } => cmd_lint(ctx, fix, embed),
        TagCmd::Render => cmd_render(ctx),
    }
}

/// `--tag` values for `af search`, resolved through the registry when there is one: an alt
/// label or a merged tag becomes its live id. (value as given, resolved id).
pub fn resolve_filter_tags(ctx: &Ctx, tags: &[String]) -> Vec<(String, String)> {
    let reg = match Registry::load(ctx) {
        Ok(Some(r)) => r,
        _ => return tags.iter().map(|t| (t.clone(), t.clone())).collect(),
    };
    tags.iter()
        .map(|t| match reg.resolve(t, None) {
            Resolved::Use(x, _) => (t.clone(), x.id.clone()),
            _ => (t.clone(), t.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_normalize_and_vary() {
        assert_eq!(normalize("Noodle Bowls!"), "noodle-bowls");
        assert_eq!(normalize("  Q&A "), "q-a");
        assert_eq!(normalize("---"), "");
        assert!(valid_slug("4k-screen") && valid_slug("a") && valid_slug("42"));
        assert!(!valid_slug("Bad") && !valid_slug("a--b") && !valid_slug("a-") && !valid_slug(""));
        let v = variants("patios");
        assert!(v.contains(&"patio".to_string()));
        assert!(variants("party").contains(&"parties".to_string()));
        assert!(variants("walk-in").contains(&"walkin".to_string()));
    }

    #[test]
    fn yaml_round_trips() {
        let reg = Registry {
            facets: vec![Facet { name: "cuisine".into(), applies: "places".into(), single: false }, Facet { name: "price".into(), applies: "places".into(), single: true }],
            tags: vec![
                Tag { id: "noodles".into(), facet: "cuisine".into(), status: "active".into(), alt: vec!["noodle".into()], scope: "Noodle houses; not \"ramen-only\" bars".into(), broader: None, since: Some("2026-01-02".into()), merged_into: None },
                Tag { id: "42".into(), facet: "price".into(), status: "merged".into(), alt: vec![], scope: String::new(), broader: None, since: None, merged_into: Some("noodles".into()) },
            ],
        };
        let text = reg.render_yaml();
        assert!(text.contains("  - id: \"42\"\n"), "{text}");
        assert!(text.contains("    alt: [noodle]\n"));
        let back = Registry::parse(&text).unwrap();
        assert_eq!(back.facets, reg.facets);
        assert_eq!(back.tags, reg.tags);
    }

    #[test]
    fn resolve_follows_alts_variants_and_merges() {
        let text = "facets:\n  - name: vibe\n    applies: places\n    cardinality: multi\ntags:\n  - id: quiet\n    facet: vibe\n    status: active\n    alt: [hushed, low-key]\n    scope: \"\"\n  - id: calm\n    facet: vibe\n    status: merged\n    alt: []\n    scope: \"\"\n    merged_into: quiet\n  - id: patio\n    facet: vibe\n    status: deprecated\n    alt: []\n    scope: \"\"\n";
        let reg = Registry::parse(text).unwrap();
        assert!(matches!(reg.resolve("Quiet", None), Resolved::Use(t, v) if t.id == "quiet" && v == "id"));
        assert!(matches!(reg.resolve("Low Key", None), Resolved::Use(t, v) if t.id == "quiet" && v == "alt label 'low-key'"));
        assert!(matches!(reg.resolve("calm", None), Resolved::Use(t, v) if t.id == "quiet" && v == "merged from 'calm'"));
        assert!(matches!(reg.resolve("patios", None), Resolved::Deprecated(t) if t.id == "patio"));
        assert!(matches!(reg.resolve("loud", None), Resolved::None));
        assert!(matches!(reg.resolve("quiet", Some("price")), Resolved::None));
    }

    #[test]
    fn a_bad_registry_is_a_refusal_that_names_the_line() {
        let text = "facets:\n  - name: vibe\ntags:\n  - id: Quiet\n    facet: vibe\n";
        let e = Registry::parse(text).err().unwrap().to_string();
        assert!(e.contains("tag id 'Quiet' is not a slug"), "{e}");
        let e = Registry::parse("facets:\n  - name: vibe\ntags:\n  - id: quiet\n    facet: mood\n").err().unwrap().to_string();
        assert!(e.contains("names facet 'mood', which is not declared"), "{e}");
    }

    fn set(raw: &str, create: bool) -> R<String> {
        rewrite_tags(raw, &["a".to_string(), "b".to_string()], "p.md", if create { "knowledge" } else { "places" })
    }

    #[test]
    fn rewrite_tags_changes_the_field_and_nothing_else() {
        assert_eq!(set("---\nid: x\n---\nbody\n", false).unwrap(), "---\nid: x\ntags: [a, b]\n---\nbody\n");
        assert_eq!(set("---\r\nid: x\r\ntags: [z]\r\nk: v\r\n---\r\nbody\r\n", false).unwrap(), "---\r\nid: x\r\ntags: [a, b]\r\nk: v\r\n---\r\nbody\r\n");
        assert_eq!(set("---\r\ntags:\r\n  - z\r\n---\r\n", false).unwrap(), "---\r\ntags: [a, b]\r\n---\r\n");
        // a rule line in the body is the body's; a rule that only starts with dashes closes nothing
        assert_eq!(set("---\nid: x\n----\nk: v\n---\n---\nt: 1\n", false).unwrap(), "---\nid: x\n----\nk: v\ntags: [a, b]\n---\n---\nt: 1\n");
        assert!(set("---\nid: x\nnever closed\n", false).unwrap_err().to_string().contains("has no frontmatter block"));
        assert!(set("# Heading\n", false).is_err());
        assert_eq!(set("# Heading\n", true).unwrap(), "---\ntags: [a, b]\n---\n# Heading\n");
        assert_eq!(set("# Heading\r\nx\r\n", true).unwrap(), "---\r\ntags: [a, b]\r\n---\r\n# Heading\r\nx\r\n");
        assert_eq!(set("", true).unwrap(), "---\ntags: [a, b]\n---\n");
    }

    #[test]
    fn io_reason_keeps_the_permission_wording_for_permission_and_sharing_errors_only() {
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert_eq!(io_reason(&denied), "it is read-only or open in another program");
        assert_eq!(io_reason(&std::io::Error::from_raw_os_error(32)), "it is read-only or open in another program");
        let other = std::io::Error::new(std::io::ErrorKind::InvalidInput, "the name is not valid");
        assert_eq!(io_reason(&other), "the name is not valid");
    }

    #[test]
    fn a_block_is_frontmatter_only_if_every_line_is_blank_a_comment_indented_a_list_item_or_a_key() {
        assert!(looks_like_frontmatter("id: a\n# c\ntags:\n  - x\n- y\n\nlist: [\n  a,\n]\n\"q\": 1\n"));
        assert!(looks_like_frontmatter("") && looks_like_frontmatter("\n\n"));
        assert!(!looks_like_frontmatter("From: a friend\nSubject: dinner\n\nsee you at 7\n"));
        assert!(!looks_like_frontmatter("# only a comment\n"));
        assert!(!looks_like_frontmatter("key:value\n"));
        assert!(is_key_line("tags : [a]") && is_key_line("a-b_1:") && !is_key_line("two words: x") && !is_key_line("url:http"));
    }

    #[test]
    fn swap_keeps_order_and_dedupes() {
        let tags: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(swap_tag(&tags, "b", "z"), vec!["a", "z", "c"]);
        assert_eq!(swap_tag(&tags, "b", "c"), vec!["a", "c"]);
    }
}
