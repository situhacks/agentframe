//! The spine shared by every verb: roots, clocks, project resolution, version chains,
//! the plugin host, and the export gate.

use crate::die;
use crate::error::{crash, CrashKind, R};
use crate::fm::{self, fm_list, fm_list_values, get_scalar, row_get, rx, split_fm};
use crate::pyglob;
use crate::pyos::{self, join, join_all};

pub const PROJECT_SCHEMA_VERSION: &str = "2026-07-19-v2";
pub const STATUS_ENUM: [&str; 5] = ["not_started", "drafting", "ready", "published", "deferred"];
pub const LIFECYCLE_ENUM: [&str; 3] = ["active", "complete", "cancelled"];
pub const EXPORTABLE_INGREDIENTS: [&str; 1] = ["image-prompts"];
pub const DEFAULT_DOMAIN: &str = "project-mgmt";

pub const BINARY_HEAD_HINT: &str = "the buttons stamp frontmatter, so a binary artifact cannot carry status. Point this row at a \
Markdown head beside it and record the binary in that head's exports[] under media/ \
(convention: library/process/deck-production.md)";

/// The repo root and the surfaces under it.
#[derive(Debug, Clone)]
pub struct Ctx {
    pub root: String,
    pub projects: String,
    pub domains: String,
    pub pipeline: String,
    pub studio: String,
    pub run_log: String,
}

impl Ctx {
    pub fn new(root: &str) -> Ctx {
        let root = pyos::abspath(root);
        Ctx {
            projects: join_all(&[&root, "workspace", "projects"]),
            domains: join_all(&[&root, "library", "domains"]),
            pipeline: join_all(&[&root, "workspace", "pipeline"]),
            studio: join_all(&[&root, "workspace", "studio"]),
            run_log: join_all(&[&root, "system", "logs", "af-runs.jsonl"]),
            root,
        }
    }

    /// `os.path.relpath(p, ROOT).replace("\\", "/")`
    pub fn rel(&self, p: &str) -> R<String> {
        Ok(pyos::relpath(p, &self.root)?.replace('\\', "/"))
    }
}

// ------------------------------------------------------------------ clocks

pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub fn today_date() -> chrono::NaiveDate {
    chrono::Local::now().date_naive()
}

pub fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
}

