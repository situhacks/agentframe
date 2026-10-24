//! `media_intake.notes()` (system/tools/media_intake.py), the media-shelf drift notes the
//! full doctor surfaces. Cards are YAML frontmatter, loaded with PyYAML's semantics
//! (crate::pyyaml), so a hand-edited `consent: no` or unquoted date reads as Python reads it.

use crate::core::Ctx;
use crate::fm::rx;
use crate::pyos::{self, join, join_all};
use crate::pyyaml::{self, PyVal};

const BROWSER_CODECS: [&str; 4] = ["h264", "vp8", "vp9", "av1"];

/// `str(exc)` for an error the port carries as "ClassName: message".
fn exc_str(e: &crate::error::AfError) -> String {
    let m = e.to_string();
    match m.split_once(": ") {
        Some((cls, rest)) if !cls.is_empty() && cls.chars().all(|c| c.is_ascii_alphanumeric()) => rest.to_string(),
        _ => m,
    }
}

/// `open(path, encoding=...).read()` as a text file: strict UTF-8, universal newlines.
/// With `sig` false (plain utf-8) a BOM survives as U+FEFF, which PyYAML's scanner skips.
fn read_text(path: &str, sig: bool) -> Result<String, String> {
    if sig {
        return pyos::read(path).map_err(|e| exc_str(&e));
    }
    let bytes = std::fs::read(path).map_err(|e| exc_str(&pyos::os_error(&e, path)))?;
    let (bom, body) = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { ("\u{feff}", &bytes[3..]) } else { ("", &bytes[..]) };
    pyos::decode_text(body).map(|t| format!("{bom}{t}")).map_err(|e| exc_str(&e))
}

/// `os.path.expanduser` (ntpath flavour on Windows).
fn expanduser(path: &str) -> String {
    if !path.starts_with('~') {
        return path.to_string();
    }
    let chars: Vec<char> = path.chars().collect();
    let mut i = 1;
    while i < chars.len() && !pyos::is_sep(chars[i]) {
        i += 1;
    }
    let home = if cfg!(windows) {
        std::env::var("USERPROFILE").ok().or_else(|| {
            let drive = std::env::var("HOMEDRIVE").unwrap_or_default();
            std::env::var("HOMEPATH").ok().map(|p| join(&drive, &p))
        })
    } else {
        std::env::var("HOME").ok()
    };
    let mut userhome = match home {
        Some(h) => h,
        None => return path.to_string(),
    };
    if i != 1 {
        let target: String = chars[1..i].iter().collect();
        let current = std::env::var("USERNAME").or_else(|_| std::env::var("USER")).unwrap_or_default();
        if target != current {
            userhome = join(&pyos::dirname(&userhome), &target);
        }
    }
    let rest: String = chars[i..].iter().collect();
    format!("{userhome}{rest}")
}

/// `os.path.expandvars` (ntpath flavour: `%VAR%`, `$VAR`, `${VAR}`, '...' literal).
fn expandvars(path: &str) -> String {
    if !path.contains('$') && !path.contains('%') {
        return path.to_string();
    }
    let env = |k: &str| std::env::var(k).ok();
    let varchar = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
    let mut p: Vec<char> = path.chars().collect();
    let mut res = String::new();
    let mut index = 0usize;
    while index < p.len() {
        let c = p[index];
        if c == '\'' && cfg!(windows) {
            p = p[index + 1..].to_vec();
            match p.iter().position(|&x| x == '\'') {
                Some(k) => {
                    res.push('\'');
                    res.extend(&p[..=k]);
                    index = k;
                }
                None => {
                    res.push('\'');
                    res.extend(&p);
                    index = p.len().saturating_sub(1);
                }
            }
        } else if c == '%' && cfg!(windows) {
            if p.get(index + 1) == Some(&'%') {
                res.push('%');
                index += 1;
            } else {
                p = p[index + 1..].to_vec();
                match p.iter().position(|&x| x == '%') {
                    None => {
                        res.push('%');
                        res.extend(&p);
                        index = p.len().saturating_sub(1);
                    }
                    Some(k) => {
                        let var: String = p[..k].iter().collect();
                        res.push_str(&env(&var).unwrap_or_else(|| format!("%{var}%")));
                        index = k;
                    }
                }
            }
        } else if c == '$' {
            if p.get(index + 1) == Some(&'$') && cfg!(windows) {
                res.push('$');
                index += 1;
            } else if p.get(index + 1) == Some(&'{') {
                p = p[index + 2..].to_vec();
                match p.iter().position(|&x| x == '}') {
                    None => {
                        res.push_str("${");
                        res.extend(&p);
                        index = p.len().saturating_sub(1);
                    }
                    Some(k) => {
                        let var: String = p[..k].iter().collect();
                        res.push_str(&env(&var).unwrap_or_else(|| format!("${{{var}}}")));
                        index = k;
                    }
                }
            } else {
                let mut var = String::new();
                index += 1;
                while index < p.len() && varchar(p[index]) {
                    var.push(p[index]);
                    index += 1;
                }
                res.push_str(&env(&var).unwrap_or_else(|| format!("${var}")));
                if index < p.len() {
                    index -= 1;
                }
            }
        } else {
            res.push(c);
        }
        index += 1;
    }
    res
}

