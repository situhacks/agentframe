//! `af tag`: the controlled vocabulary behind `tags:` on place and person pages.
//!
//! The registry `workspace/life/tags.yaml` is the truth (facets, then one entry per tag with
//! its facet, lifecycle status, alt labels, scope note); `tags-log.md` is its append-only
//! change log and `tags.md` a rendered view. The verbs: `match` resolves a phrase to a usable
//! tag (exact id, alt label, singular/plural or hyphen variant; an embedding is only ever a
//! suggestion), `propose` / `promote` / `deprecate` / `drop` move a tag through its life,
//! `merge` and `rename` rewrite every page and keep the old form as an alt label, `add`
//! writes validated tags on one page, `stats` and `lint` are the weekly pass's instruments,
//! and `doctor_checks` is what `af doctor` adds: unknown or retired tags on pages, a stale
//! rendered view, and a `[[slug]]` with no page under places/ or people/.

use crate::core::{parse_iso_date, today, today_date, Ctx};
use crate::die;
use crate::error::R;
use crate::fm::{fm_list_values, get_scalar, rx, split_fm_optional};
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
/// Folders under workspace/life/ whose `[[slug]]`s are not checked: immutable inputs,
/// archives, the voice corpus, and the append-only log.
pub const LINK_SKIP_DIRS: [&str; 5] = ["sources", "voice", "_history", "_archive", "log"];
pub const STATUSES: [&str; 4] = ["proposed", "active", "deprecated", "merged"];
pub const APPLIES: [&str; 3] = ["places", "people", "both"];
/// A proposed tag becomes active at this many uses on this many pages.
pub const PROMOTE_USES: usize = 3;
pub const PROMOTE_PAGES: usize = 2;
/// A proposed tag with no use after this many days is dropped.
pub const PROPOSED_STALE_DAYS: i64 = 60;
/// An active tag with one use after this many days is listed for review, never auto-dropped.
pub const SINGLETON_REVIEW_DAYS: i64 = 90;
pub const PAGE_TAG_CAP: usize = 8;
/// A place page (not an area) with fewer tags than this is under-tagged.
pub const PAGE_TAG_FLOOR: usize = 3;
/// Cosine at or above which an embedding neighbour is worth suggesting.
pub const SUGGEST_COSINE: f64 = 0.55;
pub const SUGGEST_N: usize = 3;

/// The starter facets `af tag init` writes; names only, no instance vocabulary.
pub const DEFAULT_FACETS: [(&str, &str, bool); 11] = [
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
];

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
    /// Write validated tags on one page (alt labels resolve; unknown and retired tags refuse)
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

// ------------------------------------------------------------------ registry

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Facet {
    pub name: String,
    /// places | people | both
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
                    die!("{REGISTRY_REL}: facet '{name}' applies '{applies}' is not one of places, people, both");
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
        s.push_str("# The controlled vocabulary for `tags:` on place and person pages. Edit it through `af tag`\n");
        s.push_str("# (match, propose, promote, merge, rename, deprecate); tags.md is the rendered view.\n");
        s.push_str("# facet: applies is places | people | both; cardinality single means one tag per page.\n");
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
    /// places | people, by folder.
    pub side: &'static str,
    pub has_tags_field: bool,
}

impl Page {
    pub fn is_area(&self) -> bool {
        self.kind.as_deref() == Some("area")
    }
}

/// Every .md under places/ and people/ (recursive), with its tags and kind.
pub fn scan_pages(ctx: &Ctx) -> R<Vec<Page>> {
    let mut pages = Vec::new();
    for (dir, side) in PAGE_DIRS.iter().zip(["places", "people"]) {
        let top = vpath(ctx, dir);
        if !pyos::isdir(&top) {
            continue;
        }
        let mut files: Vec<String> = Vec::new();
        pyos::walk(&top, &mut |dirpath: &str, dirnames: &mut Vec<String>, filenames: &[String]| {
            dirnames.retain(|d| !d.starts_with('.') && d != "_archive" && d != "_history");
            for f in filenames {
                if f.to_lowercase().ends_with(".md") {
                    files.push(pyos::join(dirpath, f));
                }
            }
        });
        files.sort();
        for path in files {
            let rel = ctx.rel(&path)?;
            let text = match TextFile::load(&path)? {
                Some(f) => f.text,
                None => continue,
            };
            let (fm, has_tags_field, tags, kind) = match split_fm_optional(&text) {
                Some((fm, _)) => {
                    let has = rx(r"(?m)^tags:").is_match(&fm);
                    (fm.clone(), has, fm_list_values(&fm, "tags"), get_scalar(&fm, "kind").filter(|k| !k.is_empty()))
                }
                None => (String::new(), false, Vec::new(), None),
            };
            let _ = fm;
            pages.push(Page { rel, path, tags, kind, side, has_tags_field });
        }
    }
    Ok(pages)
}