/// `datetime.now().astimezone().isoformat(timespec="seconds")`
pub fn now_iso() -> String {
    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

/// `datetime.date.fromisoformat(value[:10])`, None when it does not parse.
pub fn parse_iso_date(value: Option<&str>) -> Option<chrono::NaiveDate> {
    let value = value?;
    if value.is_empty() || value == "null" {
        return None;
    }
    let s = pyos::py_prefix(value, 10);
    let b = s.as_bytes();
    if !(b.len() == 7 || b.len() == 8 || b.len() == 10) {
        return None;
    }
    let digits = |from: usize, n: usize| -> Option<u32> {
        if from + n > b.len() {
            return None;
        }
        let mut v = 0u32;
        for &c in &b[from..from + n] {
            if !c.is_ascii_digit() {
                return None;
            }
            v = v * 10 + (c - b'0') as u32;
        }
        Some(v)
    };
    let year = digits(0, 4)? as i32;
    let mut p = 4;
    let sep = b.get(p) == Some(&b'-');
    if sep {
        p += 1;
    }
    if b.get(p) == Some(&b'W') {
        p += 1;
        let week = digits(p, 2)?;
        p += 2;
        let day = if p < b.len() {
            if sep {
                if b.get(p) != Some(&b'-') {
                    return None;
                }
                p += 1;
            }
            digits(p, 1)?
        } else {
            1
        };
        return chrono::NaiveDate::from_isoywd_opt(year, week, chrono::Weekday::try_from((day as u8).wrapping_sub(1)).ok()?);
    }
    let month = digits(p, 2)?;
    p += 2;
    if sep {
        if b.get(p) != Some(&b'-') {
            return None;
        }
        p += 1;
    }
    let day = digits(p, 2)?;
    chrono::NaiveDate::from_ymd_opt(year, month, day)
}

// ------------------------------------------------------------------ text formatting

/// Python `str.format(**kwargs)` for skeletons: `{key}`, `{{`, `}}`, `!s`/`!r`.
pub fn py_format(template: &str, kwargs: &[(&str, &str)]) -> R<String> {
    let t: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < t.len() {
        let c = t[i];
        if c == '{' {
            if t.get(i + 1) == Some(&'{') {
                out.push('{');
                i += 2;
                continue;
            }
            let close = match (i + 1..t.len()).find(|&k| t[k] == '}') {
                Some(k) => k,
                None => return Err(crash(CrashKind::Other, "ValueError: expected '}' before end of string")),
            };
            let field: String = t[i + 1..close].iter().collect();
            i = close + 1;
            let (head, spec) = match field.find(':') {
                Some(k) => (&field[..k], &field[k + 1..]),
                None => (&field[..], ""),
            };
            let (name, conv) = match head.find('!') {
                Some(k) => (&head[..k], Some(&head[k + 1..])),
                None => (head, None),
            };
            if name.is_empty() || name.chars().all(|c| c.is_ascii_digit()) {
                return Err(crash(
                    CrashKind::Other,
                    format!("IndexError: Replacement index {} out of range for positional args tuple", if name.is_empty() { "0" } else { name }),
                ));
            }
            let value = match kwargs.iter().find(|(k, _)| *k == name) {
                Some((_, v)) => v.to_string(),
                None => return Err(crash(CrashKind::Other, format!("KeyError: {}", pyos::py_repr_str(name)))),
            };
            let value = match conv {
                None | Some("s") => value,
                Some("r") | Some("a") => pyos::py_repr_str(&value),
                Some(_) => return Err(crash(CrashKind::Other, "ValueError: Unknown conversion specifier")),
            };
            if !spec.is_empty() {
                return Err(crash(CrashKind::Other, format!("ValueError: format spec '{spec}' is not supported by the Rust port")));
            }
            out.push_str(&value);
        } else if c == '}' {
            if t.get(i + 1) == Some(&'}') {
                out.push('}');
                i += 2;
                continue;
            }
            return Err(crash(CrashKind::Other, "ValueError: Single '}' encountered in format string"));
        } else {
            out.push(c);
            i += 1;
        }
    }
    Ok(out)
}

/// f-string rendering of an optional value (`None` prints as "None").
pub fn py_opt(v: &Option<String>) -> String {
    v.clone().unwrap_or_else(|| "None".to_string())
}

pub fn truthy(v: &Option<String>) -> bool {
    matches!(v, Some(s) if !s.is_empty())
}

// ------------------------------------------------------------------ io

pub fn read(path: &str) -> R<String> {
    pyos::read(path)
}

pub fn write(path: &str, text: &str) -> R<()> {
    pyos::write(path, text)
}

// ------------------------------------------------------------------ project resolution

/// Find a work folder by name (projects, completed, applications, posts), else by slug.
pub fn project_dir(ctx: &Ctx, arg: &str) -> R<String> {
    let completed = join(&ctx.projects, "completed");
    for base in [&ctx.projects, &completed] {
        let d = join(base, arg);
        if pyos::isfile(&join(&d, "project.md")) {
            return Ok(d);
        }
    }
    let d = join_all(&[&ctx.pipeline, "applications", arg]);
    if pyos::isfile(&join(&d, "application.md")) {
        return Ok(d);
    }
    let d = join_all(&[&ctx.studio, "posts", arg]);
    if pyos::isfile(&join(&d, "post.md")) {
        return Ok(d);
    }
    for base in [&ctx.projects, &completed] {
        if !pyos::isdir(base) {
            continue;
        }
        for name in pyos::sorted_listdir(base)? {
            let sp = join_all(&[base, &name, "project.md"]);
            if pyos::isfile(&sp) {
                let (sfm, _) = split_fm(&read(&sp)?, &sp)?;
                if get_scalar(&sfm, "slug").as_deref() == Some(arg) {
                    return Ok(join(base, &name));
                }
            }
        }
    }
    die!("project '{arg}' not found under workspace/projects/, workspace/pipeline/applications/, or workspace/studio/posts/")
}

/// The state file for a work folder.
pub fn state_doc(cdir: &str) -> &'static str {
    for name in ["application.md", "post.md"] {
        if pyos::isfile(&join(cdir, name)) {
            return name;
        }
    }
    "project.md"
}

/// (tracker row, project-relative file) for a row slug or a direct path.
pub fn resolve_deliverable_target(fm_text: &str, target: &str) -> R<(Option<String>, String)> {
    let norm = target.replace('\\', "/");
    if !norm.contains('/') {
        return match row_get(fm_text, target, "file") {
            Some(f) if !f.is_empty() => Ok((Some(target.to_string()), f)),
            _ => die!("row '{target}' has no file pointer"),
        };
    }
    let matches: Vec<String> = fm::all_rows(fm_text)
        .into_iter()
        .filter(|row| row_get(fm_text, row, "file").as_deref() == Some(norm.as_str()))
        .collect();
    let slug = if matches.len() == 1 { Some(matches[0].clone()) } else { None };
    Ok((slug, norm))
}

