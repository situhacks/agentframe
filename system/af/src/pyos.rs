//! Python `os.path` (ntpath on Windows, posixpath elsewhere) and the few `os` calls af.py
//! makes, as string operations.
//!
//! af.py builds paths by string joins and prints some of them (`{cdir} already exists`,
//! relpaths in doctor lines), and it compares normalized strings in places
//! (`abspath(a) == abspath(b)`). Reproducing the string semantics, mixed separators
//! included, keeps those outputs and comparisons byte-identical.

use crate::error::{crash, CrashKind, R};
use std::path::Path;

// ------------------------------------------------------------------ char slicing

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn cs(v: &[char], a: usize, b: usize) -> String {
    let b = b.min(v.len());
    if a >= b {
        return String::new();
    }
    v[a..b].iter().collect()
}

// ------------------------------------------------------------------ separators

#[cfg(windows)]
pub const SEP: char = '\\';
#[cfg(not(windows))]
pub const SEP: char = '/';

#[cfg(windows)]
pub fn is_sep(c: char) -> bool {
    c == '\\' || c == '/'
}
#[cfg(not(windows))]
pub fn is_sep(c: char) -> bool {
    c == '/'
}

// ------------------------------------------------------------------ ntpath

#[cfg(windows)]
pub fn splitroot(p: &str) -> (String, String, String) {
    let pc = chars(p);
    let normp: Vec<char> = pc.iter().map(|&c| if c == '/' { '\\' } else { c }).collect();
    let n = normp.len();
    let at = |i: usize| if i < n { Some(normp[i]) } else { None };
    if at(0) == Some('\\') {
        if at(1) == Some('\\') {
            let upper8: String = normp.iter().take(8).collect::<String>().to_uppercase();
            let start = if upper8 == "\\\\?\\UNC\\" { 8 } else { 2 };
            let find = |from: usize| (from..n).find(|&i| normp[i] == '\\');
            let index = match find(start) {
                Some(i) => i,
                None => return (p.to_string(), String::new(), String::new()),
            };
            let index2 = match find(index + 1) {
                Some(i) => i,
                None => return (p.to_string(), String::new(), String::new()),
            };
            return (cs(&pc, 0, index2), cs(&pc, index2, index2 + 1), cs(&pc, index2 + 1, pc.len()));
        }
        return (String::new(), cs(&pc, 0, 1), cs(&pc, 1, pc.len()));
    }
    if at(1) == Some(':') {
        if at(2) == Some('\\') {
            return (cs(&pc, 0, 2), cs(&pc, 2, 3), cs(&pc, 3, pc.len()));
        }
        return (cs(&pc, 0, 2), String::new(), cs(&pc, 2, pc.len()));
    }
    (String::new(), String::new(), p.to_string())
}

#[cfg(not(windows))]
pub fn splitroot(p: &str) -> (String, String, String) {
    if !p.starts_with('/') {
        (String::new(), String::new(), p.to_string())
    } else if !p.starts_with("//") || p.starts_with("///") {
        (String::new(), "/".to_string(), p.trim_start_matches('/').to_string())
    } else {
        (String::new(), "//".to_string(), p[2..].to_string())
    }
}

#[cfg(windows)]
pub fn isabs(s: &str) -> bool {
    let head: String = s.chars().take(3).map(|c| if c == '/' { '\\' } else { c }).collect();
    head.starts_with('\\') || head.chars().skip(1).collect::<String>().starts_with(":\\")
}
#[cfg(not(windows))]
pub fn isabs(s: &str) -> bool {
    s.starts_with('/')
}

#[cfg(windows)]
pub fn normcase(s: &str) -> String {
    s.replace('/', "\\").to_lowercase()
}
#[cfg(not(windows))]
pub fn normcase(s: &str) -> String {
    s.to_string()
}

