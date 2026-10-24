//! The slice of pathlib the hooks lean on: `str(Path(...))`, `/`, `.name`, `.resolve()`,
//! and `relative_to` as a containment test (case-insensitive on Windows).

use crate::pyos;
use std::path::Path;

#[cfg(windows)]
const SEP: &str = "\\";
#[cfg(not(windows))]
const SEP: &str = "/";

/// `str(Path(p))`: separators unified, empty and `.` parts dropped, `.` for nothing.
pub fn pstr(p: &str) -> String {
    #[cfg(windows)]
    let p = p.replace('/', "\\");
    #[cfg(not(windows))]
    let p = p.to_string();
    let (drive, root, tail) = pyos::splitroot(&p);
    let parts: Vec<&str> = tail.split(SEP).filter(|x| !x.is_empty() && *x != ".").collect();
    let s = format!("{drive}{root}{}", parts.join(SEP));
    if s.is_empty() {
        ".".into()
    } else {
        s
    }
}

/// `str(Path(a) / b)`
pub fn pjoin(a: &str, b: &str) -> String {
    pstr(&pyos::join(&pstr(a), &pstr(b)))
}

/// `Path(p).name`
pub fn name(p: &str) -> String {
    let s = pstr(p);
    let (drive, root, tail) = pyos::splitroot(&s);
    let _ = (drive, root);
    if tail.is_empty() {
        return String::new();
    }
    tail.rsplit(SEP).next().unwrap_or("").to_string()
}

/// `Path(p).is_absolute()`
pub fn is_absolute(p: &str) -> bool {
    let (drive, root, _) = pyos::splitroot(&pstr(p));
    if cfg!(windows) {
        !drive.is_empty() && !root.is_empty()
    } else {
        !root.is_empty()
    }
}

fn strip_verbatim(s: String) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s
    }
}

/// `str(Path(p).resolve())`: absolute and normalized, the longest existing prefix
/// resolved through the filesystem, the rest appended as written.
pub fn resolve(p: &str) -> String {
    let abs = pyos::abspath(&pstr(p));
    let mut head = abs.clone();
    let mut tail: Vec<String> = Vec::new();
    loop {
        if let Ok(c) = std::fs::canonicalize(Path::new(&head)) {
            let mut out = strip_verbatim(c.to_string_lossy().into_owned());
            for t in tail.iter().rev() {
                out = pyos::join(&out, t);
            }
            return out;
        }
        let (parent, base) = pyos::split(&head);
        if base.is_empty() || parent == head {
            return abs;
        }
        tail.push(base);
        head = parent;
    }
}

fn norm_case(s: &str) -> String {
    if cfg!(windows) {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// `path.resolve().relative_to(ancestor.resolve())` succeeds.
pub fn is_under(path: &str, ancestor: &str) -> bool {
    let a = norm_case(&resolve(path));
    let b = norm_case(&resolve(ancestor));
    if a == b {
        return true;
    }
    if b.ends_with(SEP) {
        a.starts_with(&b)
    } else {
        a.starts_with(&format!("{b}{SEP}"))
    }
}
