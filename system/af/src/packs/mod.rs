//! Domain pack rules, compiled in. A pack's rules are selected by its folder name; a
//! pack with no compiled-in rules runs on the generic spine alone.

pub mod careers;
pub mod marketing;

use crate::core::Ctx;
use crate::error::R;
use crate::pyos;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rules {
    Marketing,
    Careers,
}

/// The compiled-in rules for the pack at `pack_dir`, if it has any.
pub fn load_rules(_ctx: &Ctx, pack_dir: Option<&str>) -> R<Option<Rules>> {
    Ok(match pack_dir.map(pyos::basename).as_deref() {
        Some("marketing") => Some(Rules::Marketing),
        Some("careers") => Some(Rules::Careers),
        _ => None,
    })
}