/// Rewrite the `tags:` field (inline form) on one page, leaving every other line as it is.
pub fn write_page_tags(path: &str, tags: &[String]) -> R<()> {
    let mut file = match TextFile::load(path)? {
        Some(f) => f,
        None => die!("{path}: no such page"),
    };
    let (fm, body) = match split_fm_optional(&file.text) {
        Some(p) => p,
        None => die!("{path} has no frontmatter block"),
    };
    let line = format!("tags: [{}]", tags.join(", "));
    let lines: Vec<String> = fm.split('\n').map(|s| s.to_string()).collect();
    let mut out: Vec<String> = Vec::with_capacity(lines.len() + 1);
    let mut i = 0;
    let mut replaced = false;
    while i < lines.len() {
        if !replaced && (lines[i] == "tags:" || lines[i].starts_with("tags:")) && !lines[i].starts_with("tags:-") {
            out.push(line.clone());
            replaced = true;
            i += 1;
            // a block list under it
            while i < lines.len() && rx(r"\A[ \t]+-[ \t]").is_match(&lines[i]) {
                i += 1;
            }
            continue;
        }
        out.push(lines[i].clone());
        i += 1;
    }
    if !replaced {
        out.push(line);
    }
    file.text = format!("---\n{}\n---\n{body}", out.join("\n"));
    file.save(path)
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
    let pages = scan_pages(ctx)?;
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
    let pages = scan_pages(ctx)?;
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
    let pages = scan_pages(ctx)?;
    let used: Vec<&Page> = uses_of(&pages, &from);
    let kind = if let Some(f) = reg.get(&from) {
        if f.status == "merged" && f.merged_into.as_deref() != Some(to.id.as_str()) {
            die!("tag '{from}' is already merged into '{}'", f.merged_into.clone().unwrap_or_default());
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
    let mut rewritten: Vec<String> = Vec::new();
    for p in &used {
        write_page_tags(&p.path, &swap_tag(&p.tags, &from, &to.id))?;
        rewritten.push(p.rel.clone());
    }
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
    reg.save(ctx)?;
    append_log(ctx, "merge", &format!("{from} -> {} · {} page(s) rewritten", to.id, rewritten.len()))?;
    render_view(ctx, &reg)?;
    out::line(&format!("af tag merge: {from} ({kind}) -> {} · {} page(s) rewritten", to.id, rewritten.len()));
    for r in &rewritten {
        out::line(&format!("  {r}"));
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
    let pages = scan_pages(ctx)?;
    let mut rewritten = 0usize;
    for p in uses_of(&pages, old) {
        write_page_tags(&p.path, &swap_tag(&p.tags, old, new))?;
        rewritten += 1;
    }
    reg.save(ctx)?;
    append_log(ctx, "rename", &format!("{old} -> {new} · {rewritten} page(s) rewritten"))?;
    render_view(ctx, &reg)?;
    out::line(&format!("af tag rename: {old} -> {new} · {} · {rewritten} page(s) rewritten; {old} stays an alt label", t.status));
    Ok(0)
}

/// Where a page path points, as a vault-relative page under places/ or people/.
fn page_of<'a>(ctx: &Ctx, pages: &'a [Page], given: &str) -> R<&'a Page> {
    let abs = if pyos::isabs(given) { given.to_string() } else { vpath(ctx, &given.replace('\\', "/")) };
    let rel = match pyos::relpath(&abs, &ctx.root) {
        Ok(r) => r.replace('\\', "/"),
        Err(_) => given.replace('\\', "/"),
    };
    let rel = rel.trim_start_matches("./").to_string();
    match pages.iter().find(|p| p.rel == rel) {
        Some(p) => Ok(p),
        None => die!("{given} is not a page under {} or {}", PAGE_DIRS[0], PAGE_DIRS[1]),
    }
}

pub fn cmd_add(ctx: &Ctx, page: &str, wanted: &[String]) -> R<i32> {
    let reg = Registry::require(ctx)?;
    let pages = scan_pages(ctx)?;
    let p = page_of(ctx, &pages, page)?;
    let mut tags = p.tags.clone();
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
            if f.applies != "both" && f.applies != p.side {
                die!("tag '{}' is in facet '{}', a {} facet; this is a {} page", id.id, f.name, f.applies, p.side);
            }
            if f.single {
                if let Some(other) = tags.iter().find(|t| reg.get(t).map(|x| x.facet == f.name).unwrap_or(false)) {
                    die!("{} already carries '{other}' in the single-valued facet '{}'; af tag add after removing it, or keep {other}", p.rel, f.name);
                }
            }
        }
        tags.push(id.id.clone());
    }
    if tags.len() > PAGE_TAG_CAP {
        die!("{} would carry {} tags, over the cap of {PAGE_TAG_CAP}; keep the ones a search would use", p.rel, tags.len());
    }
    let added = tags.len() - p.tags.len();
    write_page_tags(&p.path, &tags)?;
    out::line(&format!("af tag add: {} · +{added} · tags: [{}]", p.rel, tags.join(", ")));
    for n in notes {
        out::line(&format!("  {n}"));
    }
    Ok(0)
}