/// All strictly-versioned `{name}-v{N}.md` numbers in a folder.
pub fn versions_in(folder: &str, name: &str) -> Vec<u64> {
    let pattern = join(folder, &format!("{name}-v*.md"));
    let re = rx(&format!(r"\A{}-v(\d+)\.md\z", fm::esc(name)));
    let mut out = Vec::new();
    for p in pyglob::glob(&pattern, false) {
        let base = pyos::basename(&p);
        if let Some(c) = re.captures(&base) {
            // Python's int() of a long digit run never overflows; saturate instead.
            out.push(c[1].parse::<u64>().unwrap_or(u64::MAX));
        }
    }
    out
}

pub fn require_markdown_head(rel: &str) -> R<()> {
    if !rel.to_lowercase().ends_with(".md") {
        die!("{rel}: not a Markdown head — {BINARY_HEAD_HINT}");
    }
    Ok(())
}

pub fn version_re() -> regex::Regex {
    rx(r"\A(.+)-v(\d+)\.md\z")
}

/// Verify a versioned file is the highest v{N} in its folder; (name, N).
pub fn head_of(path: &str) -> R<Option<(String, u64)>> {
    let base = pyos::basename(path);
    let c = match version_re().captures(&base) {
        Some(c) => c,
        None => return Ok(None),
    };
    let name = c[1].to_string();
    let n: u64 = c[2].parse().unwrap_or(u64::MAX);
    let highest = match versions_in(&pyos::dirname(path), &name).into_iter().max() {
        Some(h) => h,
        None => return Err(crash(CrashKind::Other, "ValueError: max() arg is an empty sequence")),
    };
    if n != highest {
        die!("{base} is not the head ({name}-v{highest}.md exists) — point at the head or run doctor");
    }
    Ok(Some((name, n)))
}

pub fn touch_lifecycle(cfm: &str) -> R<String> {
    fm::set_scalar(cfm, "last_activity", &now_iso(), "project.md")
}

pub fn append_activity(cdir: &str, line: &str) -> R<()> {
    let path = join(cdir, "activity.md");
    let mut text = if pyos::isfile(&path) { read(&path)? } else { String::new() };
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    write(&path, &format!("{text}{} — {line}\n", stamp()))
}

// ------------------------------------------------------------------ plugin host

pub fn project_domain(cfm: &str) -> Option<String> {
    get_scalar(cfm, "domain")
}

/// (descriptor frontmatter, pack dir). The pack is the only artifact that knows a domain.
pub fn load_pack(ctx: &Ctx, domain: Option<&str>) -> R<(Option<String>, Option<String>)> {
    let domain = match domain {
        Some(d) if !d.is_empty() => d,
        _ => return Ok((None, None)),
    };
    let pack_dir = join(&ctx.domains, domain);
    let desc = join(&pack_dir, "pack.md");
    if !pyos::isfile(&desc) {
        return Ok((None, Some(pack_dir)));
    }
    let (pfm, _) = split_fm(&read(&desc)?, "pack.md")?;
    Ok((Some(pfm), Some(pack_dir)))
}

/// Deliverable basenames the owning pack gates behind filed exports.
pub fn pack_exportables(ctx: &Ctx, cfm: &str) -> R<Vec<String>> {
    let (desc, _) = load_pack(ctx, project_domain(cfm).as_deref())?;
    Ok(match desc {
        Some(d) if !d.is_empty() => fm_list(&d, "exportable"),
        _ => Vec::new(),
    })
}

pub fn exportable_ingredient(path: &str, extra: &[String]) -> Option<String> {
    let base = pyos::basename(path);
    let c = rx(r"\A(.+)-v\d+\.md\z").captures(&base)?;
    let name = c[1].to_string();
    if EXPORTABLE_INGREDIENTS.contains(&name.as_str()) || extra.contains(&name) {
        Some(name)
    } else {
        None
    }
}

