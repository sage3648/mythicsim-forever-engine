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
const EFFECTS: &[&str] = &[
    "consecration",
    "divine_favor",
    "hammer_of_wrath",
    "holy_shock",
    "holy_strike",
    "judgement",
    "judgement_refresh",
    "sacred_arbiter",
    "sanctified_judgement",
    "seal_of_command",
    "seal_of_righteousness",
    "twist_of_light",
    "vengeance",
    "vindication",
];

/// The effect kind whose implementation executes a Paladin spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    match spell.class_spell.as_deref()? {
        "seal_of_command" | "seal_of_command_proc" | "judgement_of_command" => {
            Some("seal_of_command")
        }
        "seal_of_righteousness" | "seal_of_righteousness_proc" | "judgement_of_righteousness" => {
            Some("seal_of_righteousness")
        }
        "judgement" => Some("judgement"),
        "holy_strike" => Some("holy_strike"),
        "hammer_of_wrath" => Some("hammer_of_wrath"),
        "consecration" => Some("consecration"),
        "holy_shock" => Some("holy_shock"),
        "divine_favor" => Some("divine_favor"),
        _ => None,
    }
}

/// Aura labels a Paladin effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::JudgementRefresh { trigger_aura, .. }
        | Effect::Vindication { trigger_aura, .. }
        | Effect::SanctifiedJudgement { trigger_aura, .. }
        | Effect::SacredArbiter { trigger_aura, .. }
        | Effect::TwistOfLight { trigger_aura, .. }
        | Effect::DivineFavor {
            aura: trigger_aura, ..
        } => vec![("player", trigger_aura)],
        Effect::Vengeance {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("player", aura)],
        Effect::SealOfCommand { ranks, .. } => ranks
            .iter()
            .map(|rank| ("player", rank.aura.as_str()))
            .collect(),
        Effect::SealOfRighteousness { ranks, .. } => ranks
            .iter()
            .map(|rank| ("player", rank.aura.as_str()))
            .collect(),
        _ => Vec::new(),
    }
}
