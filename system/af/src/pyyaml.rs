//! `yaml.safe_load` as PyYAML 6 (YAML 1.1) runs it, for the media-shelf check.
//!
//! yaml-rust2 supplies the syntax: scanner and parser events. Everything PyYAML decides
//! after that is ported here: the reader's printable-character check, the implicit
//! resolvers (yes/no/on/off booleans, octal, binary and sexagesimal ints, dates and
//! timestamps, `<<` merge keys, `=`), the SafeConstructor with its explicit tags,
//! construction order and error texts, and the Python value model (str, repr,
//! truthiness, type names) the caller formats notes with.
//!
//! Known gaps: scanner, parser and composer errors keep
//! yaml-rust2's wording (and its YAML 1.2 view of NEL/LS/PS line breaks and duplicate
//! anchors); constructor-error marks are rebuilt from yaml-rust2 event positions and can
//! drift on unusual layouts.

use crate::pyos::{py_isspace, py_repr_str};
use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use regex::Regex;
use std::collections::HashMap;
use std::fmt;
use std::sync::OnceLock;
use yaml_rust2::parser::{Event, Parser};
use yaml_rust2::scanner::{Scanner, TScalarStyle, Token, TokenType};

const T: &str = "tag:yaml.org,2002:";

// ------------------------------------------------------------------ Python values

/// A Python int: arbitrary precision, base-1e9 limbs, little-endian, zero = no limbs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PyInt {
    neg: bool,
    mag: Vec<u32>,
}

const LIMB: u64 = 1_000_000_000;

impl PyInt {
    fn zero() -> PyInt {
        PyInt { neg: false, mag: Vec::new() }
    }

    pub fn is_zero(&self) -> bool {
        self.mag.is_empty()
    }

    fn mul_add(&mut self, m: u64, a: u64) {
        let mut carry = a;
        for limb in self.mag.iter_mut() {
            let v = *limb as u64 * m + carry;
            *limb = (v % LIMB) as u32;
            carry = v / LIMB;
        }
        while carry > 0 {
            self.mag.push((carry % LIMB) as u32);
            carry /= LIMB;
        }
    }

    /// Magnitude addition; both sides non-negative.
    fn add_mag(&mut self, o: &PyInt) {
        let mut carry = 0u64;
        let n = self.mag.len().max(o.mag.len());
        for i in 0..n {
            if i == self.mag.len() {
                self.mag.push(0);
            }
            let v = self.mag[i] as u64 + o.mag.get(i).copied().unwrap_or(0) as u64 + carry;
            self.mag[i] = (v % LIMB) as u32;
            carry = v / LIMB;
        }
        if carry > 0 {
            self.mag.push(carry as u32);
        }
    }

    fn negated(mut self) -> PyInt {
        if !self.is_zero() {
            self.neg = !self.neg;
        }
        self
    }

    fn from_i128(v: i128) -> PyInt {
        let mut out = PyInt::zero();
        let mut m = v.unsigned_abs();
        while m > 0 {
            out.mag.push((m % LIMB as u128) as u32);
            m /= LIMB as u128;
        }
        if v < 0 {
            out.neg = true;
        }
        out
    }

    fn to_i128(&self) -> Option<i128> {
        let mut v: i128 = 0;
        for l in self.mag.iter().rev() {
            v = v.checked_mul(LIMB as i128)?.checked_add(*l as i128)?;
        }
        Some(if self.neg { -v } else { v })
    }

    fn to_f64(&self) -> f64 {
        let mut f = 0.0;
        for l in self.mag.iter().rev() {
            f = f * LIMB as f64 + *l as f64;
        }
        if self.neg {
            -f
        } else {
            f
        }
    }
}

impl fmt::Display for PyInt {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let Some(top) = self.mag.last() else {
            return write!(f, "0");
        };
        if self.neg {
            write!(f, "-")?;
        }
        write!(f, "{top}")?;
        for l in self.mag.iter().rev().skip(1) {
            write!(f, "{l:09}")?;
        }
        Ok(())
    }
}

/// What `yaml.safe_load` can return, with the Python behaviour callers format with.
#[derive(Clone, Debug)]
pub enum PyVal {
    None,
    Bool(bool),
    Int(PyInt),
    Float(f64),
    Str(String),
    Bytes(Vec<u8>),
    Date(NaiveDate),
    /// Naive or aware (offset seconds east of UTC; `Some(0)` is `timezone.utc`).
    DateTime(NaiveDateTime, Option<i32>),
    List(Vec<PyVal>),
    Tuple(Vec<PyVal>),
    Dict(Vec<(PyVal, PyVal)>),
    Set(Vec<PyVal>),
    /// A container met again inside itself (an aliased cycle); Python prints `[...]`.
    Recursive(&'static str),
}

impl PyVal {
    pub fn truthy(&self) -> bool {
        match self {
            PyVal::None => false,
            PyVal::Bool(b) => *b,
            PyVal::Int(i) => !i.is_zero(),
            PyVal::Float(f) => *f != 0.0,
            PyVal::Str(s) => !s.is_empty(),
            PyVal::Bytes(b) => !b.is_empty(),
            PyVal::Date(_) | PyVal::DateTime(..) | PyVal::Recursive(_) => true,
            PyVal::List(v) | PyVal::Tuple(v) | PyVal::Set(v) => !v.is_empty(),
            PyVal::Dict(m) => !m.is_empty(),
        }
    }

    /// `Py_TYPE(v)->tp_name`, what most TypeError texts print.
    pub fn type_name(&self) -> &'static str {
        match self {
            PyVal::None => "NoneType",
            PyVal::Bool(_) => "bool",
            PyVal::Int(_) => "int",
            PyVal::Float(_) => "float",
            PyVal::Str(_) => "str",
            PyVal::Bytes(_) => "bytes",
            PyVal::Date(_) => "datetime.date",
            PyVal::DateTime(..) => "datetime.datetime",
            PyVal::List(_) => "list",
            PyVal::Tuple(_) => "tuple",
            PyVal::Dict(_) => "dict",
            PyVal::Set(_) => "set",
            PyVal::Recursive(k) => k,
        }
    }

    /// `type(v).__name__`, what `os.fspath` errors print.
    pub fn short_type_name(&self) -> &'static str {
        match self {
            PyVal::Date(_) => "date",
            PyVal::DateTime(..) => "datetime",
            other => other.type_name(),
        }
    }

    pub fn is_hashable(&self) -> bool {
        !matches!(self, PyVal::List(_) | PyVal::Dict(_) | PyVal::Set(_) | PyVal::Recursive(_))
    }

    pub fn repr(&self) -> String {
        match self {
            PyVal::None => "None".to_string(),
            PyVal::Bool(b) => if *b { "True" } else { "False" }.to_string(),
            PyVal::Int(i) => i.to_string(),
            PyVal::Float(f) => float_repr(*f),
            PyVal::Str(s) => py_repr_str(s),
            PyVal::Bytes(b) => bytes_repr(b),
            PyVal::Date(d) => format!("datetime.date({}, {}, {})", d.year(), d.month(), d.day()),
            PyVal::DateTime(dt, tz) => {
                let mut s = format!(
                    "datetime.datetime({}, {}, {}, {}, {}",
                    dt.year(),
                    dt.month(),
                    dt.day(),
                    dt.hour(),
                    dt.minute()
                );
                let us = dt.nanosecond() / 1000;
                if us != 0 {
                    s.push_str(&format!(", {}, {}", dt.second(), us));
                } else if dt.second() != 0 {
                    s.push_str(&format!(", {}", dt.second()));
                }
                if let Some(off) = tz {
                    s.push_str(", tzinfo=");
                    s.push_str(&timezone_repr(*off));
                }
                s.push(')');
                s
            }
            PyVal::List(v) => format!("[{}]", v.iter().map(PyVal::repr).collect::<Vec<_>>().join(", ")),
            PyVal::Tuple(v) if v.len() == 1 => format!("({},)", v[0].repr()),
            PyVal::Tuple(v) => format!("({})", v.iter().map(PyVal::repr).collect::<Vec<_>>().join(", ")),
            PyVal::Dict(m) => format!(
                "{{{}}}",
                m.iter().map(|(k, v)| format!("{}: {}", k.repr(), v.repr())).collect::<Vec<_>>().join(", ")
            ),
            PyVal::Set(v) if v.is_empty() => "set()".to_string(),
            PyVal::Set(v) => format!("{{{}}}", v.iter().map(PyVal::repr).collect::<Vec<_>>().join(", ")),
            PyVal::Recursive("dict") => "{...}".to_string(),
            PyVal::Recursive(_) => "[...]".to_string(),
        }
    }

    /// Python `str(v)`.
    pub fn py_str(&self) -> String {
        match self {
            PyVal::Str(s) => s.clone(),
            PyVal::Date(d) => format!("{:04}-{:02}-{:02}", d.year(), d.month(), d.day()),
            PyVal::DateTime(dt, tz) => {
                let mut s = format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    dt.year(),
                    dt.month(),
                    dt.day(),
                    dt.hour(),
                    dt.minute(),
                    dt.second()
                );
                let us = dt.nanosecond() / 1000;
                if us != 0 {
                    s.push_str(&format!(".{us:06}"));
                }
                if let Some(off) = tz {
                    let sign = if *off < 0 { '-' } else { '+' };
                    let a = off.unsigned_abs();
                    s.push_str(&format!("{sign}{:02}:{:02}", a / 3600, a % 3600 / 60));
                    if a % 60 != 0 {
                        s.push_str(&format!(":{:02}", a % 60));
                    }
                }
                s
            }
            other => other.repr(),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PyVal::Str(s) => Some(s),
            _ => None,
        }
    }

    /// `d.get(key)` on a dict, by a str key.
    pub fn get(&self, key: &str) -> Option<&PyVal> {
        match self {
            PyVal::Dict(m) => m.iter().find(|(k, _)| k.as_str() == Some(key)).map(|(_, v)| v),
            _ => None,
        }
    }

    /// `self == "text"`.
    pub fn eq_str(&self, text: &str) -> bool {
        self.as_str() == Some(text)
    }
}

