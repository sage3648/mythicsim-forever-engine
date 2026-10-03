//! The Warrior part of the prepared v2 build gate: Warrior effects, the effects of Warrior
//! spells and the auras Warrior effects claim. The shared gate is in `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassWarrior",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits,
};

/// Warrior effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "anger_management",
    "berserker_rage",
    "bloodrage",
    "bloodthirst",
    "death_wish",
    "deep_wounds",
    "execute",
    "flurry",
    "hamstring",
    "heroic_strike_queue",
    "overpower_window",
    "recklessness",
    "sunder_armor",
    "unbridled_wrath",
    "warrior_stances",
    "whirlwind",
];

/// The effect kind whose implementation executes a Warrior spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    // Bloodrage and the strikes' queue casts carry no class mask; the shared gate names them
    // by their effects' spells.
    match spell.class_spell.as_deref()? {
        "bloodthirst" => Some("bloodthirst"),
        "whirlwind" | "whirlwind_off_hand" => Some("whirlwind"),
        "execute" => Some("execute"),
        "hamstring" => Some("hamstring"),
        "berserker_rage" => Some("berserker_rage"),
        "death_wish" => Some("death_wish"),
        "recklessness" => Some("recklessness"),
        "sunder_armor" => Some("sunder_armor"),
        "deep_wounds" => Some("deep_wounds"),
        "heroic_strike" | "cleave" => Some("heroic_strike_queue"),
        "battle_stance" | "berserker_stance" | "defensive_stance" | "retaliation"
        | "shield_wall" => Some("warrior_stances"),
        _ => None,
    }
}

/// Aura labels a Warrior effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::DeepWounds { trigger_aura, .. }
        | Effect::UnbridledWrath { trigger_aura, .. }
        | Effect::OverpowerWindow { trigger_aura, .. } => vec![("player", trigger_aura)],
        Effect::Flurry {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("player", aura)],
        _ => Vec::new(),
    }
}

/// The stance each stance-locked spell needs, from Go's `ExtraCastCondition`s: retaliation.go
/// and shield_wall.go. Reaching one in the stance it needs is a stance the runtime lacks.
fn stance_spell(class_spell: &str) -> Option<&'static str> {
    match class_spell {
        "battle_stance" | "retaliation" => Some("battle"),
        "berserker_stance" => Some("berserker"),
        "defensive_stance" | "shield_wall" => Some("defensive"),
        _ => None,
    }
}

/// Stance changes and the warrior's own Sunder Armor stacks are not implemented.
fn limits(prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    let default_stance = prepared.effects.iter().find_map(|effect| match effect {
        Effect::WarriorStances { default_stance, .. } => Some(default_stance.as_str()),
        _ => None,
    });
    let sunder_blocked = prepared
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::SunderArmor { blocked: true, .. }));
    let mut reasons = Vec::new();
    for spell in reachable {
        let id = spell.action_id.clone().unwrap_or_default();
        let Some(class_spell) = spell.class_spell.as_deref() else {
            continue;
        };
        match class_spell {
            "battle_stance" | "berserker_stance" | "defensive_stance"
                if stance_spell(class_spell) != default_stance =>
            {
                reasons.push(format!("rotation reaches {id}, a stance change"));
            }
            "retaliation" | "shield_wall" if stance_spell(class_spell) == default_stance => {
                reasons.push(format!("rotation reaches {id} in the stance it needs"));
            }
            "sunder_armor" if !sunder_blocked => {
                reasons.push(format!(
                    "rotation reaches {id}, which stacks the warrior's own Sunder Armor"
                ));
            }
            _ => {}
        }
    }
    reasons.sort();
    reasons.dedup();
    reasons
}
