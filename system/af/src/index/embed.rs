//! The local embedding backend: Ollama's /api/embed (`embed_texts`), with failures named
//! the way urllib/json raise them so the degraded-mode messages read the same.

use std::io::Read;
use std::time::Duration;

pub const OLLAMA_URL: &str = "http://127.0.0.1:11434/api/embed";
pub const EMBED_MODEL: &str = "qwen3-embedding:0.6b";
pub const EMBED_BATCH: usize = 32;
pub const EMBED_MAX_CHARS: usize = 6000;

/// A failed embedding call: the Python exception's class name and `str(exc)`.
#[derive(Debug, Clone)]
pub struct EmbedErr {
    pub class: String,
    pub msg: String,
}

impl EmbedErr {
    pub fn new(class: &str, msg: impl Into<String>) -> EmbedErr {
        EmbedErr { class: class.to_string(), msg: msg.into() }
    }
}

/// A batch embedder: texts in, one vector per text out.
pub type Embedder<'a> = &'a dyn Fn(&[String]) -> Result<Vec<Vec<f64>>, EmbedErr>;

/// The endpoint. `AF_OLLAMA_URL` overrides it (tests point it at a dead port to drive the
/// keyword-only path); the Python has no override.
pub fn ollama_url() -> String {
    match std::env::var("AF_OLLAMA_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => OLLAMA_URL.to_string(),
    }
}

/// `str(OSError)` on Windows: `[WinError N] message` with the trailing period dropped.
fn os_error_text(e: &std::io::Error) -> String {
    let full = e.to_string();
    let msg = match full.rfind(" (os error ") {
        Some(i) => &full[..i],
        None => full.as_str(),
    };
    let msg = msg.trim_end_matches(|c: char| c <= ' ' || c == '.');
    match e.raw_os_error() {
        Some(code) if cfg!(windows) => format!("[WinError {code}] {msg}"),
        Some(code) => format!("[Errno {code}] {msg}"),
        None => msg.to_string(),
    }
}

fn find_io_error<'a>(e: &'a (dyn std::error::Error + 'static)) -> Option<&'a std::io::Error> {
    let mut cur: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(c) = cur {
        if let Some(io) = c.downcast_ref::<std::io::Error>() {
            return Some(io);
        }
        cur = c.source();
    }
    None
}

/// `embed_texts(texts)`: local-only embeddings; Err so callers can degrade to keyword-only.
pub fn embed_texts(texts: &[String]) -> Result<Vec<Vec<f64>>, EmbedErr> {
    let body = serde_json::json!({ "model": EMBED_MODEL, "input": texts }).to_string();
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(300)).build();
    let resp = agent.post(&ollama_url()).set("Content-Type", "application/json").send_string(&body);
    let resp = match resp {
        Ok(r) => r,
        Err(ureq::Error::Status(code, r)) => {
            return Err(EmbedErr::new("HTTPError", format!("HTTP Error {code}: {}", r.status_text())));
        }
        Err(ureq::Error::Transport(t)) => {
            let reason = match std::error::Error::source(&t).and_then(find_io_error) {
                Some(io) if io.kind() == std::io::ErrorKind::TimedOut => "timed out".to_string(),
                Some(io) => os_error_text(io),
                None => t.message().map(|m| m.to_string()).unwrap_or_else(|| t.kind().to_string()),
            };
            return Err(EmbedErr::new("URLError", format!("<urlopen error {reason}>")));
        }
    };
    let mut raw = Vec::new();
    if let Err(e) = resp.into_reader().read_to_end(&mut raw) {
        if e.kind() == std::io::ErrorKind::TimedOut {
            return Err(EmbedErr::new("TimeoutError", "timed out"));
        }
        return Err(EmbedErr::new("OSError", os_error_text(&e)));
    }
    let text = String::from_utf8(raw).map_err(|_| EmbedErr::new("UnicodeDecodeError", "'utf-8' codec can't decode the response"))?;
    let data = Json::parse(&text)?;
    let obj = match &data {
        Json::Obj(kv) => kv,
        other => {
            return Err(EmbedErr::new("AttributeError", format!("'{}' object has no attribute 'get'", other.py_type())));
        }
    };
    // `data.get("embeddings") or []`: the last duplicate key wins; a falsy value is [].
    let embs = obj.iter().rev().find(|(k, _)| k == "embeddings").map(|(_, v)| v);
    let empty = Vec::new();
    let list: &Vec<Json> = match embs {
        Some(Json::Arr(a)) => a,
        Some(v) if !v.truthy() => &empty,
        Some(v) => return Err(EmbedErr::new("TypeError", format!("object of type '{}' has no len()", v.py_type()))),
        None => &empty,
    };
    if list.len() != texts.len() {
        return Err(EmbedErr::new(
            "RuntimeError",
            format!("embedding backend returned {} vectors for {} inputs", list.len(), texts.len()),
        ));
    }
    let mut out = Vec::with_capacity(list.len());
    for v in list {
        let arr = match v {
            Json::Arr(a) => a,
            other => return Err(EmbedErr::new("TypeError", format!("object of type '{}' has no len()", other.py_type()))),
        };
        let mut vec = Vec::with_capacity(arr.len());
        for x in arr {
            match x {
                Json::Num(f) => vec.push(*f),
                other => {
                    let t = other.py_type();
                    return Err(EmbedErr::new("TypeError", format!("unsupported operand type(s) for *: '{t}' and '{t}'")));
                }
            }
        }
        out.push(vec);
    }
    Ok(out)
}

