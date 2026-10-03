//! The Rogue part of the prepared v2 build gate: Rogue effects, the effects of Rogue spells
//! and the auras Rogue effects claim. The shared gate is in `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassRogue",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits,
};

/// Rogue effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "adrenaline_rush",
    "ambush",
    "backstab",
    "blade_flurry",
    "cold_blood",
    "deadly_poison",
    "eviscerate",
    "instant_poison",
    "mutilate",
    "premeditation",
    "preparation",
    "rogue_finisher",
    "rogue_proc",
    "rupture",
    "sinister_strike",
    "slice_and_dice",
    "stealth",
    "thousand_cuts",
];

/// The effect kind whose implementation executes a Rogue spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    match spell.class_spell.as_deref()? {
        "sinister_strike" => Some("sinister_strike"),
        "backstab" => Some("backstab"),
        "eviscerate" => Some("eviscerate"),
        "slice_and_dice" => Some("slice_and_dice"),
        "blade_flurry" => Some("blade_flurry"),
        "adrenaline_rush" => Some("adrenaline_rush"),
        "instant_poison" => Some("instant_poison"),
        "deadly_poison" => Some("deadly_poison"),
        "ambush" => Some("ambush"),
        "rupture" => Some("rupture"),
        "mutilate" | "mutilate_hit" => Some("mutilate"),
        "cold_blood" => Some("cold_blood"),
        "premeditation" => Some("premeditation"),
        "preparation" => Some("preparation"),
        "stealth" | "vanish" => Some("stealth"),
        _ => None,
    }
}

/// Aura labels a Rogue effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::InstantPoison { trigger_aura, .. } | Effect::DeadlyPoison { trigger_aura, .. } => {
            vec![("player", trigger_aura)]
        }
        Effect::SliceAndDice { aura, .. }
        | Effect::BladeFlurry { aura, .. }
        | Effect::AdrenalineRush { aura, .. }
        | Effect::ColdBlood { aura, .. }
        | Effect::ThousandCuts { aura, .. } => vec![("player", aura)],
        Effect::RogueProc { trigger_aura, .. } => vec![("player", trigger_aura)],
        _ => Vec::new(),
    }
}

/// Rogue limits: finishers need the finisher effect, and every energy spell an energy bar.
fn limits(prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    let mut reasons = Vec::new();
    let finisher = prepared
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::RogueFinisher { .. }));
    for spell in reachable {
        let class = spell.class_spell.as_deref();
        if matches!(class, Some("eviscerate" | "slice_and_dice" | "rupture")) && !finisher {
            reasons.push(format!(
                "rotation reaches finisher {} without the finisher effect",
                spell.action_id.clone().unwrap_or_default()
            ));
        }
        let energy = spell
            .cost
            .as_ref()
            .is_some_and(|cost| cost.resource == "energy");
        if energy && prepared.player.energy.is_none() {
            reasons.push(format!(
                "rotation reaches {}, which costs energy the player lacks",
                spell.action_id.clone().unwrap_or_default()
            ));
        }
    }
    reasons
}