fn numeric(v: &PyVal) -> Option<f64> {
    match v {
        PyVal::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        PyVal::Int(i) => Some(i.to_f64()),
        PyVal::Float(f) => Some(*f),
        _ => None,
    }
}

/// `a == b` for the hashable values a mapping key can hold. All NaNs compare equal,
/// because PyYAML hands back one shared `nan_value` object and dicts test identity first.
fn py_eq(a: &PyVal, b: &PyVal) -> bool {
    match (a, b) {
        (PyVal::None, PyVal::None) => true,
        (PyVal::Str(x), PyVal::Str(y)) => x == y,
        (PyVal::Bytes(x), PyVal::Bytes(y)) => x == y,
        (PyVal::Date(x), PyVal::Date(y)) => x == y,
        (PyVal::DateTime(x, tx), PyVal::DateTime(y, ty)) => match (tx, ty) {
            (Some(ox), Some(oy)) => *x - Duration::seconds(*ox as i64) == *y - Duration::seconds(*oy as i64),
            (None, None) => x == y,
            _ => false,
        },
        (PyVal::Int(x), PyVal::Int(y)) => x == y,
        _ => match (numeric(a), numeric(b)) {
            (Some(x), Some(y)) => x == y || (x.is_nan() && y.is_nan()),
            _ => false,
        },
    }
}

/// `repr(float)`: shortest round-trip digits, exponent outside 1e-4 <= |x| < 1e16.
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".to_string();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.to_string();
    }
    let e = format!("{:e}", f.abs());
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(|c| *c != '.').collect();
    let n = digits.len() as i32;
    let decpt = exp + 1;
    let body = if decpt > -4 && decpt <= 16 {
        if decpt <= 0 {
            format!("0.{}{}", "0".repeat((-decpt) as usize), digits)
        } else if decpt >= n {
            format!("{}{}.0", digits, "0".repeat((decpt - n) as usize))
        } else {
            format!("{}.{}", &digits[..decpt as usize], &digits[decpt as usize..])
        }
    } else {
        let m = if n == 1 { digits.clone() } else { format!("{}.{}", &digits[..1], &digits[1..]) };
        let x = decpt - 1;
        format!("{m}e{}{:02}", if x < 0 { '-' } else { '+' }, x.abs())
    };
    format!("{}{}", if f < 0.0 { "-" } else { "" }, body)
}