/// A minimal JSON value. Numbers parse with Rust's correctly rounded `str::parse::<f64>`,
/// matching Python's `float()`; serde_json's default float path can be one ulp off.
#[derive(Debug, Clone)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn py_type(&self) -> &'static str {
        match self {
            Json::Null => "NoneType",
            Json::Bool(_) => "bool",
            Json::Num(_) => "float",
            Json::Str(_) => "str",
            Json::Arr(_) => "list",
            Json::Obj(_) => "dict",
        }
    }

    fn truthy(&self) -> bool {
        match self {
            Json::Null => false,
            Json::Bool(b) => *b,
            Json::Num(f) => *f != 0.0,
            Json::Str(s) => !s.is_empty(),
            Json::Arr(a) => !a.is_empty(),
            Json::Obj(o) => !o.is_empty(),
        }
    }

    pub fn parse(text: &str) -> Result<Json, EmbedErr> {
        let b = text.as_bytes();
        let mut p = 0usize;
        let v = parse_value(b, &mut p)?;
        skip_ws(b, &mut p);
        if p != b.len() {
            return Err(json_err("Extra data", p));
        }
        Ok(v)
    }
}

fn json_err(what: &str, pos: usize) -> EmbedErr {
    EmbedErr::new("JSONDecodeError", format!("{what}: char {pos}"))
}

fn skip_ws(b: &[u8], p: &mut usize) {
    while *p < b.len() && matches!(b[*p], b' ' | b'\t' | b'\n' | b'\r') {
        *p += 1;
    }
}

fn parse_value(b: &[u8], p: &mut usize) -> Result<Json, EmbedErr> {
    skip_ws(b, p);
    if *p >= b.len() {
        return Err(json_err("Expecting value", *p));
    }
    match b[*p] {
        b'{' => {
            *p += 1;
            let mut kv = Vec::new();
            skip_ws(b, p);
            if *p < b.len() && b[*p] == b'}' {
                *p += 1;
                return Ok(Json::Obj(kv));
            }
            loop {
                skip_ws(b, p);
                if *p >= b.len() || b[*p] != b'"' {
                    return Err(json_err("Expecting property name enclosed in double quotes", *p));
                }
                let k = parse_string(b, p)?;
                skip_ws(b, p);
                if *p >= b.len() || b[*p] != b':' {
                    return Err(json_err("Expecting ':' delimiter", *p));
                }
                *p += 1;
                let v = parse_value(b, p)?;
                kv.push((k, v));
                skip_ws(b, p);
                match b.get(*p) {
                    Some(b',') => *p += 1,
                    Some(b'}') => {
                        *p += 1;
                        return Ok(Json::Obj(kv));
                    }
                    _ => return Err(json_err("Expecting ',' delimiter", *p)),
                }
            }
        }
        b'[' => {
            *p += 1;
            let mut items = Vec::new();
            skip_ws(b, p);
            if *p < b.len() && b[*p] == b']' {
                *p += 1;
                return Ok(Json::Arr(items));
            }
            loop {
                items.push(parse_value(b, p)?);
                skip_ws(b, p);
                match b.get(*p) {
                    Some(b',') => *p += 1,
                    Some(b']') => {
                        *p += 1;
                        return Ok(Json::Arr(items));
                    }
                    _ => return Err(json_err("Expecting ',' delimiter", *p)),
                }
            }
        }
        b'"' => Ok(Json::Str(parse_string(b, p)?)),
        b't' if b[*p..].starts_with(b"true") => {
            *p += 4;
            Ok(Json::Bool(true))
        }
        b'f' if b[*p..].starts_with(b"false") => {
            *p += 5;
            Ok(Json::Bool(false))
        }
        b'n' if b[*p..].starts_with(b"null") => {
            *p += 4;
            Ok(Json::Null)
        }
        b'N' if b[*p..].starts_with(b"NaN") => {
            *p += 3;
            Ok(Json::Num(f64::NAN))
        }
        b'I' if b[*p..].starts_with(b"Infinity") => {
            *p += 8;
            Ok(Json::Num(f64::INFINITY))
        }
        b'-' if b[*p..].starts_with(b"-Infinity") => {
            *p += 9;
            Ok(Json::Num(f64::NEG_INFINITY))
        }
        b'-' | b'0'..=b'9' => {
            let start = *p;
            if b[*p] == b'-' {
                *p += 1;
            }
            while *p < b.len() && (b[*p].is_ascii_digit() || matches!(b[*p], b'.' | b'e' | b'E' | b'+' | b'-')) {
                *p += 1;
            }
            let s = std::str::from_utf8(&b[start..*p]).unwrap();
            s.parse::<f64>().map(Json::Num).map_err(|_| json_err("Expecting value", start))
        }
        _ => Err(json_err("Expecting value", *p)),
    }
}

