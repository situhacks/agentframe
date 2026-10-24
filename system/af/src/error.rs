//! Refusals and crashes, shaped like the Python's `die()` and uncaught exceptions.
//!
//! `die()` in af.py prints `af: ERROR: msg` to stderr *at the moment it is called* and
//! records the message for the run log, then raises SystemExit. Several doctor helpers
//! catch that SystemExit and carry on, so the print and the recorded message survive the
//! catch. `refuse()` reproduces both side effects at construction time; callers that
//! mirror an `except SystemExit` simply drop the `Refusal`.

use std::sync::Mutex;

static LAST_ERROR: Mutex<Option<String>> = Mutex::new(None);

/// The class of an exception the Python would not catch (a crash), or would catch only
/// under `except (UnicodeDecodeError, OSError)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashKind {
    Decode,
    Os,
    Other,
}

#[derive(Debug, Clone)]
pub enum AfError {
    /// `die(msg)`: exit 1, `refusal` in the run log.
    Refusal(String),
    /// An argparse usage error: exit 2, `refusal: usage`.
    Usage(String),
    /// An uncaught Python exception: exit 1, the run log records exit 2 and `crash`.
    Crash { kind: CrashKind, msg: String },
}

pub type R<T> = Result<T, AfError>;

impl AfError {
    pub fn is_refusal(&self) -> bool {
        matches!(self, AfError::Refusal(_))
    }
    pub fn is_decode_or_os(&self) -> bool {
        matches!(self, AfError::Crash { kind: CrashKind::Decode | CrashKind::Os, .. })
    }
}

impl std::fmt::Display for AfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AfError::Refusal(m) | AfError::Usage(m) => write!(f, "{m}"),
            AfError::Crash { msg, .. } => write!(f, "{msg}"),
        }
    }
}

/// `die(msg)`: print now, remember for the run log, return the error to propagate.
pub fn refuse(msg: impl Into<String>) -> AfError {
    let msg = msg.into();
    crate::out::err_line(&format!("af: ERROR: {msg}"));
    *LAST_ERROR.lock().unwrap() = Some(msg.clone());
    AfError::Refusal(msg)
}

pub fn crash(kind: CrashKind, msg: impl Into<String>) -> AfError {
    AfError::Crash { kind, msg: msg.into() }
}

pub fn last_error() -> Option<String> {
    LAST_ERROR.lock().unwrap().clone()
}

pub fn reset_last_error() {
    *LAST_ERROR.lock().unwrap() = None;
}

/// `return Err(refuse(format!(...)))`
#[macro_export]
macro_rules! die {
    ($($arg:tt)*) => {
        return Err($crate::error::refuse(format!($($arg)*)))
    };
}

/// Mirror `try: ... except SystemExit: <fallback>`: a refusal becomes `None`, anything else
/// propagates.
pub fn soft<T>(r: R<T>) -> R<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(AfError::Refusal(_)) => Ok(None),
        Err(e) => Err(e),
    }
}
