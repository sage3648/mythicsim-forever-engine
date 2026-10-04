//! The Mage part of the prepared v2 build gate: Mage effects, the effects of Mage spells
//! and the auras Mage effects claim. The shared gate is in `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassMage",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits: |_, _| Vec::new(),
};

/// Mage effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "arcane_blast",
    "arcane_concentration",
    "arcane_explosion",
    "arcane_missiles",
    "arcane_power",
    "blast_wave",
    "blizzard",
    "cold_snap",
    "combustion",
    "cone_of_cold",
    "evocation",
    "fingers_of_frost",
    "fire_blast",
    "fireball",
    "flamestrike",
    "frost_nova",
    "frostbolt",
    "frostfire_bolt",
    "heating_up",
    "ice_lance",
    "ignite",
    "mana_gems",
    "master_of_elements",
    "missile_barrage",
    "presence_of_mind",
    "pyroblast",
    "scorch",
    "winters_chill",
];

/// The effect kind whose implementation executes a Mage spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    match spell.class_spell.as_deref()? {
        "frostbolt" => Some("frostbolt"),
        "arcane_blast" => Some("arcane_blast"),
        "arcane_power" => Some("arcane_power"),
        "fire_blast" => Some("fire_blast"),
        "fireball" => Some("fireball"),
        "frostfire_bolt" => Some("frostfire_bolt"),
        "combustion" => Some("combustion"),
        "ignite" => Some("ignite"),
        "pyroblast" => Some("pyroblast"),
        "scorch" => Some("scorch"),
        "presence_of_mind" => Some("presence_of_mind"),
        "ice_lance" => Some("ice_lance"),
        "arcane_missiles_cast" => Some("arcane_missiles"),
        "cold_snap" => Some("cold_snap"),
        "evocation" => Some("evocation"),
        "mana_gem" => Some("mana_gems"),
        "arcane_explosion" => Some("arcane_explosion"),
        "cone_of_cold" => Some("cone_of_cold"),
        "frost_nova" => Some("frost_nova"),
        "blast_wave" => Some("blast_wave"),
        "flamestrike" => Some("flamestrike"),
        "blizzard" | "improved_blizzard" => Some("blizzard"),
        _ => None,
    }
}

/// Aura labels a Mage effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::ArcaneConcentration {
            trigger_aura, aura, ..
        }
        | Effect::MissileBarrage {
            trigger_aura, aura, ..
        }
        | Effect::HeatingUp {
            trigger_aura, aura, ..
        }
        | Effect::FingersOfFrost {
            trigger_aura, aura, ..
        }
        | Effect::WintersChill {
            trigger_aura, aura, ..
        } => {
            vec![("player", trigger_aura), ("player", aura)]
        }
        Effect::Evocation {
            regen_aura,
            channel_aura,
            ..
        } => {
            vec![("player", regen_aura), ("player", channel_aura)]
        }
        Effect::Ignite { trigger_aura, .. } | Effect::MasterOfElements { trigger_aura, .. } => {
            vec![("player", trigger_aura)]
        }
        Effect::MageArmor { aura }
        | Effect::ArcaneBlast { aura, .. }
        | Effect::ArcanePower { aura, .. }
        | Effect::Combustion { aura, .. }
        | Effect::PresenceOfMind { aura, .. } => vec![("player", aura)],
        _ => Vec::new(),
    }
}
