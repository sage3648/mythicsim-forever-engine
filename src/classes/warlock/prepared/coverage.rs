//! The Warlock part of the prepared v2 build gate: Warlock effects, the effects of Warlock
//! spells and the auras Warlock effects claim. The shared gate is in `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassWarlock",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits,
};

/// Warlock effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "amplify_curse",
    "bane_of_agony",
    "conflagrate",
    "corruption",
    "curse_of_the_elements",
    "immolate",
    "improved_shadow_bolt",
    "lash_of_pain",
    "life_tap",
    "nightfall",
    "searing_pain",
    "shadow_and_flame",
    "shadow_bolt",
    "shadowburn",
    "soul_fire",
    "warlock_pet",
];

/// The effect kind whose implementation executes a Warlock spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    let damage = spell.damage_effect.is_some();
    let dot = spell.dot.is_some();
    match spell.class_spell.as_deref()? {
        "shadow_bolt" if damage => Some("shadow_bolt"),
        "immolate" if damage && spell.related_dot_spell.is_some() => Some("immolate"),
        "corruption" if dot => Some("corruption"),
        "bane_of_agony" if dot => Some("bane_of_agony"),
        "curse_of_the_elements" => Some("curse_of_the_elements"),
        "life_tap" => Some("life_tap"),
        "conflagrate" if damage => Some("conflagrate"),
        "shadowburn" if damage => Some("shadowburn"),
        "searing_pain" if damage => Some("searing_pain"),
        "soul_fire" if damage => Some("soul_fire"),
        "amplify_curse" => Some("amplify_curse"),
        "succubus_lash_of_pain" => Some("lash_of_pain"),
        _ => None,
    }
}

/// Aura labels a Warlock effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::ImprovedShadowBolt {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("target", aura)],
        Effect::ShadowAndFlame {
            trigger_aura,
            shadow_aura,
            fire_aura,
            ..
        } => vec![
            ("player", trigger_aura),
            ("player", shadow_aura),
            ("player", fire_aura),
        ],
        Effect::CurseOfTheElements { aura, .. } => vec![("target", aura)],
        Effect::Nightfall {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("player", aura)],
        Effect::WarlockPet { pet, .. } => vec![("pet unit", pet)],
        _ => Vec::new(),
    }
}

/// The demon's abilities, which its AI reaches, need a behavior too.
fn limits(prepared: &PreparedV2, _reachable: &[&Spell]) -> Vec<String> {
    let mut reasons = Vec::new();
    for effect in &prepared.effects {
        let Effect::WarlockPet {
            pet,
            autocast_spells,
            ..
        } = effect
        else {
            continue;
        };
        let Some(exported) = prepared.pets.iter().find(|exported| exported.label == *pet) else {
            reasons.push(format!("the demon {pet:?} is not simulated"));
            continue;
        };
        for &position in autocast_spells {
            let spell = exported.spells.get(position);
            let known = spell
                .and_then(spell_capability)
                .is_some_and(|kind| prepared.effects.iter().any(|effect| effect.kind() == kind));
            if !known {
                let id = spell
                    .and_then(|spell| spell.action_id.clone())
                    .unwrap_or_default();
                reasons.push(format!("the demon casts {id} without a known behavior"));
            }
        }
    }
    reasons
}