fn bytes_repr(b: &[u8]) -> String {
    let quote = if b.contains(&b'\'') && !b.contains(&b'"') { b'"' } else { b'\'' };
    let mut out = String::from("b");
    out.push(quote as char);
    for &c in b {
        match c {
            b'\\' => out.push_str("\\\\"),
            b'\t' => out.push_str("\\t"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            c if c == quote => {
                out.push('\\');
                out.push(c as char);
            }
            0x20..=0x7e => out.push(c as char),
            c => out.push_str(&format!("\\x{c:02x}")),
        }
    }
    out.push(quote as char);
    out
}

fn timedelta_repr(secs: i64) -> String {
    let days = secs.div_euclid(86400);
    let rest = secs.rem_euclid(86400);
    let mut parts = Vec::new();
    if days != 0 {
        parts.push(format!("days={days}"));
    }
    if rest != 0 {
        parts.push(format!("seconds={rest}"));
    }
    if parts.is_empty() {
        parts.push("0".to_string());
    }
    format!("datetime.timedelta({})", parts.join(", "))
}

fn timezone_repr(off: i32) -> String {
    if off == 0 {
        "datetime.timezone.utc".to_string()
    } else {
        format!("datetime.timezone({})", timedelta_repr(off as i64))
    }
}

// ------------------------------------------------------------------ errors and marks

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Mark {
    line: usize,
    col: usize,
    ptr: usize,
}

/// The text being loaded, as PyYAML's Reader sees it (name, buffer, snippet or not).
struct Src {
    chars: Vec<char>,
    name: String,
    snippet: bool,
}

fn is_break(c: char) -> bool {
    matches!(c, '\0' | '\r' | '\n' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

impl Src {
    fn at(&self, i: usize) -> char {
        self.chars.get(i).copied().unwrap_or('\0')
    }

    /// Reader.forward() line/column bookkeeping up to char offset `ptr`.
    fn mark(&self, ptr: usize) -> Mark {
        let (mut line, mut col) = (0, 0);
        for i in 0..ptr.min(self.chars.len()) {
            let ch = self.chars[i];
            if matches!(ch, '\n' | '\u{85}' | '\u{2028}' | '\u{2029}') || (ch == '\r' && self.at(i + 1) != '\n') {
                line += 1;
                col = 0;
            } else if ch != '\u{feff}' {
                col += 1;
            }
        }
        Mark { line, col, ptr }
    }

    /// `Mark.get_snippet()`.
    fn snippet_at(&self, ptr: usize) -> String {
        let half = 75.0 / 2.0 - 1.0;
        let len = self.chars.len() + 1;
        let (mut head, mut start) = ("", ptr);
        while start > 0 && !is_break(self.at(start - 1)) {
            start -= 1;
            if (ptr - start) as f64 > half {
                head = " ... ";
                start += 5;
                break;
            }
        }
        let (mut tail, mut end) = ("", ptr);
        while end < len && !is_break(self.at(end)) {
            end += 1;
            if (end - ptr) as f64 > half {
                tail = " ... ";
                end -= 5;
                break;
            }
        }
        let snippet: String = (start..end).map(|i| self.at(i)).collect();
        format!("    {head}{snippet}{tail}\n{}^", " ".repeat(4 + ptr - start + head.len()))
    }

    /// `str(Mark)`.
    fn where_(&self, m: Mark) -> String {
        let mut s = format!("  in \"{}\", line {}, column {}", self.name, m.line + 1, m.col + 1);
        if self.snippet {
            s.push_str(":\n");
            s.push_str(&self.snippet_at(m.ptr));
        }
        s
    }
}

enum YErr {
    /// A MarkedYAMLError (ComposerError, ConstructorError).
    Marked { context: Option<String>, context_mark: Option<Mark>, problem: String, problem_mark: Mark },
    /// `str(exc)` of a plain exception (ValueError, KeyError, IndexError, AttributeError).
    Plain(String),
}

impl YErr {
    fn render(&self, src: &Src) -> String {
        match self {
            YErr::Plain(s) => s.clone(),
            YErr::Marked { context, context_mark, problem, problem_mark } => {
                let mut lines = Vec::new();
                if let Some(c) = context {
                    lines.push(c.to_string());
                }
                if let Some(cm) = context_mark {
                    if cm.line != problem_mark.line || cm.col != problem_mark.col {
                        lines.push(src.where_(*cm));
                    }
                }
                lines.push(problem.clone());
                lines.push(src.where_(*problem_mark));
                lines.join("\n")
            }
        }
    }
}

fn plain(s: impl Into<String>) -> YErr {
    YErr::Plain(s.into())
}

// ------------------------------------------------------------------ resolver

/// A PyYAML regexp with Python's `$` (end, or before one final newline).
fn rx_py(body: &str) -> Regex {
    Regex::new(&format!(r"^(?:{body})\n?\z")).expect("static regex")
}

struct Resolvers {
    bool_: Regex,
    float: Regex,
    int: Regex,
    merge: Regex,
    null: Regex,
    timestamp: Regex,
    value: Regex,
    ts_parts: Regex,
}

fn resolvers() -> &'static Resolvers {
    static R: OnceLock<Resolvers> = OnceLock::new();
    R.get_or_init(|| Resolvers {
        bool_: rx_py("yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF"),
        float: rx_py(concat!(
            r"[-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+][0-9]+)?",
            r"|\.[0-9][0-9_]*(?:[eE][-+][0-9]+)?",
            r"|[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*",
            r"|[-+]?\.(?:inf|Inf|INF)",
            r"|\.(?:nan|NaN|NAN)"
        )),
        int: rx_py(concat!(
            r"[-+]?0b[0-1_]+",
            r"|[-+]?0[0-7_]+",
            r"|[-+]?(?:0|[1-9][0-9_]*)",
            r"|[-+]?0x[0-9a-fA-F_]+",
            r"|[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+"
        )),
        merge: rx_py("<<"),
        null: rx_py("~|null|Null|NULL|"),
        timestamp: rx_py(concat!(
            r"[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]",
            r"|[0-9][0-9][0-9][0-9]-[0-9][0-9]?-[0-9][0-9]?",
            r"(?:[Tt]|[ \t]+)[0-9][0-9]?",
            r":[0-9][0-9]:[0-9][0-9](?:\.[0-9]*)?",
            r"(?:[ \t]*(?:Z|[-+][0-9][0-9]?(?::[0-9][0-9])?))?"
        )),
        value: rx_py("="),
        ts_parts: rx_py(concat!(
            r"(?P<year>[0-9][0-9][0-9][0-9])",
            r"-(?P<month>[0-9][0-9]?)",
            r"-(?P<day>[0-9][0-9]?)",
            r"(?:(?:[Tt]|[ \t]+)",
            r"(?P<hour>[0-9][0-9]?)",
            r":(?P<minute>[0-9][0-9])",
            r":(?P<second>[0-9][0-9])",
            r"(?:\.(?P<fraction>[0-9]*))?",
            r"(?:[ \t]*(?P<tz>Z|(?P<tz_sign>[-+])(?P<tz_hour>[0-9][0-9]?)",
            r"(?::(?P<tz_minute>[0-9][0-9]))?))?)?"
        )),
    })
}

/// `Resolver.resolve(ScalarNode, value, (True, False))`: first-character tables, then
/// the resolvers in the order PyYAML registers them.
fn resolve(value: &str) -> String {
    let r = resolvers();
    let candidates: &[(&str, &Regex)] = match value.chars().next() {
        None => &[("null", &r.null)],
        Some(c) => match c {
            'y' | 'Y' | 't' | 'T' | 'f' | 'F' | 'o' | 'O' => &[("bool", &r.bool_)],
            'n' | 'N' => &[("bool", &r.bool_), ("null", &r.null)],
            '-' | '+' => &[("float", &r.float), ("int", &r.int)],
            '.' => &[("float", &r.float)],
            '0'..='9' => &[("float", &r.float), ("int", &r.int), ("timestamp", &r.timestamp)],
            '<' => &[("merge", &r.merge)],
            '~' => &[("null", &r.null)],
            '=' => &[("value", &r.value)],
            _ => &[],
        },
    };
    for (tag, re) in candidates {
        if re.is_match(value) {
            return format!("{T}{tag}");
        }
    }
    format!("{T}str")
}

// ------------------------------------------------------------------ scalar constructors

/// `int(text, base)`; Err is the ValueError text.
fn int_from_str(text: &str, base: u32) -> Result<PyInt, YErr> {
    let bad = || {
        let r: String = py_repr_str(text).chars().take(200).collect();
        plain(format!("invalid literal for int() with base {base}: {r}"))
    };
    let s = text.trim_matches(py_isspace);
    let (neg, mut s) = match s.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };
    let prefix = match base {
        16 => Some("0x"),
        8 => Some("0o"),
        2 => Some("0b"),
        _ => None,
    };
    if let Some(p) = prefix {
        if s.get(..2).is_some_and(|h| h.eq_ignore_ascii_case(p)) {
            s = &s[2..];
        }
    }
    if s.is_empty() {
        return Err(bad());
    }
    let mut n = PyInt::zero();
    for c in s.chars() {
        match c.to_digit(base) {
            Some(d) => n.mul_add(base as u64, d as u64),
            None => return Err(bad()),
        }
    }
    Ok(if neg { n.negated() } else { n })
}

/// `float(text)`; Err is the ValueError text.
fn float_from_str(text: &str) -> Result<f64, YErr> {
    text.trim_matches(py_isspace)
        .parse::<f64>()
        .map_err(|_| plain(format!("could not convert string to float: {}", py_repr_str(text))))
}

const INDEX_ERROR: &str = "string index out of range";

/// SafeConstructor.construct_yaml_int.
fn construct_int(raw: &str) -> Result<PyInt, YErr> {
    let mut value = raw.replace('_', "");
    let first = value.chars().next().ok_or_else(|| plain(INDEX_ERROR))?;
    let neg = first == '-';
    if first == '-' || first == '+' {
        value.remove(0);
    }
    let n = if value == "0" {
        PyInt::zero()
    } else if let Some(rest) = value.strip_prefix("0b") {
        int_from_str(rest, 2)?
    } else if let Some(rest) = value.strip_prefix("0x") {
        int_from_str(rest, 16)?
    } else if value.is_empty() {
        return Err(plain(INDEX_ERROR));
    } else if value.starts_with('0') {
        int_from_str(&value, 8)?
    } else if value.contains(':') {
        let digits = value.split(':').map(|p| int_from_str(p, 10)).collect::<Result<Vec<_>, _>>()?;
        if digits.iter().any(|d| d.neg) {
            // `!!int 1:-5`: signed parts; i128 is plenty for anything hand-written.
            let mut v: i128 = 0;
            for d in &digits {
                v = v.saturating_mul(60).saturating_add(d.to_i128().unwrap_or(0));
            }
            PyInt::from_i128(v)
        } else {
            let mut v = PyInt::zero();
            for d in &digits {
                v.mul_add(60, 0);
                v.add_mag(d);
            }
            v
        }
    } else {
        int_from_str(&value, 10)?
    };
    Ok(if neg { n.negated() } else { n })
}

/// SafeConstructor.construct_yaml_float.
fn construct_float(raw: &str) -> Result<f64, YErr> {
    let mut value = raw.replace('_', "").to_lowercase();
    let first = value.chars().next().ok_or_else(|| plain(INDEX_ERROR))?;
    let sign = if first == '-' { -1.0 } else { 1.0 };
    if first == '-' || first == '+' {
        value.remove(0);
    }
    if value == ".inf" {
        return Ok(sign * f64::INFINITY);
    }
    if value == ".nan" {
        return Ok(f64::NAN);
    }
    if value.contains(':') {
        let digits = value.split(':').map(float_from_str).collect::<Result<Vec<_>, _>>()?;
        let (mut base, mut v) = (1.0, 0.0);
        for d in digits.iter().rev() {
            v += d * base;
            base *= 60.0;
        }
        return Ok(sign * v);
    }
    Ok(sign * float_from_str(&value)?)
}

/// SafeConstructor.construct_yaml_bool: `bool_values[value.lower()]`.
fn construct_bool(raw: &str) -> Result<bool, YErr> {
    match raw.to_lowercase().as_str() {
        "yes" | "true" | "on" => Ok(true),
        "no" | "false" | "off" => Ok(false),
        other => Err(plain(py_repr_str(other))),
    }
}

fn py_date(y: i64, m: i64, d: i64) -> Result<NaiveDate, YErr> {
    if !(1..=9999).contains(&y) {
        return Err(plain(format!("year {y} is out of range")));
    }
    if !(1..=12).contains(&m) {
        return Err(plain("month must be in 1..12"));
    }
    NaiveDate::from_ymd_opt(y as i32, m as u32, d as u32).ok_or_else(|| plain("day is out of range for month"))
}

/// SafeConstructor.construct_yaml_timestamp.
fn construct_timestamp(value: &str) -> Result<PyVal, YErr> {
    let c = resolvers()
        .ts_parts
        .captures(value)
        .ok_or_else(|| plain("'NoneType' object has no attribute 'groupdict'"))?;
    let num = |k: &str| c.name(k).map(|m| m.as_str().parse::<i64>().unwrap_or(0));
    let (year, month, day) = (num("year").unwrap_or(0), num("month").unwrap_or(0), num("day").unwrap_or(0));
    let Some(hour) = num("hour") else {
        return py_date(year, month, day).map(PyVal::Date);
    };
    let (minute, second) = (num("minute").unwrap_or(0), num("second").unwrap_or(0));
    let mut fraction = 0u32;
    if let Some(f) = c.name("fraction").filter(|m| !m.as_str().is_empty()) {
        let mut s: String = f.as_str().chars().take(6).collect();
        while s.len() < 6 {
            s.push('0');
        }
        fraction = s.parse().unwrap_or(0);
    }
    let tz = if let Some(sign) = c.name("tz_sign") {
        let mut delta = num("tz_hour").unwrap_or(0) * 3600 + num("tz_minute").unwrap_or(0) * 60;
        if sign.as_str() == "-" {
            delta = -delta;
        }
        if delta.abs() >= 86400 {
            return Err(plain(format!(
                "offset must be a timedelta strictly between -timedelta(hours=24) and timedelta(hours=24), not {}.",
                timedelta_repr(delta)
            )));
        }
        Some(delta as i32)
    } else if c.name("tz").is_some() {
        Some(0)
    } else {
        None
    };
    let date = py_date(year, month, day)?;
    if !(0..=23).contains(&hour) {
        return Err(plain("hour must be in 0..23"));
    }
    if !(0..=59).contains(&minute) {
        return Err(plain("minute must be in 0..59"));
    }
    if !(0..=59).contains(&second) {
        return Err(plain("second must be in 0..59"));
    }
    let dt = date
        .and_hms_micro_opt(hour as u32, minute as u32, second as u32, fraction)
        .ok_or_else(|| plain("second must be in 0..59"))?;
    Ok(PyVal::DateTime(dt, tz))
}

/// SafeConstructor.construct_yaml_binary; Err is the ConstructorError problem text.
fn construct_binary(raw: &str) -> Result<Vec<u8>, String> {
    let chars: Vec<char> = raw.chars().collect();
    if let Some(i) = chars.iter().position(|c| !c.is_ascii()) {
        let mut j = i;
        while j < chars.len() && !chars[j].is_ascii() {
            j += 1;
        }
        let what = if j - i == 1 {
            let c = chars[i] as u32;
            let esc = if c <= 0xff {
                format!("\\x{c:02x}")
            } else if c <= 0xffff {
                format!("\\u{c:04x}")
            } else {
                format!("\\U{c:08x}")
            };
            format!("character '{esc}' in position {i}")
        } else {
            format!("characters in position {i}-{}", j - 1)
        };
        return Err(format!(
            "failed to convert base64 data into ascii: 'ascii' codec can't encode {what}: ordinal not in range(128)"
        ));
    }
    a2b_base64(raw.as_bytes()).map_err(|e| format!("failed to decode base64 data: {e}"))
}

/// `binascii.a2b_base64(data)` in its default, non-strict mode.
fn a2b_base64(data: &[u8]) -> Result<Vec<u8>, String> {
    let val = |c: u8| -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    };
    let mut out = Vec::new();
    let (mut quad_pos, mut leftchar, mut pads) = (0u8, 0u8, 0u8);
    for &ch in data {
        if ch == b'=' {
            if quad_pos >= 2 {
                pads += 1;
                if quad_pos + pads >= 4 {
                    return Ok(out);
                }
            }
            continue;
        }
        let Some(v) = val(ch) else { continue };
        pads = 0;
        match quad_pos {
            0 => {
                quad_pos = 1;
                leftchar = v;
            }
            1 => {
                quad_pos = 2;
                out.push((leftchar << 2) | (v >> 4));
                leftchar = v & 0x0f;
            }
            2 => {
                quad_pos = 3;
                out.push((leftchar << 4) | (v >> 2));
                leftchar = v & 0x03;
            }
            _ => {
                quad_pos = 0;
                out.push((leftchar << 6) | v);
                leftchar = 0;
            }
        }
    }
    match quad_pos {
        0 => Ok(out),
        1 => Err(format!(
            "Invalid base64-encoded string: number of data characters ({}) cannot be 1 more than a multiple of 4",
            out.len() / 3 * 4 + 1
        )),
        _ => Err("Incorrect padding".to_string()),
    }
}