struct Shelf {
    shelf: String,
    cards: String,
    config: String,
}

impl Shelf {
    fn new(root: &str) -> Shelf {
        let shelf = join_all(&[root, "library", "assets", "media"]);
        Shelf { cards: join(&shelf, "cards"), config: join(&shelf, "media.yaml"), shelf }
    }

    /// `blob_root()`: `load_config().get("root") or "."`, then SHELF or the expanded path.
    fn blob_root(&self) -> Result<String, String> {
        let mut r = PyVal::Str(".".to_string());
        if pyos::isfile(&self.config) {
            let text = read_text(&self.config, false)?;
            // `yaml.safe_load(fh) or {}` then `cfg.setdefault("root", ".")`
            let cfg = pyyaml::safe_load(&text, &self.config, false)?;
            match cfg {
                PyVal::Dict(_) => {
                    if let Some(v) = cfg.get("root") {
                        r = v.clone();
                    }
                }
                v if !v.truthy() => {}
                v => return Err(format!("'{}' object has no attribute 'setdefault'", v.type_name())),
            }
        }
        if !r.truthy() || r.eq_str(".") {
            return Ok(self.shelf.clone());
        }
        Ok(pyos::abspath(&expandvars(&expanduser(&r.py_str()))))
    }
}

/// `read_card(path)[0]`: the frontmatter as `yaml.safe_load(...) or {}`, None without one.
fn read_card(path: &str) -> Result<Option<PyVal>, String> {
    let text = read_text(path, true)?;
    let c = match rx(r"(?s)\A---\r?\n(.*?)\r?\n---\r?\n?(.*)\z").captures(&text) {
        None => return Ok(None),
        Some(c) => c,
    };
    let rec = pyyaml::safe_load(&c[1], "<unicode string>", true)?;
    Ok(Some(if rec.truthy() { rec } else { PyVal::Dict(Vec::new()) }))
}

/// `all_cards()`: every card read before any is checked, as the Python does.
fn all_cards(shelf: &Shelf) -> Result<Vec<PyVal>, String> {
    let mut out = Vec::new();
    let mut names = pyos::listdir(&shelf.cards).map_err(|e| exc_str(&e))?;
    names.sort();
    for name in names {
        if !name.ends_with(".md") {
            continue;
        }
        let Some(rec) = read_card(&join(&shelf.cards, &name))? else { continue };
        // `if rec and rec.get("sha256")`
        if !rec.truthy() {
            continue;
        }
        if !matches!(rec, PyVal::Dict(_)) {
            return Err(format!("'{}' object has no attribute 'get'", rec.type_name()));
        }
        if rec.get("sha256").is_some_and(PyVal::truthy) {
            out.push(rec);
        }
    }
    Ok(out)
}

