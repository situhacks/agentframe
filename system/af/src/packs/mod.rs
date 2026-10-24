//! Domain pack rules, compiled in.
//!
//! af.py imports `library/domains/{domain}/rules.py` when the file exists and calls the
//! hooks it defines. The Rust host keeps the same switch: a pack's compiled-in rules are
//! active only while that pack directory ships `rules.py`. A pack shipping a rules.py
//! the port has no counterpart for is refused loudly, the way a rules.py that fails to
//! import is refused by the Python. Cutover replaces the rules.py check (see PORT-STATUS.md).

pub mod careers;
pub mod marketing;

use crate::core::Ctx;
use crate::die;
use crate::error::R;
use crate::pyos;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rules {
    Marketing,
    Careers,
}

/// `load_rules(pack_dir)`
pub fn load_rules(ctx: &Ctx, pack_dir: Option<&str>) -> R<Option<Rules>> {
    let pack_dir = match pack_dir {
        Some(p) => p,
        None => return Ok(None),
    };
    let rp = pyos::join(pack_dir, "rules.py");
    if !pyos::isfile(&rp) {
        return Ok(None);
    }
    match pyos::basename(pack_dir).as_str() {
        "marketing" => Ok(Some(Rules::Marketing)),
        "careers" => Ok(Some(Rules::Careers)),
        _ => die!(
            "domain rules module failed to load ({}): the Rust host has no compiled-in port of this pack's rules; add it under system/af/src/packs/",
            ctx.rel(&rp)?
        ),
    }
}
