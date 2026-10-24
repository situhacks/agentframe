//! stdout/stderr the way Python's text streams write them.
//!
//! On Windows `sys.stdout`/`sys.stderr` translate every "\n" to "\r\n" (the af.py
//! `reconfigure(encoding="utf-8")` changes the encoding, not the newline mode), so the
//! port does the same translation on Windows and none elsewhere.

use std::io::Write;

pub const NL: &str = if cfg!(windows) { "\r\n" } else { "\n" };

fn translate(text: &str) -> String {
    if cfg!(windows) {
        text.replace('\n', "\r\n")
    } else {
        text.to_string()
    }
}

/// `print(text)`
pub fn line(text: &str) {
    let mut s = translate(text);
    s.push_str(NL);
    // print! (not a raw handle write) so `cargo test` captures in-process output.
    print!("{s}");
    let _ = std::io::stdout().flush();
}

/// `print(text, file=sys.stderr)`
pub fn err_line(text: &str) {
    let mut s = translate(text);
    s.push_str(NL);
    eprint!("{s}");
}

#[macro_export]
macro_rules! outln {
    ($($arg:tt)*) => {
        $crate::out::line(&format!($($arg)*))
    };
}
