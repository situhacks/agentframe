//! Chunk building: frontmatter split, fence-aware heading chunks with breadcrumbs, and the
//! version-head signals (`split_frontmatter`, `chunk_markdown`, `file_signals`).

use crate::fm::rx;
use crate::pyos::{self, py_splitlines, py_strip};
use std::collections::{HashMap, HashSet};

/// The characters Python's `re` matches with `\s` in a str pattern (`str.isspace()`),
/// as the inside of a character class. Rust's `\s` lacks U+001C..U+001F.
pub const WS_CHARS: &str =
    r"\t\n\x0B\x0C\r\x1C-\x20\x{85}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}";

pub const MAX_CHUNK_CHARS: usize = 9000;

fn fm_re() -> regex::Regex {
    rx(&format!(r"(?s)\A---[{WS_CHARS}]*\n(.*?)\n---[{WS_CHARS}]*\n?"))
}

fn heading_re() -> regex::Regex {
    rx(&format!(r"^(#{{1,6}})[{WS_CHARS}]+(.*)$"))
}

fn fence_re() -> regex::Regex {
    rx(&format!(r"^[{WS_CHARS}]*(```|~~~)"))
}

/// `VERSION_RE = re.compile(r"-v(\d+)\.[a-z0-9]+$", re.IGNORECASE)`
pub fn version_re() -> regex::Regex {
    rx(r"(?i)-v([0-9]+)\.[a-z0-9]+$")
}

pub fn version_of(rel: &str) -> Option<i64> {
    version_re().captures(rel).and_then(|c| c[1].parse::<i64>().ok())
}

fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// `split_frontmatter(text)`: (flat frontmatter map, body without frontmatter).
pub fn split_frontmatter(text: &str) -> (HashMap<String, String>, String) {
    let caps = match fm_re().captures(text) {
        Some(c) => c,
        None => return (HashMap::new(), text.to_string()),
    };
    let mut fm = HashMap::new();
    for line in py_splitlines(&caps[1]) {
        if let Some((key, val)) = line.split_once(':') {
            let v = py_strip(val).trim_matches('"').trim_matches('\'');
            fm.insert(py_strip(key).to_string(), v.to_string());
        }
    }
    let end = caps.get(0).unwrap().end();
    (fm, text[end..].to_string())
}

/// `chunk_markdown(body)`: [(breadcrumb titles, text)], split on headings outside code
/// fences; a section over MAX_CHUNK_CHARS splits on blank lines into `(part N)` chunks.
pub fn chunk_markdown(body: &str) -> Vec<(Vec<String>, String)> {
    let mut sections: Vec<(Vec<String>, String)> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut cur: Vec<String> = Vec::new();

    fn flush(sections: &mut Vec<(Vec<String>, String)>, stack: &[(usize, String)], cur: &mut Vec<String>) {
        let joined = cur.join("\n");
        let text = py_strip(&joined);
        if !text.is_empty() {
            sections.push((stack.iter().map(|(_, t)| t.clone()).collect(), text.to_string()));
        }
        cur.clear();
    }

    let (fence, heading) = (fence_re(), heading_re());
    let mut fenced = false;
    let mut fence_mark = String::new();
    for line in py_splitlines(body) {
        if let Some(fm) = fence.captures(&line) {
            let mark = fm[1].to_string();
            if !fenced {
                fenced = true;
                fence_mark = mark;
            } else if mark == fence_mark {
                fenced = false;
                fence_mark.clear();
            }
            cur.push(line);
            continue;
        }
        if !fenced {
            if let Some(hm) = heading.captures(&line) {
                flush(&mut sections, &stack, &mut cur);
                let level = hm[1].len();
                let title = py_strip(&hm[2]).to_string();
                while stack.last().map(|(l, _)| *l >= level).unwrap_or(false) {
                    stack.pop();
                }
                stack.push((level, title));
                continue;
            }
        }
        cur.push(line);
    }
    flush(&mut sections, &stack, &mut cur);

    if sections.is_empty() {
        let stripped = py_strip(body);
        return if stripped.is_empty() { Vec::new() } else { vec![(Vec::new(), stripped.to_string())] };
    }

    let mut out = Vec::new();
    for (crumbs, text) in sections {
        if char_len(&text) <= MAX_CHUNK_CHARS {
            out.push((crumbs, text));
            continue;
        }
        let mut parts: Vec<String> = Vec::new();
        let mut buf = String::new();
        let mut buf_len = 0usize;
        for para in text.split("\n\n") {
            let plen = char_len(para);
            if !buf.is_empty() && buf_len + plen + 2 > MAX_CHUNK_CHARS {
                parts.push(std::mem::take(&mut buf));
                buf = para.to_string();
                buf_len = plen;
            } else if !buf.is_empty() {
                buf.push_str("\n\n");
                buf.push_str(para);
                buf_len += plen + 2;
            } else {
                buf = para.to_string();
                buf_len = plen;
            }
        }
        if !buf.is_empty() {
            parts.push(buf);
        }
        let many = parts.len() > 1;
        for (i, part) in parts.into_iter().enumerate() {
            let mut crumb = crumbs.clone();
            if many {
                crumb.push(format!("(part {})", i + 1));
            }
            out.push((crumb, part));
        }
    }
    out
}

