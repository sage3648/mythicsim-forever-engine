//! A bounded Forever simulation engine organized by shared systems and class/spec domains.
//!
//! Current simulation support is prepared, single-target Frostbolt only. The prepared v2
//! contract describes complete reset Go simulations; [`check_prepared`] reports which
//! mechanics an input still needs. Character preparation, complete Mage mechanics and
//! production reports remain separate migration work. The root re-exports preserve the
//! original public API and prepared v1 JSON contract.

pub mod contracts;
pub mod report;

mod classes;
mod core;
mod engine;
mod mechanics;
mod rotation;

pub use classes::mage::spells::frostbolt::hit_chance;
pub use contracts::{Caster, Request, Spell, Target};
pub use engine::prepared::{
    check as check_prepared, coverage as prepared_coverage, simulate as simulate_prepared,
    PreparedError, PreparedReport, CLIENT_BUILD,
};
pub use engine::simulate;

/// Prepared v2 effect kinds this engine executes. The release manifest must agree.
pub fn implemented_prepared_effects() -> &'static [&'static str] {
    classes::mage::specs::frost::IMPLEMENTED_EFFECTS
}
pub use report::{Counts, Report, TraceEvent, Work};

/// The pinned Go reference revision, from upstream/sources.json.
pub const SOURCE_REVISION: &str = env!("FOREVER_REFERENCE_REVISION");