// ------------------------------------------------------------------ scanner differences

fn is_yaml_break(ch: char) -> bool {
    matches!(ch, '\r' | '\n' | '\u{85}' | '\u{2028}' | '\u{2029}')
}

/// Index of the closing quote of the quoted scalar opening at `at`.
fn quoted_end(c: &[char], at: usize) -> Option<usize> {
    let q = *c.get(at)?;
    let double = q == '"';
    let mut i = at + 1;
    while i < c.len() {
        match c[i] {
            '\\' if double => i += 2,
            '\'' if !double && c.get(i + 1) == Some(&'\'') => i += 2,
            ch if ch == q => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// Where the block scalar whose indicator sits at `ind` stops, by PyYAML's rule: the
/// first non-blank line indented less than the content (a tab is never indentation).
fn block_end(c: &[char], ind: usize) -> usize {
    let line_end = |mut i: usize| {
        while i < c.len() && c[i] != '\n' {
            i += 1;
        }
        i
    };
    let mut ls = line_end(ind) + 1;
    let mut max_indent = 0;
    let indent = loop {
        if ls >= c.len() {
            return c.len();
        }
        let mut j = ls;
        while j < c.len() && c[j] == ' ' {
            j += 1;
        }
        if j >= c.len() {
            return c.len();
        }
        if c[j] == '\n' || c[j] == '\r' {
            max_indent = max_indent.max(j - ls);
            ls = line_end(ls) + 1;
            continue;
        }
        break max_indent.max(j - ls).max(1);
    };
    while ls < c.len() {
        let mut j = ls;
        while j < c.len() && c[j] == ' ' {
            j += 1;
        }
        let blank = j >= c.len() || c[j] == '\n' || c[j] == '\r';
        if !blank && j - ls < indent {
            return ls;
        }
        ls = line_end(ls) + 1;
    }
    c.len()
}

/// The first tab PyYAML's scanner would trip on: outside quoted scalars, block scalar
/// content and comments, PyYAML treats every tab as "a character that cannot start any
/// token" (scan_to_next_token and scan_plain_spaces only skip spaces).
fn first_bad_tab(c: &[char], tokens: &[(usize, TokenType)]) -> Option<usize> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for (i, (at, tt)) in tokens.iter().enumerate() {
        let TokenType::Scalar(style, _) = tt else { continue };
        match style {
            TScalarStyle::SingleQuoted | TScalarStyle::DoubleQuoted => {
                if let Some(end) = quoted_end(c, *at) {
                    spans.push((*at, end + 1));
                }
            }
            TScalarStyle::Literal | TScalarStyle::Folded => {
                let from = if i > 0 { tokens[i - 1].0.min(*at) } else { 0 };
                if let Some(ind) = (from..*at.min(&c.len())).find(|&k| matches!(c[k], '|' | '>')) {
                    let mut end = block_end(c, ind);
                    let next_node = tokens[i + 1..]
                        .iter()
                        .find(|(_, t)| !matches!(t, TokenType::BlockEnd | TokenType::StreamEnd | TokenType::DocumentEnd));
                    if let Some((next, _)) = next_node {
                        let mut line_start = (*next).min(c.len());
                        while line_start > 0 && c[line_start - 1] != '\n' {
                            line_start -= 1;
                        }
                        if line_start > ind {
                            end = end.min(line_start);
                        }
                    }
                    spans.push((ind, end));
                }
            }
            _ => {}
        }
    }
    spans.sort();
    let mut si = 0;
    let mut i = 0;
    while i < c.len() {
        while si < spans.len() && spans[si].1 <= i {
            si += 1;
        }
        if si < spans.len() && spans[si].0 <= i {
            i = spans[si].1;
            continue;
        }
        match c[i] {
            '#' if i == 0 || matches!(c[i - 1], ' ' | '\t' | '\n' | '\r') => {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            '\t' => return Some(i),
            _ => i += 1,
        }
    }
    None
}

/// PyYAML's ComposerError for a re-used anchor name: (name, first node start, second).
fn duplicate_anchor(tokens: &[(usize, TokenType)]) -> Option<(String, usize, usize)> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for (i, (at, tt)) in tokens.iter().enumerate() {
        let TokenType::Anchor(name) = tt else { continue };
        let start = match tokens.get(i.wrapping_sub(1)) {
            Some((tag_at, TokenType::Tag(..))) => *tag_at,
            _ => *at,
        };
        if let Some(first) = seen.get(name.as_str()) {
            return Some((name.clone(), *first, start));
        }
        seen.insert(name, start);
    }
    None
}

/// PyYAML's scan_flow_scalar: decode the quoted scalar opening at `at` the way PyYAML
/// does (NEL, LS and PS are line breaks; `\N \_ \L \P` escapes). None leaves the
/// scalar to yaml-rust2, which already accepted it.
fn decode_quoted(c: &[char], at: usize) -> Option<String> {
    let quote = *c.get(at)?;
    let double = quote == '"';
    if !double && quote != '\'' {
        return None;
    }
    let peek = |i: usize| c.get(i).copied().unwrap_or('\0');
    let line_break = |i: usize| -> (String, usize) {
        match peek(i) {
            '\r' if peek(i + 1) == '\n' => ("\n".to_string(), i + 2),
            '\r' | '\n' | '\u{85}' => ("\n".to_string(), i + 1),
            ch @ ('\u{2028}' | '\u{2029}') => (ch.to_string(), i + 1),
            _ => (String::new(), i),
        }
    };
    let breaks = |mut i: usize| -> Option<(Vec<String>, usize)> {
        let mut chunks = Vec::new();
        loop {
            let prefix: String = (0..3).map(|k| peek(i + k)).collect();
            let after = peek(i + 3);
            if (prefix == "---" || prefix == "...") && (matches!(after, '\0' | ' ' | '\t') || is_yaml_break(after)) {
                return None;
            }
            while matches!(peek(i), ' ' | '\t') {
                i += 1;
            }
            if is_yaml_break(peek(i)) {
                let (b, j) = line_break(i);
                chunks.push(b);
                i = j;
            } else {
                return Some((chunks, i));
            }
        }
    };
    let mut out = String::new();
    let mut i = at + 1;
    loop {
        loop {
            let start = i;
            while !matches!(peek(i), '\'' | '"' | '\\' | '\0' | ' ' | '\t') && !is_yaml_break(peek(i)) {
                i += 1;
            }
            out.extend(&c[start..i]);
            let ch = peek(i);
            if !double && ch == '\'' && peek(i + 1) == '\'' {
                out.push('\'');
                i += 2;
            } else if (double && ch == '\'') || (!double && (ch == '"' || ch == '\\')) {
                out.push(ch);
                i += 1;
            } else if double && ch == '\\' {
                i += 1;
                let e = peek(i);
                let replacement = match e {
                    '0' => Some('\0'),
                    'a' => Some('\u{7}'),
                    'b' => Some('\u{8}'),
                    't' | '\t' => Some('\t'),
                    'n' => Some('\n'),
                    'v' => Some('\u{b}'),
                    'f' => Some('\u{c}'),
                    'r' => Some('\r'),
                    'e' => Some('\u{1b}'),
                    ' ' => Some(' '),
                    '"' => Some('"'),
                    '\\' => Some('\\'),
                    '/' => Some('/'),
                    'N' => Some('\u{85}'),
                    '_' => Some('\u{a0}'),
                    'L' => Some('\u{2028}'),
                    'P' => Some('\u{2029}'),
                    _ => None,
                };
                if let Some(r) = replacement {
                    out.push(r);
                    i += 1;
                } else if let Some(n) = match e {
                    'x' => Some(2),
                    'u' => Some(4),
                    'U' => Some(8),
                    _ => None,
                } {
                    i += 1;
                    let hex: String = (0..n).map(|k| peek(i + k)).collect();
                    if !hex.chars().all(|h| h.is_ascii_hexdigit()) {
                        return None;
                    }
                    out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?).unwrap_or('\u{fffd}'));
                    i += n;
                } else if is_yaml_break(e) {
                    let (_, j) = line_break(i);
                    let (bs, j) = breaks(j)?;
                    for b in bs {
                        out.push_str(&b);
                    }
                    i = j;
                } else {
                    return None;
                }
            } else {
                break;
            }
        }
        if peek(i) == quote {
            return Some(out);
        }
        let start = i;
        while matches!(peek(i), ' ' | '\t') {
            i += 1;
        }
        let ch = peek(i);
        if ch == '\0' {
            return None;
        }
        if is_yaml_break(ch) {
            let (lb, j) = line_break(i);
            let (bs, j) = breaks(j)?;
            i = j;
            if lb != "\n" {
                out.push_str(&lb);
            } else if bs.is_empty() {
                out.push(' ');
            }
            for b in bs {
                out.push_str(&b);
            }
        } else {
            out.extend(&c[start..i]);
        }
    }
}