pub fn cmd_list(ctx: &Ctx, facet: Option<&str>, status: Option<&str>) -> R<i32> {
    let reg = Registry::require(ctx)?;
    let pages = scan_pages(ctx)?;
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
        "A view of `tags.yaml`, rewritten by `af tag`; edit the vocabulary with `af tag`, never here. Before writing `tags:` on a place or person page: `af tag match \"<phrase>\"` per candidate, reuse the id it returns, `af tag propose` only for a genuinely new one; one tag per single-valued facet, {PAGE_TAG_FLOOR} to {PAGE_TAG_CAP} tags a page, and nothing that is already a field (kind, area, status, groups). Search: `af search --tag <id>`.\n\n"
    ));
    for (side, title) in [("places", "Places"), ("people", "People"), ("both", "Places and people")] {
        let facets: Vec<&Facet> = reg.facets.iter().filter(|f| f.applies == side).collect();
        if facets.is_empty() {
            continue;
        }
        s.push_str(&format!("## {title}\n\n"));
        for f in facets {
            let mut active: Vec<&str> = reg.tags.iter().filter(|t| t.facet == f.name && t.status == "active").map(|t| t.id.as_str()).collect();
            let mut proposed: Vec<&str> = reg.tags.iter().filter(|t| t.facet == f.name && t.status == "proposed").map(|t| t.id.as_str()).collect();
            active.sort();
            proposed.sort();
            let mut line = format!("- **{}**{}: {}", f.name, if f.single { " (one per page)" } else { "" }, active.join(", "));
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
    let pages = scan_pages(ctx)?;
    let count = |status: &str| reg.tags.iter().filter(|t| t.status == status).count();
    out::line(&format!(
        "af tag stats: {} tags · {} active · {} proposed · {} deprecated · {} merged · {} pages ({} places, {} people)",
        reg.tags.len(),
        count("active"),
        count("proposed"),
        count("deprecated"),
        count("merged"),
        pages.len(),
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
                        if f.applies != "both" && f.applies != p.side {
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
        if p.tags.len() > PAGE_TAG_CAP {
            push("issue", format!("{}: {} tags, over the cap of {PAGE_TAG_CAP}; keep the ones a search would use", p.rel, p.tags.len()));
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
        let used = uses_of(pages, &t.id);
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
    out
}

pub fn cmd_lint(ctx: &Ctx, fix: bool, embed: bool) -> R<i32> {
    let reg = Registry::require(ctx)?;
    let pages = scan_pages(ctx)?;
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
    if findings.is_empty() {
        out::line(&format!("af tag lint: clean · {} tags · {} pages", reg.tags.len(), pages.len()));
        return Ok(0);
    }
    out::line(&format!("af tag lint: {} finding(s) · {} tags · {} pages", findings.len(), reg.tags.len(), pages.len()));
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
            let slug = py_strip(&c[1]).to_string();
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
    let pages = scan_pages(ctx)?;
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
        assert_eq!(normalize("Sharing Plates!"), "sharing-plates");
        assert_eq!(normalize("  R&B "), "r-b");
        assert_eq!(normalize("---"), "");
        assert!(valid_slug("19-plus") && valid_slug("a") && valid_slug("222"));
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
                Tag { id: "222".into(), facet: "price".into(), status: "merged".into(), alt: vec![], scope: String::new(), broader: None, since: None, merged_into: Some("noodles".into()) },
            ],
        };
        let text = reg.render_yaml();
        assert!(text.contains("  - id: \"222\"\n"), "{text}");
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

    #[test]
    fn swap_keeps_order_and_dedupes() {
        let tags: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        assert_eq!(swap_tag(&tags, "b", "z"), vec!["a", "z", "c"]);
        assert_eq!(swap_tag(&tags, "b", "c"), vec!["a", "c"]);
    }
}
