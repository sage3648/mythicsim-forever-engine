//! The Shaman part of the prepared v2 build gate: Shaman effects, the effects of Shaman
//! spells and the auras Shaman effects claim. The shared gate is in `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassShaman",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits,
};

/// Shaman effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "chain_lightning",
    "earth_shock",
    "elemental_devastation",
    "elemental_focus",
    "fire_nova",
    "flame_shock",
    "flametongue_weapon",
    "flurry",
    "frost_shock",
    "frostbrand_weapon",
    "improved_stormstrike",
    "lava_burst",
    "lightning_bolt",
    "maelstrom_weapon",
    "rage_of_the_farseer",
    "rockbiter_weapon",
    "searing_totem",
    "stormstrike",
    "strength_of_earth_totem",
];

/// Go `applyRageOfTheFarseer` registers its cast (client 425336) without a class mask.
const RAGE_OF_THE_FARSEER: i32 = 425336;

/// The effect kind whose implementation executes a Shaman spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    let id = spell.action_id.clone().unwrap_or_default();
    if spell.class_spell.is_none() && id.spell_id == RAGE_OF_THE_FARSEER && id.tag == 0 {
        return Some("rage_of_the_farseer");
    }
    match spell.class_spell.as_deref()? {
        "lightning_bolt" | "lightning_bolt_overload" => Some("lightning_bolt"),
        "chain_lightning" | "chain_lightning_overload" => Some("chain_lightning"),
        "flame_shock_direct" | "flame_shock_dot" => Some("flame_shock"),
        "lava_burst" => Some("lava_burst"),
        "fire_nova" => Some("fire_nova"),
        "searing_totem" => Some("searing_totem"),
        "earth_shock" => Some("earth_shock"),
        "frost_shock" => Some("frost_shock"),
        "stormstrike_cast" | "stormstrike_damage" => Some("stormstrike"),
        // Only Strength of Earth Totem among the basic totems; `limits` rejects the others.
        "basic_totem" => Some("strength_of_earth_totem"),
        _ => None,
    }
}

/// Aura labels a Shaman effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::ElementalFocus {
            trigger_aura, aura, ..
        }
        | Effect::ElementalDevastation {
            trigger_aura, aura, ..
        }
        | Effect::MaelstromWeapon {
            trigger_aura, aura, ..
        }
        | Effect::Flurry {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("player", aura)],
        Effect::RageOfTheFarseer { aura, .. }
        | Effect::RockbiterWeapon { aura, .. }
        | Effect::FrostbrandWeapon {
            trigger_aura: aura, ..
        } => vec![("player", aura)],
        Effect::FlametongueWeapon { hands } => hands
            .iter()
            .map(|hand| ("player", hand.trigger_aura.as_str()))
            .collect(),
        // The reset hears only hits the player takes, which never happen in scope.
        Effect::ImprovedStormstrike {
            trigger_aura,
            aura,
            reset_aura,
            ..
        } => vec![
            ("player", trigger_aura),
            ("player", aura),
            ("player", reset_aura),
        ],
        Effect::Stormstrike { aura, .. } => vec![("target", aura)],
        _ => Vec::new(),
    }
}

/// Basic totems other than Strength of Earth Totem have no behavior.
fn limits(prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    let earth = prepared.effects.iter().find_map(|effect| match effect {
        Effect::StrengthOfEarthTotem { spell_id, .. } => Some(*spell_id),
        _ => None,
    });
    reachable
        .iter()
        .filter(|spell| spell.class_spell.as_deref() == Some("basic_totem"))
        .filter_map(|spell| spell.action_id.as_ref())
        .filter(|id| Some(id.spell_id) != earth || id.tag != 0)
        .map(|id| format!("rotation reaches {id}, a totem without a known behavior"))
        .collect()
}
