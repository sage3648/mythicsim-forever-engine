//! A bounded Forever simulation engine organized by shared systems and class/spec domains.
//!
//! Current support is prepared, single-target Frostbolt only. Character preparation,
//! complete Mage mechanics and production reports remain separate migration work.
//! The root re-exports preserve the original public API and prepared v1 JSON contract.

pub mod contracts;
pub mod report;

mod classes;
mod core;
mod engine;
mod mechanics;

pub use classes::mage::spells::frostbolt::hit_chance;
pub use contracts::{Caster, Request, Spell, Target};
pub use engine::simulate;
pub use report::{Counts, Report, TraceEvent, Work};

pub const SOURCE_REVISION: &str = "6823b49eb8aff741f197ef36d83766ef6a218285";