/// `os.path.join(a, b)`
#[cfg(windows)]
pub fn join(a: &str, b: &str) -> String {
    let (mut rd, mut rr, mut rp) = splitroot(a);
    let (pd, pr, pp) = splitroot(b);
    if !pr.is_empty() {
        if !pd.is_empty() || rd.is_empty() {
            rd = pd;
        }
        rr = pr;
        rp = pp;
    } else {
        let mut relative = true;
        if !pd.is_empty() && pd != rd {
            if pd.to_lowercase() != rd.to_lowercase() {
                rd = pd;
                rr = pr;
                rp = pp.clone();
                relative = false;
            } else {
                rd = pd;
            }
        }
        if relative {
            if !rp.is_empty() && !rp.ends_with(is_sep) {
                rp.push('\\');
            }
            rp.push_str(&pp);
        }
    }
    if !rp.is_empty() && rr.is_empty() && !rd.is_empty() {
        let last = rd.chars().last().unwrap();
        if last != ':' && !is_sep(last) {
            return format!("{rd}\\{rp}");
        }
    }
    format!("{rd}{rr}{rp}")
}

#[cfg(not(windows))]
pub fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        return b.to_string();
    }
    if a.is_empty() || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

pub fn join_all(parts: &[&str]) -> String {
    let mut it = parts.iter();
    let mut acc = it.next().map(|s| s.to_string()).unwrap_or_default();
    for p in it {
        acc = join(&acc, p);
    }
    acc
}

/// `os.path.split(p)`
pub fn split(p: &str) -> (String, String) {
    let (d, r, rest) = splitroot(p);
    let pc = chars(&rest);
    let mut i = pc.len();
    while i > 0 && !is_sep(pc[i - 1]) {
        i -= 1;
    }
    let head = cs(&pc, 0, i);
    let tail = cs(&pc, i, pc.len());
    let head = head.trim_end_matches(is_sep).to_string();
    (format!("{d}{r}{head}"), tail)
}

pub fn dirname(p: &str) -> String {
    split(p).0
}

pub fn basename(p: &str) -> String {
    split(p).1
}

