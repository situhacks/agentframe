//! Frontmatter as text: the regex helpers the buttons edit state with.
//!
//! Nothing here round-trips YAML. Keys are found and replaced in place with the same
//! patterns the Python uses, so comment lines, blank lines, key order, and the quirks
//! of those patterns (an indented `status:` matching before a top-level one, a blank line
//! appearing before the closing fence after an append) carry over unchanged.

use crate::error::{crash, CrashKind, R};
use crate::pyos::{py_splitlines, py_strip};
use crate::die;
use regex::{Captures, Regex};
use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static CACHE: RefCell<HashMap<String, Regex>> = RefCell::new(HashMap::new());
}

/// Compile (once) and return a regex.
pub fn rx(pat: &str) -> Regex {
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(r) = c.get(pat) {
            return r.clone();
        }
        let r = Regex::new(pat).unwrap_or_else(|e| panic!("bad regex {pat}: {e}"));
        c.insert(pat.to_string(), r.clone());
        r
    })
}

pub fn esc(s: &str) -> String {
    regex::escape(s)
}

/// Expand a Python `re.sub` replacement template against one match.
/// Supports `\g<n>`, `\g<name>`, `\N` group refs, octal, and the escapes Python accepts;
/// a bad escape is the crash Python raises.
pub fn expand_template(template: &str, caps: &Captures, ngroups: usize) -> R<String> {
    let t: Vec<char> = template.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let bad = |msg: String, pos: usize| crash(CrashKind::Other, format!("error: {msg} at position {pos}"));
    let group = |out: &mut String, idx: usize, pos: usize| -> R<()> {
        if idx > ngroups {
            return Err(bad(format!("invalid group reference {idx}"), pos));
        }
        if let Some(m) = caps.get(idx) {
            out.push_str(m.as_str());
        }
        Ok(())
    };
    while i < t.len() {
        let c = t[i];
        if c != '\\' {
            out.push(c);
            i += 1;
            continue;
        }
        let pos = i;
        i += 1;
        if i >= t.len() {
            return Err(bad("bad escape (end of pattern)".to_string(), pos));
        }
        let c = t[i];
        i += 1;
        match c {
            'g' => {
                if i >= t.len() || t[i] != '<' {
                    return Err(bad("missing <".to_string(), i));
                }
                let close = (i + 1..t.len()).find(|&k| t[k] == '>');
                let close = match close {
                    Some(k) => k,
                    None => return Err(bad("missing >, unterminated name".to_string(), i + 1)),
                };
                let name: String = t[i + 1..close].iter().collect();
                i = close + 1;
                if let Ok(idx) = name.parse::<usize>() {
                    group(&mut out, idx, pos + 3)?;
                } else if let Some(m) = caps.name(&name) {
                    out.push_str(m.as_str());
                } else {
                    return Err(crash(CrashKind::Other, format!("IndexError: unknown group name {}", crate::pyos::py_repr_str(&name))));
                }
            }
            '0' => {
                let mut v = 0u32;
                let mut k = 0;
                while k < 2 && i < t.len() && ('0'..='7').contains(&t[i]) {
                    v = v * 8 + t[i].to_digit(8).unwrap();
                    i += 1;
                    k += 1;
                }
                out.push(char::from_u32(v).unwrap_or('\0'));
            }
            '1'..='9' => {
                let mut this = String::from(c);
                let mut octal = false;
                if i < t.len() && t[i].is_ascii_digit() {
                    this.push(t[i]);
                    i += 1;
                    let c0 = this.chars().next().unwrap();
                    let c1 = this.chars().nth(1).unwrap();
                    if ('0'..='7').contains(&c0) && ('0'..='7').contains(&c1) && i < t.len() && ('0'..='7').contains(&t[i]) {
                        this.push(t[i]);
                        i += 1;
                        octal = true;
                        let v = u32::from_str_radix(&this, 8).unwrap();
                        if v > 0o377 {
                            return Err(bad(format!("octal escape value \\{this} outside of range 0-0o377"), pos));
                        }
                        out.push(char::from_u32(v).unwrap());
                    }
                }
                if !octal {
                    group(&mut out, this.parse::<usize>().unwrap(), pos + 1)?;
                }
            }
            'a' => out.push('\u{07}'),
            'b' => out.push('\u{08}'),
            'f' => out.push('\u{0c}'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'v' => out.push('\u{0b}'),
            '\\' => out.push('\\'),
            c if c.is_ascii_alphabetic() => return Err(bad(format!("bad escape \\{c}"), pos)),
            c => {
                out.push('\\');
                out.push(c);
            }
        }
    }
    Ok(out)
}

/// `pat.sub(template, text, count=1)`
pub fn sub_first(re: &Regex, text: &str, template: &str) -> R<String> {
    match re.captures(text) {
        None => Ok(text.to_string()),
        Some(caps) => {
            let m = caps.get(0).unwrap();
            let rep = expand_template(template, &caps, re.captures_len() - 1)?;
            Ok(format!("{}{}{}", &text[..m.start()], rep, &text[m.end()..]))
        }
    }
}

// ------------------------------------------------------------------ split/join

pub fn split_fm_optional(text: &str) -> Option<(String, String)> {
    let re = rx(r"(?s)\A---\r?\n(.*?)\r?\n---\r?\n?");
    let caps = re.captures(text)?;
    let m = caps.get(0).unwrap();
    Some((caps[1].to_string(), text[m.end()..].to_string()))
}

pub fn split_fm(text: &str, path: &str) -> R<(String, String)> {
    match split_fm_optional(text) {
        Some(p) => Ok(p),
        None => die!("{path} has no frontmatter block"),
    }
}

pub fn join_fm(fm: &str, body: &str) -> String {
    format!("---\n{fm}\n---\n{body}")
}

// ------------------------------------------------------------------ scalars

pub fn clean_value(v: &str) -> String {
    let re = rx(r"\s+#.*\n?\z");
    let s = re.replace_all(v, "");
    py_strip(&s).trim_matches('"').to_string()
}

/// `yaml_quote(value)`
pub fn yaml_quote(value: Option<&str>) -> String {
    match value {
        None | Some("") => "null".to_string(),
        Some(v) => {
            let clean = crate::pyos::py_split_ws(v).join(" ");
            let clean = clean.replace('\\', "\\\\").replace('"', "\\\"");
            format!("\"{clean}\"")
        }
    }
}

pub fn get_scalar(fm: &str, key: &str) -> Option<String> {
    let re = rx(&format!(r"(?m)^\s*{}:[ \t]*(.*?)\s*$", esc(key)));
    re.captures(fm).map(|c| clean_value(&c[1]))
}

pub fn has_field(fm: &str, key: &str) -> bool {
    rx(&format!(r"(?m)^\s*{}:", esc(key))).is_match(fm)
}

/// `set_scalar`: replace `key: ...` at any indent; refuse when absent.
pub fn set_scalar(fm: &str, key: &str, value: &str, path: &str) -> R<String> {
    let re = rx(&format!(r"(?m)^(\s*{}:)[ \t]*.*$", esc(key)));
    if !re.is_match(fm) {
        die!("{path}: field '{key}' not found — fix the file or the schema first");
    }
    sub_first(&re, fm, &format!("\\g<1> {value}"))
}

/// `upsert_scalar(fm, key, value, before=...)`; `before=None` appends.
pub fn upsert_scalar(fm: &str, key: &str, value: &str, before: Option<&str>) -> R<String> {
    if has_field(fm, key) {
        return set_scalar(fm, key, value, "frontmatter");
    }
    match before {
        None => Ok(format!("{}\n{key}: {value}\n", fm.trim_end_matches('\n'))),
        Some(b) => {
            let re = rx(&format!(r"(?m)^{}:", esc(b)));
            match re.find(fm) {
                None => die!("frontmatter: cannot add optional field '{key}' before missing '{b}' block"),
                Some(m) => Ok(format!("{}{key}: {value}\n{}", &fm[..m.start()], &fm[m.start()..])),
            }
        }
    }
}

fn strip_quotes(s: &str) -> String {
    s.trim_matches(|c| c == '\'' || c == '"').to_string()
}

/// `fm_list(fm, key)`: inline `[a, b]` or a simple block list.
pub fn fm_list(fm: &str, key: &str) -> Vec<String> {
    let inline = rx(&format!(r"(?m)^{}:\s*\[(.*?)\]\s*$", esc(key)));
    if let Some(c) = inline.captures(fm) {
        return c[1]
            .split(',')
            .filter(|item| !py_strip(item).is_empty())
            .map(|item| strip_quotes(py_strip(item)))
            .collect();
    }
    let block = rx(&format!(r"(?m)^{}:\s*$\n((?:[ \t]+-[ \t]+.*(?:\n|$))*)", esc(key)));
    match block.captures(fm) {
        None => Vec::new(),
        Some(c) => py_splitlines(&c[1])
            .iter()
            .filter(|line| py_strip(line).starts_with("- "))
            .map(|line| {
                let s: String = py_strip(line).chars().skip(2).collect();
                strip_quotes(py_strip(&s))
            })
            .collect(),
    }
}

/// `fm_list_values(fm, key)`: inline or block-list values, cleaned.
pub fn fm_list_values(fm: &str, key: &str) -> Vec<String> {
    let inline = fm_list(fm, key);
    if !inline.is_empty() {
        return inline;
    }
    let re = rx(&format!(r"(?m)^\s*{}:\s*$", esc(key)));
    let m = match re.find(fm) {
        None => return Vec::new(),
        Some(m) => m,
    };
    let rest = &fm[m.end()..];
    let nxt = rx(r"(?m)^\S").find(rest);
    let block = &rest[..nxt.map(|n| n.start()).unwrap_or(rest.len())];
    let item = rx(r"^\s*-\s*(.*?)\s*\n?\z");
    let mut values = Vec::new();
    for line in py_splitlines(block) {
        if let Some(c) = item.captures(&line) {
            values.push(clean_value(&c[1]));
        }
    }
    values.into_iter().filter(|v| !v.is_empty()).collect()
}

pub fn list_from_value(value: Option<&str>) -> Vec<String> {
    let value = clean_value(value.unwrap_or(""));
    if value.is_empty() {
        return Vec::new();
    }
    if value.starts_with('[') && value.ends_with(']') {
        let inner: String = {
            let v: Vec<char> = value.chars().collect();
            if v.len() < 2 { String::new() } else { v[1..v.len() - 1].iter().collect() }
        };
        return inner.split(',').map(clean_value).filter(|s| !s.is_empty()).collect();
    }
    vec![value]
}

// ------------------------------------------------------------------ tracker rows

/// (start, end, inline_empty) of the top-level deliverables mapping.
pub fn deliverables_span(fm: &str) -> Option<(usize, usize, bool)> {
    let re = rx(r"(?m)^deliverables:\s*(\{\})?\s*$");
    let c = re.captures(fm)?;
    let m = c.get(0).unwrap();
    if c.get(1).is_some() {
        return Some((m.start(), m.end(), true));
    }
    let rest = &fm[m.end()..];
    let end = m.end() + rx(r"(?m)^\S").find(rest).map(|n| n.start()).unwrap_or(rest.len());
    Some((m.start(), end, false))
}

/// Span of a deliverable row, or of a pipeline application row on a board.
pub fn row_span(fm: &str, slug: &str) -> Option<(usize, usize)> {
    let dspan = match deliverables_span(fm) {
        Some(d) => d,
        None => {
            let c = rx(r"(?m)^applications:\s*(\{\})?\s*$").captures(fm)?;
            if c.get(1).is_some() {
                return None;
            }
            let m = c.get(0).unwrap();
            let rest = &fm[m.end()..];
            let end = m.end() + rx(r"(?m)^\S").find(rest).map(|n| n.start()).unwrap_or(rest.len());
            (m.start(), end, false)
        }
    };
    if dspan.2 {
        return None;
    }
    let (ds, de, _) = dspan;
    let block = &fm[ds..de];
    let m = rx(&format!(r"(?m)^  {}:\s*$", esc(slug))).find(block)?;
    let start = ds + m.start();
    let rest = &fm[ds + m.end()..de];
    let end = ds + m.end() + rx(r"(?m)^(  \S|\S)").find(rest).map(|n| n.start()).unwrap_or(rest.len());
    Some((start, end))
}

pub fn row_set(fm: &str, slug: &str, key: &str, value: &str) -> R<String> {
    let (s, e) = match row_span(fm, slug) {
        Some(sp) => sp,
        None => die!("tracker row '{slug}' not found"),
    };
    let block = &fm[s..e];
    let re = rx(&format!(r"(?m)^(    {}:)[ \t]*.*$", esc(key)));
    let block = if re.is_match(block) {
        sub_first(&re, block, &format!("\\g<1> {value}"))?
    } else {
        format!("{}\n    {key}: {value}\n", block.trim_end_matches('\n'))
    };
    Ok(format!("{}{}{}", &fm[..s], block, &fm[e..]))
}

pub fn row_get(fm: &str, slug: &str, key: &str) -> Option<String> {
    let (s, e) = row_span(fm, slug)?;
    let re = rx(&format!(r"(?m)^    {}:[ \t]*(.*?)\s*$", esc(key)));
    re.captures(&fm[s..e]).map(|c| clean_value(&c[1]))
}

pub fn valid_slug(slug: &str) -> bool {
    rx(r"\A[A-Za-z0-9][A-Za-z0-9_-]*\z").is_match(slug)
}

/// `row_add`: append a row, expanding an empty inline map.
pub fn row_add(fm: &str, slug: &str, fields: &[(&str, String)]) -> R<String> {
    if !valid_slug(slug) {
        die!("deliverable slug must contain only letters, numbers, underscores, and hyphens");
    }
    if row_span(fm, slug).is_some() {
        die!("project.md: tracker row '{slug}' already exists");
    }
    let (ds, de, inline) = match deliverables_span(fm) {
        Some(d) => d,
        None => die!("project.md: top-level deliverables mapping is missing"),
    };
    let mut lines = vec![format!("  {slug}:")];
    for (k, v) in fields {
        lines.push(format!("    {k}: {v}"));
    }
    let block = lines.join("\n");
    if inline {
        return Ok(format!("{}deliverables:\n{}{}", &fm[..ds], block, &fm[de..]));
    }
    let current = fm[ds..de].trim_end_matches('\n');
    Ok(format!("{}{}\n{}\n{}", &fm[..ds], current, block, fm[de..].trim_start_matches('\n')))
}

pub fn all_rows(fm: &str) -> Vec<String> {
    match deliverables_span(fm) {
        None | Some((_, _, true)) => Vec::new(),
        Some((ds, de, _)) => rx(r"(?m)^  ([A-Za-z0-9_-]+):\s*$")
            .captures_iter(&fm[ds..de])
            .map(|c| c[1].to_string())
            .collect(),
    }
}

// ------------------------------------------------------------------ boards

/// Row slugs in a pipeline board's `applications:` block.
pub fn pipe_rows(fm: &str) -> Vec<String> {
    let m = match rx(r"(?m)^applications:\s*(\{\})?\s*$").find(fm) {
        None => return Vec::new(),
        Some(m) => m,
    };
    let rest = &fm[m.end()..];
    let block = &rest[..rx(r"(?m)^\S").find(rest).map(|n| n.start()).unwrap_or(rest.len())];
    rx(r"(?m)^  ([A-Za-z0-9_-]+):\s*$").captures_iter(block).map(|c| c[1].to_string()).collect()
}

pub fn mapping_span(fm: &str, key: &str) -> Option<(usize, usize)> {
    let m = rx(&format!(r"(?m)^{}:\s*(?:\{{\}})?\s*$", esc(key))).find(fm)?;
    let rest = &fm[m.end()..];
    let end = m.end() + rx(r"(?m)^(?:\S[^:]*:|# )").find(rest).map(|n| n.start()).unwrap_or(rest.len());
    Some((m.start(), end))
}

pub fn mapping_rows(fm: &str, key: &str) -> Vec<String> {
    match mapping_span(fm, key) {
        None => Vec::new(),
        Some((s, e)) => rx(r"(?m)^  ([A-Za-z0-9_-]+):\s*$").captures_iter(&fm[s..e]).map(|c| c[1].to_string()).collect(),
    }
}

pub fn mapping_row_span(fm: &str, key: &str, slug: &str) -> Option<(usize, usize)> {
    let (s0, e0) = mapping_span(fm, key)?;
    let block = &fm[s0..e0];
    let m = rx(&format!(r"(?m)^  {}:\s*$", esc(slug))).find(block)?;
    let start = s0 + m.start();
    let rest = &fm[s0 + m.end()..e0];
    let end = s0 + m.end() + rx(r"(?m)^  \S").find(rest).map(|n| n.start()).unwrap_or(rest.len());
    Some((start, end))
}

pub fn mapping_row_get(fm: &str, mapping: &str, slug: &str, key: &str) -> Option<String> {
    let (s, e) = mapping_row_span(fm, mapping, slug)?;
    rx(&format!(r"(?m)^    {}:[ \t]*(.*?)\s*$", esc(key))).captures(&fm[s..e]).map(|c| clean_value(&c[1]))
}