// ------------------------------------------------------------------ composer

enum Kind {
    Scalar(String),
    Seq(Vec<usize>),
    Map(Vec<(usize, usize)>),
}

struct Node {
    tag: String,
    kind: Kind,
    mark: Mark,
}

impl Node {
    fn id(&self) -> &'static str {
        match self.kind {
            Kind::Scalar(_) => "scalar",
            Kind::Seq(_) => "sequence",
            Kind::Map(_) => "mapping",
        }
    }
}

struct Composer<'a> {
    src: &'a Src,
    events: Vec<(Event, usize)>,
    pos: usize,
    nodes: Vec<Node>,
    anchors: HashMap<usize, usize>,
}

impl Composer<'_> {
    fn push(&mut self, tag: String, kind: Kind, ptr: usize) -> usize {
        self.nodes.push(Node { tag, kind, mark: self.src.mark(ptr) });
        self.nodes.len() - 1
    }

    /// Step back from `p` over whitespace and up to `n` node properties (`!tag`, `&anchor`):
    /// PyYAML starts a node's mark at its first property.
    fn back_over_props(&self, mut p: usize, n: usize) -> usize {
        let c = &self.src.chars;
        for _ in 0..n {
            let mut q = p.min(c.len());
            while q > 0 && matches!(c[q - 1], ' ' | '\t' | '\n' | '\r') {
                q -= 1;
            }
            let end = q;
            while q > 0 && !matches!(c[q - 1], ' ' | '\t' | '\n' | '\r' | '[' | '{' | ',') {
                q -= 1;
            }
            if q < end && matches!(c[q], '!' | '&') {
                p = q;
            } else {
                break;
            }
        }
        p
    }

    /// True when the block scalar whose indicator sits at `ind` runs to the end of the
    /// text and its last line has no line break. PyYAML's scanner then has no final break
    /// to keep (`scan_line_break()` returns '' at EOF); yaml-rust2 appends one anyway.
    fn block_scalar_ends_unbroken(&self, ind: usize) -> bool {
        let later_nodes = self.events[self.pos..]
            .iter()
            .any(|(e, _)| !matches!(e, Event::SequenceEnd | Event::MappingEnd));
        let c = &self.src.chars;
        if later_nodes || ind >= c.len() {
            return false;
        }
        let header_end = (ind..c.len()).find(|&i| c[i] == '\n').unwrap_or(c.len());
        if c[ind..header_end].contains(&'-') || header_end >= c.len() {
            return false;
        }
        let lines: Vec<&[char]> = c[header_end + 1..].split(|&ch| ch == '\n').collect();
        let indent_of = |l: &[char]| l.iter().take_while(|&&ch| ch == ' ').count();
        let blank = |l: &[char]| l.iter().all(|&ch| ch == ' ' || ch == '\r');
        let Some(first) = lines.iter().find(|l| !blank(l)) else { return false };
        let content_indent = indent_of(first);
        let last = lines.last().copied().unwrap_or(&[]);
        // A final line of spaces only is swallowed as a break-less blank line; a final
        // content line simply has no break. Either way yaml-rust2 adds one '\n' too many.
        !last.is_empty() && (blank(last) || indent_of(last) >= content_indent)
    }

    fn compose(&mut self) -> usize {
        let (ev, at) = self.events[self.pos].clone();
        self.pos += 1;
        match ev {
            Event::Alias(id) => self.anchors.get(&id).copied().unwrap_or(0),
            Event::Scalar(value, style, anchor, tag) => {
                let tag = tag.map(|t| format!("{}{}", t.handle, t.suffix));
                let props = (anchor != 0) as usize + tag.is_some() as usize;
                let mut p = at;
                let mut value = value;
                if matches!(style, TScalarStyle::SingleQuoted | TScalarStyle::DoubleQuoted) {
                    if let Some(v) = decode_quoted(&self.src.chars, at) {
                        value = v;
                    }
                }
                let tag = match tag {
                    Some(t) if t != "!" => t,
                    Some(_) => resolve(&value),
                    None if style == TScalarStyle::Plain => resolve(&value),
                    None => format!("{T}str"),
                };
                if matches!(style, TScalarStyle::Literal | TScalarStyle::Folded) {
                    if !matches!(self.src.at(p), '|' | '>') {
                        if let Some(q) = (0..p.min(self.src.chars.len())).rev().find(|&i| matches!(self.src.chars[i], '|' | '>')) {
                            p = q;
                        }
                    }
                    if self.block_scalar_ends_unbroken(p) && value.ends_with('\n') {
                        value.pop();
                    }
                }
                let p = self.back_over_props(p, props);
                let idx = self.push(tag, Kind::Scalar(value), p);
                if anchor != 0 {
                    self.anchors.insert(anchor, idx);
                }
                idx
            }
            Event::SequenceStart(anchor, tag) => {
                let tag = tag.map(|t| format!("{}{}", t.handle, t.suffix));
                let props = (anchor != 0) as usize + tag.is_some() as usize;
                let tag = match tag {
                    Some(t) if t != "!" => t,
                    _ => format!("{T}seq"),
                };
                let mut p = at;
                if !matches!(self.src.at(p), '[' | '-') {
                    // An indentless sequence: yaml-rust2 points at the first entry's content.
                    let mut q = p.min(self.src.chars.len());
                    while q > 0 && matches!(self.src.chars[q - 1], ' ' | '\t') {
                        q -= 1;
                    }
                    if q > 0 && self.src.chars[q - 1] == '-' {
                        p = q - 1;
                    }
                }
                let p = self.back_over_props(p, props);
                let idx = self.push(tag, Kind::Seq(Vec::new()), p);
                if anchor != 0 {
                    self.anchors.insert(anchor, idx);
                }
                let mut items = Vec::new();
                while !matches!(self.events[self.pos].0, Event::SequenceEnd) {
                    items.push(self.compose());
                }
                self.pos += 1;
                self.nodes[idx].kind = Kind::Seq(items);
                idx
            }
            Event::MappingStart(anchor, tag) => {
                let tag = tag.map(|t| format!("{}{}", t.handle, t.suffix));
                let props = (anchor != 0) as usize + tag.is_some() as usize;
                let tag = match tag {
                    Some(t) if t != "!" => t,
                    _ => format!("{T}map"),
                };
                let flow_or_explicit = matches!(self.src.at(at), '{' | '?');
                let idx = self.push(tag, Kind::Map(Vec::new()), at);
                if anchor != 0 {
                    self.anchors.insert(anchor, idx);
                }
                let mut pairs = Vec::new();
                while !matches!(self.events[self.pos].0, Event::MappingEnd) {
                    let k = self.compose();
                    let v = self.compose();
                    pairs.push((k, v));
                }
                self.pos += 1;
                // A block mapping starts where its first key does.
                let p = match pairs.first() {
                    Some((k, _)) if !flow_or_explicit => self.nodes[*k].mark.ptr,
                    _ => at,
                };
                let p = self.back_over_props(p, props);
                self.nodes[idx].mark = self.src.mark(p);
                self.nodes[idx].kind = Kind::Map(pairs);
                idx
            }
            _ => 0,
        }
    }
}

