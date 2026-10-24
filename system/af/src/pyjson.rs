//! Python's `json.dumps`, `str()` and `repr()` for the values the voice lint and the hooks
//! print or compare: key order as built, float repr as CPython's, `ensure_ascii` either way.

use crate::pyos::py_repr_str;
use serde_json::Value;

/// A Python value as `json.dumps` sees it.
#[derive(Debug, Clone, PartialEq)]
pub enum PyVal {
    None,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<PyVal>),
    Dict(Vec<(String, PyVal)>),
}

impl PyVal {
    pub fn s(v: impl Into<String>) -> PyVal {
        PyVal::Str(v.into())
    }
    pub fn dict(items: Vec<(&str, PyVal)>) -> PyVal {
        PyVal::Dict(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }
}

/// `repr(float)`: the shortest round-trip digits, fixed between 1e-4 and 1e16, else
/// scientific with a signed two-digit exponent.
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "nan".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "inf".into() } else { "-inf".into() };
    }
    let sci = format!("{:e}", f.abs());
    let (mant, exp) = sci.split_once('e').unwrap_or((&sci, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let digits: String = mant.chars().filter(|c| c.is_ascii_digit()).collect();
    let sign = if f.is_sign_negative() { "-" } else { "" };
    let body = if (-4..16).contains(&exp) {
        if exp >= 0 {
            let int_len = exp as usize + 1;
            if digits.len() <= int_len {
                format!("{}{}.0", digits, "0".repeat(int_len - digits.len()))
            } else {
                format!("{}.{}", &digits[..int_len], &digits[int_len..])
            }
        } else {
            format!("0.{}{}", "0".repeat((-exp - 1) as usize), digits)
        }
    } else {
        let rest = &digits[1..];
        let m = if rest.is_empty() { digits[..1].to_string() } else { format!("{}.{}", &digits[..1], rest) };
        format!("{}e{}{:02}", m, if exp < 0 { '-' } else { '+' }, exp.abs())
    };
    format!("{sign}{body}")
}

/// `round(x, 2)`: correctly rounded on the exact binary value, as CPython does.
pub fn round2(x: f64) -> f64 {
    format!("{x:.2}").parse().unwrap_or(x)
}

fn esc_str(s: &str, ascii: bool) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if ascii && (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", u));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn scalar(v: &PyVal, ascii: bool) -> Option<String> {
    Some(match v {
        PyVal::None => "null".into(),
        PyVal::Bool(b) => if *b { "true" } else { "false" }.into(),
        PyVal::Int(i) => i.to_string(),
        PyVal::Float(f) => {
            if f.is_nan() {
                "NaN".into()
            } else if f.is_infinite() {
                if *f > 0.0 { "Infinity" } else { "-Infinity" }.into()
            } else {
                float_repr(*f)
            }
        }
        PyVal::Str(s) => esc_str(s, ascii),
        _ => return None,
    })
}

/// `json.dumps(v)`: separators `", "` and `": "`, `ensure_ascii=True`.
pub fn dumps(v: &PyVal) -> String {
    if let Some(s) = scalar(v, true) {
        return s;
    }
    match v {
        PyVal::List(items) => format!("[{}]", items.iter().map(dumps).collect::<Vec<_>>().join(", ")),
        PyVal::Dict(items) => format!(
            "{{{}}}",
            items.iter().map(|(k, v)| format!("{}: {}", esc_str(k, true), dumps(v))).collect::<Vec<_>>().join(", ")
        ),
        _ => unreachable!(),
    }
}

/// `json.dumps(v, ensure_ascii=False, indent=1)`
pub fn dumps_indent1(v: &PyVal) -> String {
    let mut out = String::new();
    indent_into(v, 0, &mut out);
    out
}

fn indent_into(v: &PyVal, level: usize, out: &mut String) {
    if let Some(s) = scalar(v, false) {
        out.push_str(&s);
        return;
    }
    let pad = |n: usize| " ".repeat(n);
    match v {
        PyVal::List(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                out.push_str(if i == 0 { "\n" } else { ",\n" });
                out.push_str(&pad(level + 1));
                indent_into(item, level + 1, out);
            }
            out.push('\n');
            out.push_str(&pad(level));
            out.push(']');
        }
        PyVal::Dict(items) => {
            if items.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, item)) in items.iter().enumerate() {
                out.push_str(if i == 0 { "\n" } else { ",\n" });
                out.push_str(&pad(level + 1));
                out.push_str(&esc_str(k, false));
                out.push_str(": ");
                indent_into(item, level + 1, out);
            }
            out.push('\n');
            out.push_str(&pad(level));
            out.push('}');
        }
        _ => unreachable!(),
    }
}

// ------------------------------------------------------------------ JSON-decoded values

/// Python truthiness of a `json.loads` value (a missing key is `None`).
pub fn truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

fn num_repr(n: &serde_json::Number) -> String {
    if let Some(i) = n.as_i64() {
        i.to_string()
    } else if let Some(u) = n.as_u64() {
        u.to_string()
    } else {
        float_repr(n.as_f64().unwrap_or(0.0))
    }
}

/// `str(value)` for a `json.loads` value.
pub fn py_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => py_repr(other),
    }
}

/// `repr(value)` for a `json.loads` value.
pub fn py_repr(v: &Value) -> String {
    match v {
        Value::Null => "None".into(),
        Value::Bool(b) => if *b { "True" } else { "False" }.into(),
        Value::Number(n) => num_repr(n),
        Value::String(s) => py_repr_str(s),
        Value::Array(a) => format!("[{}]", a.iter().map(py_repr).collect::<Vec<_>>().join(", ")),
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter().map(|(k, v)| format!("{}: {}", py_repr_str(k), py_repr(v))).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// `value or default` then used as a string: Some(str) only for a string value; a falsy
/// value yields the default; any other truthy value is None (the Python would raise on
/// the string method that follows).
pub fn str_or<'a>(v: Option<&'a Value>, default: &'a str) -> Option<&'a str> {
    if !truthy(v) {
        return Some(default);
    }
    match v {
        Some(Value::String(s)) => Some(s.as_str()),
        _ => None,
    }
}
