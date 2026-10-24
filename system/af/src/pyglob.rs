//! Python's `glob.glob` (3.12 algorithm) and `fnmatch`, so result strings and the
//! hidden-file and case rules match what af.py sorts and prints.

use crate::pyos;
use regex::Regex;

fn has_magic(s: &str) -> bool {
    s.contains(['*', '?', '['])
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn is_recursive(p: &str) -> bool {
    p == "**"
}

/// `fnmatch.translate` (without the 3.12 range edge cases), anchored, DOTALL.
pub fn translate(pat: &str) -> String {
    let p: Vec<char> = pat.chars().collect();
    let n = p.len();
    let mut res = String::from("(?s)^");
    let mut i = 0;
    let mut last_star = false;
    while i < n {
        let c = p[i];
        i += 1;
        if c == '*' {
            if !last_star {
                res.push_str(".*");
            }
            last_star = true;
            continue;
        }
        last_star = false;
        if c == '?' {
            res.push('.');
        } else if c == '[' {
            let mut j = i;
            if j < n && p[j] == '!' {
                j += 1;
            }
            if j < n && p[j] == ']' {
                j += 1;
            }
            while j < n && p[j] != ']' {
                j += 1;
            }
            if j >= n {
                res.push_str("\\[");
            } else {
                let stuff: Vec<char> = p[i..j].to_vec();
                i = j + 1;
                let (neg, body) = if stuff.first() == Some(&'!') { (true, &stuff[1..]) } else { (false, &stuff[..]) };
                res.push('[');
                if neg {
                    res.push('^');
                }
                let mut k = 0;
                while k < body.len() {
                    if k + 2 < body.len() && body[k + 1] == '-' {
                        res.push_str(&regex::escape(&body[k].to_string()));
                        res.push('-');
                        res.push_str(&regex::escape(&body[k + 2].to_string()));
                        k += 3;
                    } else {
                        res.push_str(&regex::escape(&body[k].to_string()));
                        k += 1;
                    }
                }
                res.push(']');
            }
        } else {
            res.push_str(&regex::escape(&c.to_string()));
        }
    }
    res.push('$');
    res
}

/// `fnmatch.filter(names, pattern)` with the platform's normcase.
pub fn filter(names: &[String], pattern: &str) -> Vec<String> {
    let re = match Regex::new(&translate(&pyos::normcase(pattern))) {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    names.iter().filter(|n| re.is_match(&pyos::normcase(n))).cloned().collect()
}

fn gjoin(dirname: &str, basename: &str) -> String {
    if dirname.is_empty() || basename.is_empty() {
        return if dirname.is_empty() { basename.to_string() } else { dirname.to_string() };
    }
    pyos::join(dirname, basename)
}

fn iterdir(dirname: &str, dironly: bool) -> Vec<String> {
    let arg = if dirname.is_empty() { "." } else { dirname };
    let rd = match std::fs::read_dir(arg) {
        Ok(rd) => rd,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        if !dironly || crate::pyos::entry_is_dir(&e) {
            out.push(e.file_name().to_string_lossy().into_owned());
        }
    }
    out
}

fn glob1(dirname: &str, pattern: &str, dironly: bool) -> Vec<String> {
    let mut names = iterdir(dirname, dironly);
    if !is_hidden(pattern) {
        names.retain(|x| !is_hidden(x));
    }
    filter(&names, pattern)
}

fn glob0(dirname: &str, basename: &str) -> Vec<String> {
    if !basename.is_empty() {
        if pyos::lexists(&gjoin(dirname, basename)) {
            return vec![basename.to_string()];
        }
    } else if pyos::isdir(dirname) {
        return vec![basename.to_string()];
    }
    Vec::new()
}

fn rlistdir(dirname: &str, dironly: bool, out: &mut Vec<String>) {
    for x in iterdir(dirname, dironly) {
        if is_hidden(&x) {
            continue;
        }
        out.push(x.clone());
        let path = if dirname.is_empty() { x.clone() } else { gjoin(dirname, &x) };
        let mut sub = Vec::new();
        rlistdir(&path, dironly, &mut sub);
        for y in sub {
            out.push(gjoin(&x, &y));
        }
    }
}

fn glob2(dirname: &str, dironly: bool) -> Vec<String> {
    let mut out = Vec::new();
    if dirname.is_empty() || pyos::isdir(dirname) {
        out.push(String::new());
    }
    rlistdir(dirname, dironly, &mut out);
    out
}

fn iglob(pathname: &str, recursive: bool, dironly: bool) -> Vec<String> {
    let (dirname, basename) = pyos::split(pathname);
    if !has_magic(pathname) {
        if !basename.is_empty() {
            if pyos::lexists(pathname) {
                return vec![pathname.to_string()];
            }
        } else if pyos::isdir(&dirname) {
            return vec![pathname.to_string()];
        }
        return Vec::new();
    }
    if dirname.is_empty() {
        return if recursive && is_recursive(&basename) { glob2("", dironly) } else { glob1("", &basename, dironly) };
    }
    let dirs = if dirname != pathname && has_magic(&dirname) {
        iglob(&dirname, recursive, true)
    } else {
        vec![dirname.clone()]
    };
    let mut out = Vec::new();
    for d in dirs {
        let names = if has_magic(&basename) {
            if recursive && is_recursive(&basename) {
                glob2(&d, dironly)
            } else {
                glob1(&d, &basename, dironly)
            }
        } else {
            glob0(&d, &basename)
        };
        for name in names {
            out.push(pyos::join(&d, &name));
        }
    }
    out
}

/// `glob.glob(pathname, recursive=recursive)`
pub fn glob(pathname: &str, recursive: bool) -> Vec<String> {
    let mut it = iglob(pathname, recursive, false);
    if pathname.is_empty() || (recursive && is_recursive(&pathname.chars().take(2).collect::<String>())) {
        if let Some(first) = it.first() {
            if first.is_empty() {
                it.remove(0);
            }
        }
    }
    it
}

/// `sorted(glob.glob(...))`
pub fn sorted_glob(pathname: &str, recursive: bool) -> Vec<String> {
    let mut v = glob(pathname, recursive);
    v.sort();
    v
}
