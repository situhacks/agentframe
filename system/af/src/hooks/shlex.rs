//! `shlex.split(command, posix=True)` as CPython 3.12 tokenizes it (whitespace_split,
//! no comments), with the guards' fallback to `str.split()` on a ValueError.

use crate::pyos::py_split_ws;

const WHITESPACE: [char; 4] = [' ', '\t', '\r', '\n'];

fn is_ws(c: char) -> bool {
    WHITESPACE.contains(&c)
}

fn is_quote(c: char) -> bool {
    c == '\'' || c == '"'
}

/// Err(()) is the ValueError ("No closing quotation" / "No escaped character").
pub fn split_posix(s: &str) -> Result<Vec<String>, ()> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    loop {
        // read_token
        let mut token = String::new();
        let mut quoted = false;
        let mut state: char = ' ';
        let mut escapedstate: char = ' ';
        let mut eof = false;
        loop {
            let next = chars.get(i).copied();
            i += 1;
            match state {
                ' ' => match next {
                    None => {
                        eof = true;
                        break;
                    }
                    Some(c) if is_ws(c) => {
                        if !token.is_empty() || quoted {
                            break;
                        }
                    }
                    Some('\\') => {
                        escapedstate = 'a';
                        state = '\\';
                    }
                    Some(c) if is_quote(c) => state = c,
                    Some(c) => {
                        token.push(c);
                        state = 'a';
                    }
                },
                '\'' | '"' => {
                    quoted = true;
                    match next {
                        None => return Err(()),
                        Some(c) if c == state => state = 'a',
                        Some('\\') if state == '"' => {
                            escapedstate = state;
                            state = '\\';
                        }
                        Some(c) => token.push(c),
                    }
                }
                '\\' => {
                    let c = match next {
                        None => return Err(()),
                        Some(c) => c,
                    };
                    if is_quote(escapedstate) && c != state && c != escapedstate {
                        token.push(state);
                    }
                    token.push(c);
                    state = escapedstate;
                }
                _ => match next {
                    // 'a'
                    None => {
                        eof = true;
                        break;
                    }
                    Some(c) if is_ws(c) => {
                        state = ' ';
                        if !token.is_empty() || quoted {
                            break;
                        }
                    }
                    Some(c) if is_quote(c) => state = c,
                    Some('\\') => {
                        escapedstate = 'a';
                        state = '\\';
                    }
                    Some(c) => token.push(c),
                },
            }
        }
        if token.is_empty() && !quoted {
            // posix: an unquoted empty token is the end of input
            if eof {
                break;
            }
            continue;
        }
        out.push(token);
        if eof {
            break;
        }
    }
    Ok(out)
}

/// The guards' `_tokens()`.
pub fn tokens(command: &str) -> Vec<String> {
    match split_posix(command) {
        Ok(t) => t,
        Err(()) => py_split_ws(command).into_iter().map(|s| s.to_string()).collect(),
    }
}
