//! The Shaman part of the prepared v2 build gate: Shaman effects, the effects of Shaman
//! spells and the auras Shaman effects claim. The shared gate is in `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassShaman",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits: |_, _| Vec::new(),
};

/// Shaman effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "chain_lightning",
    "elemental_focus",
    "fire_nova",
    "flame_shock",
    "lava_burst",
    "lightning_bolt",
    "searing_totem",
];

/// The effect kind whose implementation executes a Shaman spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    match spell.class_spell.as_deref()? {
        "lightning_bolt" | "lightning_bolt_overload" => Some("lightning_bolt"),
        "chain_lightning" | "chain_lightning_overload" => Some("chain_lightning"),
        "flame_shock_direct" | "flame_shock_dot" => Some("flame_shock"),
        "lava_burst" => Some("lava_burst"),
        "fire_nova" => Some("fire_nova"),
        "searing_totem" => Some("searing_totem"),
        _ => None,
    }
}

/// Aura labels a Shaman effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::ElementalFocus {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("player", aura)],
        _ => Vec::new(),
    }
}
