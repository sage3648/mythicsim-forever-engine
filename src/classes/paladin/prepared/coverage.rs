//! The Paladin part of the prepared v2 build gate. The shared gate is in
//! `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassPaladin",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits: |_, _| Vec::new(),
};

/// Paladin effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &["judgement_refresh"];

/// The effect kind whose implementation executes a Paladin spell.
fn spell_capability(_spell: &Spell) -> Option<&'static str> {
    None
}

/// Aura labels a Paladin effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::JudgementRefresh { trigger_aura, .. } => vec![("player", trigger_aura)],
        _ => Vec::new(),
    }
}