/// The breadcrumb stored with a chunk: `stem › h1 › h2`.
pub fn breadcrumb(rel: &str, crumbs: &[String]) -> String {
    let stem = pyos::splitext(&pyos::basename(rel)).0;
    let mut parts = vec![stem];
    parts.extend(crumbs.iter().cloned());
    parts.join(" › ")
}

/// `file_signals(rel_paths)[1]`: within a folder, every file at the folder's highest `-vN`
/// is a head; every unversioned file is its own head.
pub fn file_heads<'a, I: IntoIterator<Item = &'a String>>(rels: I) -> HashSet<String> {
    let mut top: HashMap<String, i64> = HashMap::new();
    let mut versioned: Vec<(String, String, i64)> = Vec::new();
    let mut heads = HashSet::new();
    for rel in rels {
        if let Some(v) = version_of(rel) {
            let folder = pyos::dirname(rel).replace('\\', "/");
            let e = top.entry(folder.clone()).or_insert(v);
            if v > *e {
                *e = v;
            }
            versioned.push((rel.clone(), folder, v));
        } else {
            heads.insert(rel.clone());
        }
    }
    for (rel, folder, v) in versioned {
        if top[&folder] == v {
            heads.insert(rel);
        }
    }
    heads
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn frontmatter_split_strips_quotes_and_keeps_body() {
        let (fm, body) = split_frontmatter("---\nstatus: \"ready\"\nname: 'x'\n---\n# T\nbody\n");
        assert_eq!(fm["status"], "ready");
        assert_eq!(fm["name"], "x");
        assert_eq!(body, "# T\nbody\n");
        let (fm, body) = split_frontmatter("no fm\n");
        assert!(fm.is_empty());
        assert_eq!(body, "no fm\n");
    }

    #[test]
    fn headings_make_breadcrumbs_and_fences_hide_headings() {
        let body = "intro\n# A\na text\n## B\n```\n# not a heading\n```\n### C\nc text\n## D\nd text\n";
        let ch = chunk_markdown(body);
        assert_eq!(ch[0], (s(&[]), "intro".to_string()));
        assert_eq!(ch[1], (s(&["A"]), "a text".to_string()));
        assert_eq!(ch[2], (s(&["A", "B"]), "```\n# not a heading\n```".to_string()));
        assert_eq!(ch[3], (s(&["A", "B", "C"]), "c text".to_string()));
        assert_eq!(ch[4], (s(&["A", "D"]), "d text".to_string()));
        assert_eq!(breadcrumb("workspace/p/notes-v2.md", &ch[3].0), "notes-v2 › A › B › C");
    }

    #[test]
    fn seven_hashes_and_tilde_fences() {
        let ch = chunk_markdown("####### seven\n~~~\n# hidden\n```\nstill fenced\n~~~\n# Shown\nx\n");
        assert_eq!(ch.len(), 2);
        assert_eq!(ch[0].0, s(&[]));
        assert!(ch[0].1.starts_with("####### seven"));
        assert_eq!(ch[1], (s(&["Shown"]), "x".to_string()));
    }

    #[test]
    fn a_headings_only_file_is_one_raw_chunk_and_a_blank_file_none() {
        assert_eq!(chunk_markdown("# Only\n\n## Headings\n"), vec![(s(&[]), "# Only\n\n## Headings".to_string())]);
        assert!(chunk_markdown("  \n\n").is_empty());
    }

    #[test]
    fn long_sections_split_into_parts() {
        let para = "w".repeat(5000);
        let body = format!("# Big\n{para}\n\n{para}\n\n{para}\n");
        let ch = chunk_markdown(&body);
        assert_eq!(ch.len(), 3);
        assert_eq!(ch[0].0, s(&["Big", "(part 1)"]));
        assert_eq!(ch[2].0, s(&["Big", "(part 3)"]));
        let ok = format!("# Fits\n{}\n\n{}\n", "a".repeat(4000), "b".repeat(4000));
        assert_eq!(chunk_markdown(&ok)[0].0, s(&["Fits"]));
    }

    #[test]
    fn heads_are_the_folder_max_version() {
        let rels = s(&["a/brief-v1.md", "a/brief-v2.md", "a/notes-v2.txt", "a/plain.md", "b/x-v1.md"]);
        let h = file_heads(&rels);
        assert!(!h.contains("a/brief-v1.md"));
        assert!(h.contains("a/brief-v2.md") && h.contains("a/notes-v2.txt"));
        assert!(h.contains("a/plain.md") && h.contains("b/x-v1.md"));
        assert_eq!(version_of("a/X-V12.MD"), Some(12));
        assert_eq!(version_of("a/x-v1.md.bak"), None);
        assert_eq!(version_of("a/x-v1"), None);
    }
}