/// None when valid/skipped; otherwise a short error label.
pub fn manifest_path_status(cdir: &str, owner_rel: &str, raw: &str) -> Option<&'static str> {
    let value = fm::clean_value(raw);
    if value.is_empty() || value.starts_with("http://") || value.starts_with("https://") {
        return None;
    }
    let croot = pyos::abspath(cdir);
    let inside = |path: &str| -> Option<String> {
        let candidate = pyos::abspath(path);
        match pyos::commonpath(&[croot.clone(), candidate.clone()]) {
            Ok(c) if c == croot => Some(candidate),
            _ => None,
        }
    };
    let (candidates, outside) = if pyos::isabs(&value) {
        let c = inside(&value);
        let outside = c.is_none();
        (vec![c], outside)
    } else {
        let owner_dir = pyos::dirname(owner_rel);
        let a = inside(&join(cdir, &value));
        let b = inside(&join(&join(cdir, &owner_dir), &value));
        let outside = a.is_none() && b.is_none();
        (vec![a, b], outside)
    };
    if outside {
        return Some("resolves outside project");
    }
    if !candidates.iter().flatten().any(|c| pyos::isfile(c)) {
        return Some("missing");
    }
    None
}

/// Blocking exports[] problems for an exportable deliverable; empty when clear.
pub fn export_gate_issues(cdir: &str, rel: &str, dfm: &str, extra: &[String]) -> Vec<String> {
    if exportable_ingredient(rel, extra).is_none() {
        return Vec::new();
    }
    let values = fm_list_values(dfm, "exports");
    if values.is_empty() {
        return vec!["exports[] is empty".to_string()];
    }
    values
        .iter()
        .filter_map(|v| manifest_path_status(cdir, rel, v).map(|s| format!("exports path {s}: {v}")))
        .collect()
}

/// Normalized project-relative path and its absolute target, refusing escapes.
pub fn safe_project_rel(cdir: &str, value: Option<&str>) -> R<(String, String)> {
    let value = pyos::py_strip(&value.unwrap_or("").replace('\\', "/")).to_string();
    if value.is_empty() || pyos::isabs(&value) || value == ".." || value.starts_with("../") || value.contains("/../") {
        die!("file path must stay inside the project and be project-relative");
    }
    if value.starts_with("workspace/") {
        die!(
            "'{value}' is repo-root-relative. --file takes a PROJECT-relative path: drop the \
leading workspace/projects/<slug>/ (or workspace/pipeline/applications/<slug>/, workspace/studio/posts/<slug>/) \
and pass only the part below it."
        );
    }
    let target = pyos::abspath(&join(cdir, &value));
    let acdir = pyos::abspath(cdir);
    if pyos::commonpath(&[acdir.clone(), target.clone()])? != acdir {
        die!("file path resolves outside the project");
    }
    Ok((value, target))
}

/// Resolve a row-owned head or an exact nested artifact head: (source_rel, move_pointer).
pub fn version_target(cdir: &str, cfm: &str, row: &str, artifact: Option<&str>) -> R<(String, bool)> {
    let rel = match row_get(cfm, row, "file") {
        Some(f) if !f.is_empty() => f,
        _ => die!("tracker row '{row}' not found or has no file"),
    };
    let artifact = match artifact {
        Some(a) if !a.is_empty() => a,
        _ => {
            let dpath = join(cdir, &rel);
            if !pyos::isfile(&dpath) {
                die!("deliverable file not found: {rel}");
            }
            require_markdown_head(&rel)?;
            if head_of(&dpath)?.is_none() {
                die!("{rel} is not a versioned -v{{N}}.md file");
            }
            return Ok((rel.replace('\\', "/"), true));
        }
    };
    if !fm::valid_slug(artifact) {
        die!("artifact name must contain only letters, numbers, underscores, and hyphens");
    }
    let parent_path = join(cdir, &rel);
    let folder = if pyos::isdir(&parent_path) {
        parent_path
    } else if pyos::isfile(&parent_path) {
        pyos::dirname(&parent_path)
    } else {
        die!("parent tracker path not found: {rel}")
    };
    let versions = versions_in(&folder, artifact);
    let n = match versions.into_iter().max() {
        Some(n) => n,
        None => die!(
            "artifact '{artifact}' has no versioned head under {}; start it with af draft",
            pyos::relpath(&folder, cdir)?.replace(pyos::SEP, "/")
        ),
    };
    let source_path = join(&folder, &format!("{artifact}-v{n}.md"));
    head_of(&source_path)?;
    Ok((pyos::relpath(&source_path, cdir)?.replace(pyos::SEP, "/"), false))
}