// ------------------------------------------------------------------ constructor

#[derive(Clone)]
enum Built {
    Val(PyVal),
    Cont(usize),
}

enum Cont {
    List(Vec<Built>),
    Dict(Vec<(PyVal, Built)>),
    Set(Vec<PyVal>),
    Pairs(Vec<(Built, Built)>),
}

#[derive(Clone, Copy, PartialEq)]
enum Gen {
    Seq,
    Map,
    Set,
    Omap,
    Pairs,
}

/// SafeConstructor: containers come back empty and fill in later rounds, as PyYAML's
/// generator constructors do, so errors surface in PyYAML's order.
struct Constructor {
    nodes: Vec<Node>,
    conts: Vec<Cont>,
    built: HashMap<usize, Built>,
    gens: Vec<(Gen, usize, usize)>,
}

impl Constructor {
    fn marked(&self, context: Option<&str>, cnode: Option<usize>, problem: String, pnode: usize) -> YErr {
        YErr::Marked {
            context: context.map(str::to_string),
            context_mark: cnode.map(|n| self.nodes[n].mark),
            problem,
            problem_mark: self.nodes[pnode].mark,
        }
    }

    fn scalar(&self, n: usize) -> Result<String, YErr> {
        match &self.nodes[n].kind {
            Kind::Scalar(v) => Ok(v.clone()),
            _ => Err(self.marked(None, None, format!("expected a scalar node, but found {}", self.nodes[n].id()), n)),
        }
    }

    fn object(&mut self, n: usize) -> Result<Built, YErr> {
        if let Some(b) = self.built.get(&n) {
            return Ok(b.clone());
        }
        let tag = self.nodes[n].tag.clone();
        let b = match tag.strip_prefix(T) {
            Some("null") => {
                self.scalar(n)?;
                Built::Val(PyVal::None)
            }
            Some("bool") => Built::Val(PyVal::Bool(construct_bool(&self.scalar(n)?)?)),
            Some("int") => Built::Val(PyVal::Int(construct_int(&self.scalar(n)?)?)),
            Some("float") => Built::Val(PyVal::Float(construct_float(&self.scalar(n)?)?)),
            Some("binary") => {
                let v = self.scalar(n)?;
                Built::Val(PyVal::Bytes(construct_binary(&v).map_err(|p| self.marked(None, None, p, n))?))
            }
            Some("timestamp") => Built::Val(construct_timestamp(&self.scalar(n)?)?),
            Some("str") => Built::Val(PyVal::Str(self.scalar(n)?)),
            Some(g @ ("seq" | "map" | "set" | "omap" | "pairs")) => {
                let (gen, cont) = match g {
                    "seq" => (Gen::Seq, Cont::List(Vec::new())),
                    "map" => (Gen::Map, Cont::Dict(Vec::new())),
                    "set" => (Gen::Set, Cont::Set(Vec::new())),
                    "omap" => (Gen::Omap, Cont::Pairs(Vec::new())),
                    _ => (Gen::Pairs, Cont::Pairs(Vec::new())),
                };
                self.conts.push(cont);
                let c = self.conts.len() - 1;
                self.gens.push((gen, n, c));
                Built::Cont(c)
            }
            _ => {
                let problem = format!("could not determine a constructor for the tag {}", py_repr_str(&tag));
                return Err(self.marked(None, None, problem, n));
            }
        };
        self.built.insert(n, b.clone());
        Ok(b)
    }

    fn pairs(&self, n: usize) -> Vec<(usize, usize)> {
        match &self.nodes[n].kind {
            Kind::Map(p) => p.clone(),
            _ => Vec::new(),
        }
    }