fn parse_string(b: &[u8], p: &mut usize) -> Result<String, EmbedErr> {
    let start = *p;
    *p += 1; // opening quote
    let mut out: Vec<u16> = Vec::new();
    let mut bytes: Vec<u8> = Vec::new();
    let flush = |bytes: &mut Vec<u8>, out: &mut Vec<u16>| {
        if !bytes.is_empty() {
            out.extend(String::from_utf8_lossy(bytes).encode_utf16());
            bytes.clear();
        }
    };
    while *p < b.len() {
        let c = b[*p];
        match c {
            b'"' => {
                *p += 1;
                flush(&mut bytes, &mut out);
                return Ok(String::from_utf16_lossy(&out));
            }
            b'\\' => {
                flush(&mut bytes, &mut out);
                let e = *b.get(*p + 1).ok_or_else(|| json_err("Unterminated string starting at", start))?;
                *p += 2;
                match e {
                    b'"' => out.push(b'"' as u16),
                    b'\\' => out.push(b'\\' as u16),
                    b'/' => out.push(b'/' as u16),
                    b'b' => out.push(8),
                    b'f' => out.push(12),
                    b'n' => out.push(10),
                    b'r' => out.push(13),
                    b't' => out.push(9),
                    b'u' => {
                        let hex = b.get(*p..*p + 4).ok_or_else(|| json_err("Invalid \\uXXXX escape", *p))?;
                        let h = std::str::from_utf8(hex).ok().and_then(|h| u16::from_str_radix(h, 16).ok());
                        out.push(h.ok_or_else(|| json_err("Invalid \\uXXXX escape", *p))?);
                        *p += 4;
                    }
                    _ => return Err(json_err("Invalid \\escape", *p - 2)),
                }
            }
            _ => {
                bytes.push(c);
                *p += 1;
            }
        }
    }
    Err(json_err("Unterminated string starting at", start))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_an_ollama_reply() {
        let j = Json::parse(r#"{"model":"m","embeddings":[[0.0049596005,-1e-3,2],[1.5]],"n":null,"s":"aé\"b"}"#).unwrap();
        match j {
            Json::Obj(kv) => {
                assert_eq!(kv[1].0, "embeddings");
                match &kv[1].1 {
                    Json::Arr(a) => match &a[0] {
                        Json::Arr(v) => {
                            assert!(matches!(v[0], Json::Num(x) if x == 0.0049596005));
                            assert!(matches!(v[1], Json::Num(x) if x == -0.001));
                        }
                        _ => panic!(),
                    },
                    _ => panic!(),
                }
                assert!(matches!(&kv[3].1, Json::Str(s) if s == "a\u{e9}\"b"));
            }
            _ => panic!(),
        }
        assert!(Json::parse("[1,]").is_err());
    }

    #[test]
    fn a_dead_port_is_a_urlerror() {
        std::env::set_var("AF_OLLAMA_URL", "http://127.0.0.1:9/api/embed");
        let e = embed_texts(&["x".to_string()]).unwrap_err();
        std::env::remove_var("AF_OLLAMA_URL");
        assert_eq!(e.class, "URLError");
        assert!(e.msg.starts_with("<urlopen error "), "{}", e.msg);
    }
}
