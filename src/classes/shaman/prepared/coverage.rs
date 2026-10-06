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
    several_targets: Some(several_targets),
    other_target_casts: Some(crate::engine::coverage::no_limits),
};

/// Shaman effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "chain_lightning",
    "earth_shock",
    "elemental_devastation",
    "elemental_focus",
    "fire_nova",
    "flame_shock",
    "flametongue_totem",
    "flametongue_weapon",
    "flurry",
    "frost_shock",
    "frostbrand_weapon",
    "grace_of_air_totem",
    "improved_stormstrike",
    "lava_burst",
    "lightning_bolt",
    "lightning_shield",
    "magma_totem",
    "mana_spring_totem",
    "maelstrom_weapon",
    "rage_of_the_farseer",
    "rockbiter_weapon",
    "searing_totem",
    "stormstrike",
    "strength_of_earth_totem",
    "weapon_sync",
    "windfury_totem_self",
    "windfury_weapon",
];

/// Go `applyRageOfTheFarseer` registers its cast (client 425336) without a class mask.
const RAGE_OF_THE_FARSEER: i32 = 425336;
/// Go `registerLightningShieldSpell` registers its cast (client 10432, the highest rank)
/// without a class mask.
const LIGHTNING_SHIELD: i32 = 10432;

/// The effect kind whose implementation executes a Shaman spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    let id = spell.action_id.clone().unwrap_or_default();
    if spell.class_spell.is_none() && id.spell_id == RAGE_OF_THE_FARSEER && id.tag == 0 {
        return Some("rage_of_the_farseer");
    }
    if spell.class_spell.is_none() && id.spell_id == LIGHTNING_SHIELD && id.tag == 0 {
        return Some("lightning_shield");
    }
    match spell.class_spell.as_deref()? {
        "lightning_bolt" | "lightning_bolt_overload" => Some("lightning_bolt"),
        "chain_lightning" | "chain_lightning_overload" => Some("chain_lightning"),
        "flame_shock_direct" | "flame_shock_dot" => Some("flame_shock"),
        "lava_burst" => Some("lava_burst"),
        "fire_nova" => Some("fire_nova"),
        "searing_totem" => Some("searing_totem"),
        "magma_totem" => Some("magma_totem"),
        "flametongue_totem" => Some("flametongue_totem"),
        "earth_shock" => Some("earth_shock"),
        "frost_shock" => Some("frost_shock"),
        "stormstrike_cast" | "stormstrike_damage" => Some("stormstrike"),
        // Strength of Earth, Grace of Air and Mana Spring among the basic totems; `limits` rejects
        // the others.
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
        Effect::LightningShield { aura, .. } => vec![("player", aura)],
        Effect::WindfuryWeapon {
            trigger_aura,
            ap_aura,
            ..
        } => vec![("player", trigger_aura), ("player", ap_aura)],
        Effect::FlametongueTotem {
            aura, trigger_aura, ..
        } => vec![("player", aura), ("player", trigger_aura)],
        Effect::WindfuryTotemSelf {
            totem_aura,
            tracking_aura,
            dummy_aura,
            trigger_aura,
            proc_aura,
            ..
        } => vec![
            ("player", totem_aura),
            ("player", tracking_aura),
            ("player", dummy_aura),
            ("player", trigger_aura),
            ("player", proc_aura),
        ],
        _ => Vec::new(),
    }
}

/// Basic totems other than Strength of Earth, Grace of Air, Mana Spring and Windfury have no
/// behavior; a Grace of Air cast contests a party air totem's slot unless the air totem
/// category resolves it, a Windfury cast would contest a party air totem or a main hand
/// Windfury Weapon, and a Flametongue Totem would share the party totem's benefit, which the
/// runtime does not model.
fn limits(prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    let mut reasons = Vec::new();
    let mut known = Vec::new();
    // buffs/air_totem.go: with the slot exported as a category, the runtime resolves it.
    let air_totem_slot = prepared.effects.iter().any(|effect| {
        matches!(effect, Effect::ExclusiveCategory { category, .. } if category == "AirTotem")
    });
    for effect in &prepared.effects {
        match effect {
            Effect::StrengthOfEarthTotem { spell_id, .. } => known.push(*spell_id),
            Effect::GraceOfAirTotem { spell_id, .. } => known.push(*spell_id),
            Effect::ManaSpringTotem { spell_id, .. } => known.push(*spell_id),
            Effect::WindfuryTotemSelf { spell_id, .. } => known.push(*spell_id),
            _ => {}
        }
    }
    for spell in reachable {
        let Some(id) = spell.action_id.as_ref() else {
            continue;
        };
        if spell.class_spell.as_deref() == Some("basic_totem")
            && (!known.contains(&id.spell_id) || id.tag != 0)
        {
            reasons.push(format!(
                "rotation reaches {id}, a totem without a known behavior"
            ));
        }
        for effect in &prepared.effects {
            match effect {
                Effect::GraceOfAirTotem {
                    spell_id,
                    party_air_totem: true,
                    ..
                } if *spell_id == id.spell_id && id.tag == 0 && !air_totem_slot => reasons.push(format!(
                    "rotation reaches {id}, a Grace of Air Totem that contests a party air totem"
                )),
                Effect::WindfuryTotemSelf {
                    spell_id,
                    contested: true,
                    ..
                } if *spell_id == id.spell_id && id.tag == 0 => reasons.push(format!(
                    "rotation reaches {id}, a Windfury Totem that contests a party air totem or Windfury Weapon"
                )),
                Effect::FlametongueTotem {
                    spell_id,
                    party_totem: true,
                    ..
                } if *spell_id == id.spell_id && id.tag == 0 => reasons.push(format!(
                    "rotation reaches {id}, a Flametongue Totem beside the party's"
                )),
                _ => {}
            }
        }
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

/// The spells that reach a target past the first in Go and not yet in Rust: none. Chain
/// Lightning, Fire Nova and Magma Totem hit each target as in Go, and Searing Totem and the
/// shocks stay on the first target.
fn several_targets(_prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    crate::engine::coverage::spells_reaching_other_targets(reachable, &[])
}