    /// SafeConstructor.flatten_mapping: splice `<<` merges in, turn `=` keys into str.
    fn flatten(&mut self, n: usize) -> Result<(), YErr> {
        const CTX: Option<&str> = Some("while constructing a mapping");
        let mut merge: Vec<(usize, usize)> = Vec::new();
        let mut index = 0;
        loop {
            let pairs = self.pairs(n);
            if index >= pairs.len() {
                break;
            }
            let (k, v) = pairs[index];
            if self.nodes[k].tag == format!("{T}merge") {
                if let Kind::Map(p) = &mut self.nodes[n].kind {
                    p.remove(index);
                }
                match &self.nodes[v].kind {
                    Kind::Map(_) => {
                        self.flatten(v)?;
                        merge.extend(self.pairs(v));
                    }
                    Kind::Seq(subs) => {
                        let subs = subs.clone();
                        let mut submerge = Vec::new();
                        for sub in subs {
                            if !matches!(self.nodes[sub].kind, Kind::Map(_)) {
                                let problem = format!("expected a mapping for merging, but found {}", self.nodes[sub].id());
                                return Err(self.marked(CTX, Some(n), problem, sub));
                            }
                            self.flatten(sub)?;
                            submerge.push(self.pairs(sub));
                        }
                        submerge.reverse();
                        for value in submerge {
                            merge.extend(value);
                        }
                    }
                    Kind::Scalar(_) => {
                        let problem = format!(
                            "expected a mapping or list of mappings for merging, but found {}",
                            self.nodes[v].id()
                        );
                        return Err(self.marked(CTX, Some(n), problem, v));
                    }
                }
            } else if self.nodes[k].tag == format!("{T}value") {
                self.nodes[k].tag = format!("{T}str");
                index += 1;
            } else {
                index += 1;
            }
        }
        if !merge.is_empty() {
            merge.extend(self.pairs(n));
            self.nodes[n].kind = Kind::Map(merge);
        }
        Ok(())
    }

    /// SafeConstructor.construct_mapping (flatten, then BaseConstructor's loop).
    fn mapping(&mut self, n: usize) -> Result<Vec<(PyVal, Built)>, YErr> {
        if matches!(self.nodes[n].kind, Kind::Map(_)) {
            self.flatten(n)?;
        }
        let Kind::Map(pairs) = &self.nodes[n].kind else {
            let problem = format!("expected a mapping node, but found {}", self.nodes[n].id());
            return Err(self.marked(None, None, problem, n));
        };
        let pairs = pairs.clone();
        let mut out: Vec<(PyVal, Built)> = Vec::new();
        for (k, v) in pairs {
            let key = match self.object(k)? {
                Built::Val(x) => x,
                Built::Cont(_) => {
                    return Err(self.marked(Some("while constructing a mapping"), Some(n), "found unhashable key".into(), k))
                }
            };
            let val = self.object(v)?;
            match out.iter_mut().find(|(x, _)| py_eq(x, &key)) {
                Some(slot) => slot.1 = val,
                None => out.push((key, val)),
            }
        }
        Ok(out)
    }

    fn run(&mut self, gen: Gen, n: usize, c: usize) -> Result<(), YErr> {
        match gen {
            Gen::Seq => {
                let Kind::Seq(items) = &self.nodes[n].kind else {
                    let problem = format!("expected a sequence node, but found {}", self.nodes[n].id());
                    return Err(self.marked(None, None, problem, n));
                };
                let items = items.clone();
                let mut out = Vec::new();
                for i in items {
                    out.push(self.object(i)?);
                }
                self.conts[c] = Cont::List(out);
            }
            Gen::Map => {
                let m = self.mapping(n)?;
                self.conts[c] = Cont::Dict(m);
            }
            Gen::Set => {
                let m = self.mapping(n)?;
                let mut s: Vec<PyVal> = Vec::new();
                for (k, _) in m {
                    if !s.iter().any(|x| py_eq(x, &k)) {
                        s.push(k);
                    }
                }
                self.conts[c] = Cont::Set(s);
            }
            Gen::Omap | Gen::Pairs => {
                let ctx = Some(if gen == Gen::Omap { "while constructing an ordered map" } else { "while constructing pairs" });
                let Kind::Seq(items) = &self.nodes[n].kind else {
                    let problem = format!("expected a sequence, but found {}", self.nodes[n].id());
                    return Err(self.marked(ctx, Some(n), problem, n));
                };
                let items = items.clone();
                let mut out = Vec::new();
                for sub in items {
                    let Kind::Map(pairs) = &self.nodes[sub].kind else {
                        let problem = format!("expected a mapping of length 1, but found {}", self.nodes[sub].id());
                        return Err(self.marked(ctx, Some(n), problem, sub));
                    };
                    if pairs.len() != 1 {
                        let problem = format!("expected a single mapping item, but found {} items", pairs.len());
                        return Err(self.marked(ctx, Some(n), problem, sub));
                    }
                    let (k, v) = pairs[0];
                    let key = self.object(k)?;
                    let val = self.object(v)?;
                    out.push((key, val));
                }
                self.conts[c] = Cont::Pairs(out);
            }
        }
        Ok(())
    }

    fn materialize(&self, b: &Built, stack: &mut Vec<usize>) -> PyVal {
        let c = match b {
            Built::Val(v) => return v.clone(),
            Built::Cont(c) => *c,
        };
        if stack.contains(&c) {
            return PyVal::Recursive(match self.conts[c] {
                Cont::Dict(_) => "dict",
                Cont::Set(_) => "set",
                _ => "list",
            });
        }
        stack.push(c);
        let v = match &self.conts[c] {
            Cont::List(items) => PyVal::List(items.iter().map(|i| self.materialize(i, stack)).collect()),
            Cont::Dict(pairs) => PyVal::Dict(pairs.iter().map(|(k, v)| (k.clone(), self.materialize(v, stack))).collect()),
            Cont::Set(items) => PyVal::Set(items.clone()),
            Cont::Pairs(items) => PyVal::List(
                items
                    .iter()
                    .map(|(k, v)| PyVal::Tuple(vec![self.materialize(k, stack), self.materialize(v, stack)]))
                    .collect(),
            ),
        };
        stack.pop();
        v
    }
}

// ------------------------------------------------------------------ entry point

/// Reader.NON_PRINTABLE's complement.
fn printable(c: char) -> bool {
    matches!(c as u32, 0x09 | 0x0A | 0x0D | 0x20..=0x7E | 0x85 | 0xA0..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)
}

/// The first document's events (positions as char offsets into the full text) and, when
/// another document follows, where it starts.
/// yaml-rust2's own error: where it points, its bare info, and its full text.
struct RustErr {
    at: usize,
    info: String,
    text: String,
}

fn rust_err(e: yaml_rust2::ScanError, offset: usize) -> RustErr {
    RustErr { at: e.marker().index() + offset, info: e.info().to_string(), text: e.to_string() }
}

fn document_events(body: &str, offset: usize) -> Result<(Vec<(Event, usize)>, Option<usize>), RustErr> {
    let mut parser = Parser::new_from_str(body);
    let mut next = || parser.next_token().map(|(ev, m)| (ev, m.index() + offset)).map_err(|e| rust_err(e, offset));
    loop {
        match next()? {
            (Event::StreamStart, _) | (Event::Nothing, _) => continue,
            (Event::DocumentStart, _) => break,
            _ => return Ok((Vec::new(), None)),
        }
    }
    let mut events = Vec::new();
    loop {
        match next()? {
            (Event::DocumentEnd, _) | (Event::StreamEnd, _) => break,
            ev => events.push(ev),
        }
    }
    match next() {
        Ok((Event::DocumentStart, at)) => Ok((events, Some(at))),
        Ok(_) => Ok((events, None)),
        Err(e) => Err(e),
    }
}