/// `os.path.normpath(p)`
pub fn normpath(path: &str) -> String {
    #[cfg(windows)]
    let path = path.replace('/', "\\");
    #[cfg(not(windows))]
    let path = path.to_string();
    let (drive, root, rest) = splitroot(&path);
    let prefix = format!("{drive}{root}");
    let mut comps: Vec<String> = rest.split(SEP).map(|s| s.to_string()).collect();
    let mut i = 0usize;
    while i < comps.len() {
        if comps[i].is_empty() || comps[i] == "." {
            comps.remove(i);
        } else if comps[i] == ".." {
            if i > 0 && comps[i - 1] != ".." {
                comps.drain(i - 1..=i);
                i -= 1;
            } else if i == 0 && !root.is_empty() {
                comps.remove(i);
            } else {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    if prefix.is_empty() && comps.is_empty() {
        comps.push(".".to_string());
    }
    format!("{prefix}{}", comps.join(&SEP.to_string()))
}

/// `os.path.abspath(p)`: GetFullPathNameW of the normalized path on Windows.
pub fn abspath(p: &str) -> String {
    let n = normpath(p);
    match std::path::absolute(Path::new(&n)) {
        Ok(pb) => pb.to_string_lossy().into_owned(),
        Err(_) => {
            let cwd = std::env::current_dir().map(|c| c.to_string_lossy().into_owned()).unwrap_or_default();
            normpath(&join(&cwd, &n))
        }
    }
}

/// `os.path.relpath(path, start)`
pub fn relpath(path: &str, start: &str) -> R<String> {
    if path.is_empty() {
        return Err(crash(CrashKind::Other, "ValueError: no path specified"));
    }
    let start_abs = abspath(&normpath(start));
    let path_abs = abspath(&normpath(path));
    let (sd, _, srest) = splitroot(&start_abs);
    let (pd, _, prest) = splitroot(&path_abs);
    if normcase(&sd) != normcase(&pd) {
        return Err(crash(
            CrashKind::Other,
            format!("ValueError: path is on mount {}, start on mount {}", py_repr_str(&pd), py_repr_str(&sd)),
        ));
    }
    let sl: Vec<&str> = srest.split(SEP).filter(|x| !x.is_empty()).collect();
    let pl: Vec<&str> = prest.split(SEP).filter(|x| !x.is_empty()).collect();
    let mut i = 0;
    for (a, b) in sl.iter().zip(pl.iter()) {
        if normcase(a) != normcase(b) {
            break;
        }
        i += 1;
    }
    let mut rel: Vec<&str> = vec![".."; sl.len() - i];
    rel.extend(&pl[i..]);
    if rel.is_empty() {
        return Ok(".".to_string());
    }
    Ok(join_all(&rel))
}

/// `os.path.commonpath(paths)`
pub fn commonpath(paths: &[String]) -> R<String> {
    if paths.is_empty() {
        return Err(crash(CrashKind::Other, "ValueError: commonpath() arg is an empty sequence"));
    }
    let alt = |p: &str| -> String {
        #[cfg(windows)]
        {
            p.replace('/', "\\")
        }
        #[cfg(not(windows))]
        {
            p.to_string()
        }
    };
    let lower = |p: &str| -> String {
        if cfg!(windows) {
            p.to_lowercase()
        } else {
            p.to_string()
        }
    };
    let splits: Vec<(String, String, String)> = paths.iter().map(|p| splitroot(&lower(&alt(p)))).collect();
    let roots: std::collections::BTreeSet<&String> = splits.iter().map(|s| &s.1).collect();
    if roots.len() != 1 {
        return Err(crash(CrashKind::Other, "ValueError: Can't mix absolute and relative paths"));
    }
    let drives: std::collections::BTreeSet<&String> = splits.iter().map(|s| &s.0).collect();
    if drives.len() != 1 {
        return Err(crash(CrashKind::Other, "ValueError: Paths don't have the same drive"));
    }
    let (drive, root, path) = splitroot(&alt(&paths[0]));
    let common: Vec<&str> = path.split(SEP).filter(|c| !c.is_empty() && *c != ".").collect();
    let split_paths: Vec<Vec<&str>> = splits
        .iter()
        .map(|s| s.2.split(SEP).filter(|c| !c.is_empty() && *c != ".").collect())
        .collect();
    let s1 = split_paths.iter().min().unwrap();
    let s2 = split_paths.iter().max().unwrap();
    let mut n = s1.len();
    for (i, c) in s1.iter().enumerate() {
        if *c != s2[i] {
            n = i;
            break;
        }
    }
    let common: Vec<&str> = common.into_iter().take(n).collect();
    Ok(format!("{drive}{root}{}", common.join(&SEP.to_string())))
}

/// `os.path.splitext(p)`
pub fn splitext(p: &str) -> (String, String) {
    let pc = chars(p);
    let sep_index = pc.iter().rposition(|&c| is_sep(c)).map(|i| i as isize).unwrap_or(-1);
    let dot_index = pc.iter().rposition(|&c| c == '.').map(|i| i as isize).unwrap_or(-1);
    if dot_index > sep_index {
        let mut fi = sep_index + 1;
        while fi < dot_index {
            if pc[fi as usize] != '.' {
                let d = dot_index as usize;
                return (cs(&pc, 0, d), cs(&pc, d, pc.len()));
            }
            fi += 1;
        }
    }
    (p.to_string(), String::new())
}

/// `str.isprintable()` for one char, without Unicode tables: the control, format,
/// separator, private-use and noncharacter code points Python escapes in `repr`.
/// Unassigned code points (also escaped by Python) are not covered.
fn py_printable(c: char) -> bool {
    let u = c as u32;
    !matches!(u,
        0x00..=0x1f | 0x7f..=0xa0 | 0xad | 0x600..=0x605 | 0x61c | 0x6dd | 0x70f | 0x890..=0x891
        | 0x8e2 | 0x1680 | 0x180e | 0x2000..=0x200f | 0x2028..=0x202f | 0x205f..=0x2064
        | 0x2066..=0x206f | 0x3000 | 0xd800..=0xf8ff | 0xfdd0..=0xfdef | 0xfeff | 0xfff9..=0xfffb
        | 0xfffe..=0xffff | 0x110bd | 0x110cd | 0x13430..=0x1343f | 0x1bca0..=0x1bca3
        | 0x1d173..=0x1d17a | 0xe0001 | 0xe0020..=0xe007f | 0xf0000..=0x10ffff)
        && (u & 0xfffe) != 0xfffe
}

/// `repr(str)`: single quotes unless the text holds one and no double; non-printable
/// characters as `\xhh`, `\uhhhh` or `\Uhhhhhhhh`.
pub fn py_repr_str(s: &str) -> String {
    let quote = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut out = String::new();
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if !py_printable(c) => {
                let u = c as u32;
                if u <= 0xff {
                    out.push_str(&format!("\\x{u:02x}"));
                } else if u <= 0xffff {
                    out.push_str(&format!("\\u{u:04x}"));
                } else {
                    out.push_str(&format!("\\U{u:08x}"));
                }
            }
            c => out.push(c),
        }
    }
    out.push(quote);
    out
}

// ------------------------------------------------------------------ filesystem

pub fn isfile(p: &str) -> bool {
    !p.is_empty() && Path::new(p).is_file()
}

pub fn isdir(p: &str) -> bool {
    !p.is_empty() && Path::new(p).is_dir()
}

pub fn exists(p: &str) -> bool {
    !p.is_empty() && Path::new(p).exists()
}

pub fn lexists(p: &str) -> bool {
    !p.is_empty() && std::fs::symlink_metadata(p).is_ok()
}

/// `os.listdir(p)` as names (unsorted, scandir order).
pub fn listdir(p: &str) -> R<Vec<String>> {
    let rd = std::fs::read_dir(if p.is_empty() { "." } else { p }).map_err(|e| os_error(&e, p))?;
    let mut out = Vec::new();
    for e in rd.flatten() {
        out.push(e.file_name().to_string_lossy().into_owned());
    }
    Ok(out)
}

pub fn sorted_listdir(p: &str) -> R<Vec<String>> {
    let mut v = listdir(p)?;
    v.sort();
    Ok(v)
}

/// `os.path.getmtime(p)` as float seconds.
pub fn getmtime(p: &str) -> R<f64> {
    let md = std::fs::metadata(p).map_err(|e| os_error(&e, p))?;
    let t = md.modified().map_err(|e| os_error(&e, p))?;
    let d = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    Ok(d.as_secs() as f64 + d.subsec_nanos() as f64 / 1e9)
}

pub fn makedirs(p: &str) -> R<()> {
    std::fs::create_dir_all(p).map_err(|e| os_error(&e, p))
}

/// `os.makedirs(p)` without exist_ok: refuses an existing leaf.
pub fn makedirs_new(p: &str) -> R<()> {
    if exists(p) {
        return Err(crash(
            CrashKind::Os,
            format!("FileExistsError: [WinError 183] Cannot create a file when that file already exists: {}", py_repr_str(p)),
        ));
    }
    makedirs(p)
}

pub fn os_error(e: &std::io::Error, path: &str) -> AfErrorAlias {
    let (cls, errno, text) = match e.kind() {
        std::io::ErrorKind::NotFound => ("FileNotFoundError", 2, "No such file or directory"),
        std::io::ErrorKind::PermissionDenied => ("PermissionError", 13, "Permission denied"),
        std::io::ErrorKind::AlreadyExists => ("FileExistsError", 17, "File exists"),
        _ => ("OSError", 22, "Invalid argument"),
    };
    crash(CrashKind::Os, format!("{cls}: [Errno {errno}] {text}: {}", py_repr_str(path)))
}

pub type AfErrorAlias = crate::error::AfError;

/// `open(path, encoding="utf-8-sig").read()` in universal-newline text mode.
pub fn read(path: &str) -> R<String> {
    let bytes = std::fs::read(path).map_err(|e| os_error(&e, path))?;
    decode_text(&bytes)
}

pub fn decode_text(bytes: &[u8]) -> R<String> {
    let body = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) { &bytes[3..] } else { bytes };
    let text = match std::str::from_utf8(body) {
        Ok(s) => s,
        Err(e) => return Err(crash(CrashKind::Decode, utf8_error_message(body, &e))),
    };
    Ok(universal_newlines(text))
}