/// `rec["sha256"][:12]`, formatted as an f-string would.
fn sha12(v: &PyVal) -> Result<String, String> {
    match v {
        PyVal::Str(s) => Ok(pyos::py_prefix(s, 12)),
        PyVal::List(items) => Ok(PyVal::List(items.iter().take(12).cloned().collect()).py_str()),
        PyVal::Bytes(b) => Ok(PyVal::Bytes(b.iter().take(12).copied().collect()).py_str()),
        PyVal::Dict(_) => Err("slice(None, 12, None)".to_string()),
        other => Err(format!("'{}' object is not subscriptable", other.type_name())),
    }
}

/// `resolve_path(rec)`: `p = rec.get("path") or ""`, absolute as is, else under blob_root().
fn resolve_path(shelf: &Shelf, rec: &PyVal) -> Result<String, String> {
    let p = match rec.get("path").filter(|v| v.truthy()) {
        None => String::new(),
        Some(PyVal::Str(s)) => s.clone(),
        Some(PyVal::Bytes(b)) => {
            let s = String::from_utf8_lossy(b).into_owned();
            if pyos::isabs(&s) {
                return Ok(s);
            }
            return Err("Can't mix strings and bytes in path components".to_string());
        }
        Some(v) => return Err(format!("expected str, bytes or os.PathLike object, not {}", v.short_type_name())),
    };
    Ok(if pyos::isabs(&p) { p } else { join(&shelf.blob_root()?, &p) })
}

fn notes_inner(ctx: &Ctx) -> Result<Vec<String>, String> {
    let shelf = Shelf::new(&ctx.root);
    let mut out = Vec::new();
    if !pyos::isdir(&shelf.cards) {
        return Ok(out);
    }
    let cfg_root = shelf.blob_root()?;
    if cfg_root != shelf.shelf && !pyos::isdir(&cfg_root) {
        out.push(format!("library/assets/media/media.yaml: root {cfg_root} is not mounted"));
    }
    let week_ago = {
        let t = chrono::Local::now() - chrono::Duration::days(7);
        let micros = t.timestamp_subsec_micros();
        if micros == 0 {
            t.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
        } else {
            format!("{}.{:06}{}", t.format("%Y-%m-%dT%H:%M:%S"), micros, t.format("%:z"))
        }
    };
    let none = PyVal::None;
    for rec in all_cards(&shelf)? {
        let field = |k: &str| rec.get(k).unwrap_or(&none);
        let sid = sha12(field("sha256"))?;
        if !pyos::isfile(&resolve_path(&shelf, &rec)?) {
            out.push(format!("media cards/{sid}.md: file missing at {}", field("path").py_str()));
        }
        let codec = field("codec");
        if field("kind").eq_str("video") && codec.truthy() {
            // `rec["codec"] not in BROWSER_CODECS` hashes the value
            if !codec.is_hashable() {
                return Err(format!("unhashable type: '{}'", codec.type_name()));
            }
            let browser = codec.as_str().is_some_and(|c| BROWSER_CODECS.contains(&c));
            if !browser && !field("proxy").truthy() {
                out.push(format!(
                    "media cards/{sid}.md: {} source with no H.264 proxy — the render engine cannot play it",
                    codec.py_str()
                ));
            }
        }
        if field("review").eq_str("pending") {
            // `(rec.get("ingested_at") or "") < week_ago`
            let ingested = field("ingested_at");
            let stamp = match ingested {
                v if !v.truthy() => "",
                PyVal::Str(s) => s.as_str(),
                v => return Err(format!("'<' not supported between instances of '{}' and 'str'", v.type_name())),
            };
            if stamp < week_ago.as_str() {
                out.push(format!("media cards/{sid}.md: review pending for over a week"));
            }
        }
        if field("people").eq_str("others") && field("used_in").truthy() && !field("consent").truthy() {
            out.push(format!(
                "media cards/{sid}.md: people: others used in {} with no consent note",
                field("used_in").py_str()
            ));
        }
    }
    Ok(out)
}

/// Advisory drift on the operator's media shelf; a broken card never takes doctor down.
pub fn media_shelf_notes(ctx: &Ctx) -> Vec<String> {
    match notes_inner(ctx) {
        Ok(v) => v,
        Err(e) => vec![format!("media shelf check failed: {e}")],
    }
}