/// `yaml.safe_load(text)`. `name` is what PyYAML's marks call the stream
/// ("<unicode string>" for a str, the file's name for an open file); snippets show only
/// for str input. Err is `str(exc)` of whatever PyYAML would raise.
pub fn safe_load(text: &str, name: &str, snippet: bool) -> Result<PyVal, String> {
    let chars: Vec<char> = text.chars().collect();
    if let Some((i, c)) = chars.iter().enumerate().find(|(_, c)| !printable(**c)) {
        return Err(format!(
            "unacceptable character #x{:04x}: special characters are not allowed\n  in \"{}\", position {}",
            *c as u32, name, i
        ));
    }
    let src = Src { chars, name: name.to_string(), snippet };
    // The scanner skips a leading BOM; marks still count it in the pointer.
    let (body, offset) = match text.strip_prefix('\u{feff}') {
        Some(b) => (b, 1),
        None => (text, 0),
    };
    // Where PyYAML (YAML 1.1) accepts what yaml-rust2 (YAML 1.2) refuses, rewrite the
    // text yaml-rust2 sees without moving any character, and scan again:
    // - a comment straight after a closing quote or flow indicator ("x"# c): blank it;
    // - a multi-line quoted scalar whose continuation lines are under-indented or
    //   tab-indented: put it on one line (its value comes from decode_quoted on the
    //   original text, so the folding is PyYAML's).
    let mut patched: Vec<char> = body.chars().collect();
    let tokens: Vec<(usize, TokenType)> = loop {
        let mut sc = Scanner::new(patched.clone().into_iter());
        let tokens: Vec<(usize, TokenType)> = (&mut sc).map(|Token(m, tt)| (m.index() + offset, tt)).collect();
        let Some(e) = sc.get_error() else { break tokens };
        let p = e.marker().index();
        let mut changed = false;
        if e.info().contains("comments must be separated") {
            if patched.get(p) == Some(&'#') && p > 0 && matches!(patched[p - 1], '"' | '\'' | ']' | '}' | ',' | '[' | '{') {
                let mut i = p;
                while i < patched.len() && !matches!(patched[i], '\n' | '\r') {
                    patched[i] = ' ';
                    i += 1;
                }
                changed = true;
            }
        } else if matches!(e.info(), "invalid indentation in quoted scalar" | "tab cannot be used as indentation") {
            let from = tokens.last().map(|(at, _)| at - offset).unwrap_or(0).min(p);
            let opener = if matches!(patched.get(p), Some('"' | '\'')) {
                Some(p)
            } else {
                (from..p).find(|&k| matches!(patched[k], '"' | '\''))
            };
            if let Some((o, end)) = opener.and_then(|o| quoted_end(&patched, o).map(|end| (o, end))) {
                let mut k = end + 1;
                while k < patched.len() && matches!(patched[k], ' ' | '\t') {
                    k += 1;
                }
                let is_key = patched.get(k) == Some(&':');
                if !is_key && decode_quoted(&src.chars, o + offset).is_some() {
                    for ch in &mut patched[o..=end] {
                        if matches!(*ch, '\n' | '\r' | '\t') {
                            *ch = ' ';
                            changed = true;
                        }
                    }
                }
            }
        }
        if !changed {
            break tokens;
        }
    };
    let body: String = patched.iter().collect();
    let mut full: Vec<char> = if offset == 1 { vec!['\u{feff}'] } else { Vec::new() };
    full.extend(&patched);
    // What PyYAML rejects and yaml-rust2 accepts (tabs between tokens, a re-used anchor
    // name), raised in stream order against yaml-rust2's own first error.
    let mut strict: Vec<(usize, YErr)> = Vec::new();
    if let Some(t) = first_bad_tab(&full, &tokens) {
        let err = YErr::Marked {
            context: Some("while scanning for the next token".to_string()),
            context_mark: None,
            problem: "found character '\\t' that cannot start any token".to_string(),
            problem_mark: src.mark(t),
        };
        strict.push((t, err));
    }
    if let Some((anchor, first, second)) = duplicate_anchor(&tokens) {
        let err = YErr::Marked {
            context: Some(format!("found duplicate anchor {}; first occurrence", py_repr_str(&anchor))),
            context_mark: Some(src.mark(first)),
            problem: "second occurrence".to_string(),
            problem_mark: src.mark(second),
        };
        strict.push((second, err));
    }
    strict.sort_by_key(|(at, _)| *at);
    let strict = strict.into_iter().next();
    let (events, second) = match document_events(&body, offset) {
        Ok(found) => found,
        Err(e) => {
            // A tab yaml-rust2 also trips on is still PyYAML's tab error.
            let at = if e.info.contains("tab") { usize::MAX } else { e.at };
            return Err(match strict {
                Some((s, err)) if s <= at => err.render(&src),
                _ => e.text,
            });
        }
    };
    if let Some((s, err)) = strict {
        if second.is_none_or(|doc| s < doc) {
            return Err(err.render(&src));
        }
    }
    if events.is_empty() {
        return Ok(PyVal::None);
    }
    let mut comp = Composer { src: &src, events, pos: 0, nodes: Vec::new(), anchors: HashMap::new() };
    let root = comp.compose();
    let nodes = std::mem::take(&mut comp.nodes);
    if let Some(at) = second {
        let err = YErr::Marked {
            context: Some("expected a single document in the stream".to_string()),
            context_mark: Some(nodes[root].mark),
            problem: "but found another document".to_string(),
            problem_mark: src.mark(at),
        };
        return Err(err.render(&src));
    }
    let mut con = Constructor { nodes, conts: Vec::new(), built: HashMap::new(), gens: Vec::new() };
    let top = con.object(root).map_err(|e| e.render(&src))?;
    while !con.gens.is_empty() {
        for (gen, n, c) in std::mem::take(&mut con.gens) {
            con.run(gen, n, c).map_err(|e| e.render(&src))?;
        }
    }
    Ok(con.materialize(&top, &mut Vec::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(doc: &str) -> String {
        match safe_load(doc, "<unicode string>", true) {
            Ok(v) => v.repr(),
            Err(e) => format!("ERR {e}"),
        }
    }

    #[test]
    fn yaml11_scalars_resolve_like_pyyaml() {
        assert_eq!(load("a: no"), "{'a': False}");
        assert_eq!(load("a: Off"), "{'a': False}");
        assert_eq!(load("a: y"), "{'a': 'y'}");
        assert_eq!(load("a: 017"), "{'a': 15}");
        assert_eq!(load("a: 09"), "{'a': '09'}");
        assert_eq!(load("a: 1:30"), "{'a': 90}");
        assert_eq!(load("a: 1e3"), "{'a': '1e3'}");
        assert_eq!(load("a: 1.0e+3"), "{'a': 1000.0}");
        assert_eq!(load("a: 2026-01-01"), "{'a': datetime.date(2026, 1, 1)}");
        assert_eq!(
            load("a: 2026-01-01 10:00:00 +5"),
            "{'a': datetime.datetime(2026, 1, 1, 10, 0, tzinfo=datetime.timezone(datetime.timedelta(seconds=18000)))}"
        );
        assert_eq!(load("a: 1e16") , "{'a': '1e16'}");
        assert_eq!(load("a: 0x123456789abcdef0123456789"), "{'a': 90144042682896311822508713865}");
        assert_eq!(load("{1: a, 1.0: b, true: c}"), "{1: 'c'}");
        assert_eq!(load("a: &x [*x]"), "{'a': [[...]]}");
    }

    #[test]
    fn constructor_errors_match_pyyaml_text() {
        assert_eq!(
            load("a: !foo bar"),
            "ERR could not determine a constructor for the tag '!foo'\n  in \"<unicode string>\", line 1, column 4:\n    a: !foo bar\n       ^"
        );
        assert_eq!(
            load("? [1]\n: 2"),
            "ERR while constructing a mapping\n  in \"<unicode string>\", line 1, column 1:\n    ? [1]\n    ^\nfound unhashable key\n  in \"<unicode string>\", line 1, column 3:\n    ? [1]\n      ^"
        );
        assert_eq!(load("a: !!int 08"), "ERR invalid literal for int() with base 8: '08'");
        assert_eq!(load("a: 2026-02-30"), "ERR day is out of range for month");
        assert_eq!(
            load("x: \u{7}"),
            "ERR unacceptable character #x0007: special characters are not allowed\n  in \"<unicode string>\", position 3"
        );
    }

    #[test]
    fn float_repr_matches_python() {
        assert_eq!(float_repr(1e16), "1e+16");
        assert_eq!(float_repr(1e15), "1000000000000000.0");
        assert_eq!(float_repr(0.0001), "0.0001");
        assert_eq!(float_repr(0.00001), "1e-05");
        assert_eq!(float_repr(123456789.123456789), "123456789.12345679");
        assert_eq!(float_repr(-0.0), "-0.0");
    }
}