pub fn universal_newlines(text: &str) -> String {
    if !text.contains('\r') {
        return text.to_string();
    }
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn utf8_error_message(data: &[u8], e: &std::str::Utf8Error) -> String {
    let pos = e.valid_up_to();
    let byte = data[pos];
    match e.error_len() {
        None => {
            let end = data.len() - 1;
            if end > pos {
                format!(
                    "UnicodeDecodeError: 'utf-8' codec can't decode bytes in position {pos}-{end}: unexpected end of data"
                )
            } else {
                format!(
                    "UnicodeDecodeError: 'utf-8' codec can't decode byte 0x{byte:02x} in position {pos}: unexpected end of data"
                )
            }
        }
        Some(n) => {
            let reason = if (0xC2..=0xF4).contains(&byte) { "invalid continuation byte" } else { "invalid start byte" };
            if n > 1 {
                let end = pos + n - 1;
                format!("UnicodeDecodeError: 'utf-8' codec can't decode bytes in position {pos}-{end}: {reason}")
            } else {
                format!("UnicodeDecodeError: 'utf-8' codec can't decode byte 0x{byte:02x} in position {pos}: {reason}")
            }
        }
    }
}

/// `open(path, "w", encoding="utf-8", newline="\n").write(text)`
pub fn write(path: &str, text: &str) -> R<()> {
    std::fs::write(path, text.as_bytes()).map_err(|e| os_error(&e, path))
}

/// `os.walk(top)` (top-down, followlinks=False, errors ignored). The visitor gets
/// (dirpath, dirnames, filenames) and may prune `dirnames`.
/// `DirEntry.is_dir()`: the type read with the directory listing, a stat only for links.
pub fn entry_is_dir(e: &std::fs::DirEntry) -> bool {
    match e.file_type() {
        Ok(ft) if ft.is_symlink() => e.path().is_dir(),
        Ok(ft) => ft.is_dir(),
        Err(_) => false,
    }
}

pub fn walk<F>(top: &str, visit: &mut F)
where
    F: FnMut(&str, &mut Vec<String>, &[String]),
{
    let rd = match std::fs::read_dir(top) {
        Ok(rd) => rd,
        Err(_) => return,
    };
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let mut symlinked = std::collections::HashSet::new();
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if entry_is_dir(&e) {
            if e.file_type().map(|t| t.is_symlink()).unwrap_or(false) {
                symlinked.insert(name.clone());
            }
            dirs.push(name);
        } else {
            files.push(name);
        }
    }
    visit(top, &mut dirs, &files);
    for d in dirs {
        if symlinked.contains(&d) {
            continue;
        }
        walk(&join(top, &d), visit);
    }
}

// ------------------------------------------------------------------ str helpers

/// `str.isspace()` for one char.
pub fn py_isspace(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `str.strip()`
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(py_isspace)
}

/// `str.split()` with no separator.
pub fn py_split_ws(s: &str) -> Vec<&str> {
    s.split(py_isspace).filter(|x| !x.is_empty()).collect()
}

/// `str.splitlines()`
pub fn py_splitlines(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        match c {
            '\n' | '\u{0b}' | '\u{0c}' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{85}' | '\u{2028}' | '\u{2029}' => {
                out.push(std::mem::take(&mut cur));
            }
            '\r' => {
                if it.peek() == Some(&'\n') {
                    it.next();
                }
                out.push(std::mem::take(&mut cur));
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `str.title()`
pub fn py_title(s: &str) -> String {
    let mut out = String::new();
    let mut prev_cased = false;
    for c in s.chars() {
        let cased = c.is_lowercase() || c.is_uppercase();
        if cased {
            if prev_cased {
                out.extend(c.to_lowercase());
            } else {
                out.extend(c.to_uppercase());
            }
        } else {
            out.push(c);
        }
        prev_cased = cased;
    }
    out
}

/// `s[:n]` by code points.
pub fn py_prefix(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}
