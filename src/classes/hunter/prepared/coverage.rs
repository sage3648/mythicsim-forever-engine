//! The Hunter part of the prepared v2 build gate: Hunter effects, the effects of Hunter spells
//! and the auras Hunter effects claim. The shared gate is in `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassHunter",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits,
};

/// Hunter effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "aimed_shot",
    "aspect_of_the_hawk",
    "bestial_wrath",
    "frenzy",
    "hunter_pet",
    "hunter_pet_strike",
    "intimidation",
    "multi_shot",
    "rapid_fire",
    "serpent_sting",
    "sniper_shot",
    "summon_hawk",
];

/// The effect kind whose implementation executes a Hunter spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    let id = spell.action_id.clone().unwrap_or_default();
    if id.tag != 0 || id.item_id != 0 {
        return None;
    }
    match spell.class_spell.as_deref()? {
        "aimed_shot" => Some("aimed_shot"),
        "sniper_shot" => Some("sniper_shot"),
        "multi_shot" => Some("multi_shot"),
        "serpent_sting" if spell.dot.is_some() => Some("serpent_sting"),
        "aspect_of_the_hawk" => Some("aspect_of_the_hawk"),
        "rapid_fire" => Some("rapid_fire"),
        "summon_hawk" => Some("summon_hawk"),
        "intimidation" => Some("intimidation"),
        "bestial_wrath" => Some("bestial_wrath"),
        _ => None,
    }
}

/// Aura labels a Hunter effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::AspectOfTheHawk {
            aura, proc_aura, ..
        } => {
            let mut claimed = vec![("player", aura.as_str())];
            claimed.extend(proc_aura.iter().map(|label| ("player", label.as_str())));
            claimed
        }
        Effect::RapidFire { aura, .. } => vec![("player", aura)],
        Effect::HunterPet { pet, .. } => vec![("pet unit", pet)],
        Effect::Frenzy { trigger_aura, .. } => vec![("pet", trigger_aura)],
        Effect::Intimidation { aura, .. } | Effect::BestialWrath { aura, .. } => {
            vec![("pet", aura)]
        }
        _ => Vec::new(),
    }
}

/// Shapes the Hunter runtime implements.
fn limits(prepared: &PreparedV2, _reachable: &[&Spell]) -> Vec<String> {
    let mut reasons = Vec::new();
    for effect in &prepared.effects {
        match effect {
            Effect::SerpentSting { tick_outcome, .. }
                if crate::classes::hunter::spells::serpent_sting::tick_outcome(tick_outcome)
                    .is_none() =>
            {
                reasons.push(format!("Serpent Sting ticks with {tick_outcome}"));
            }
            Effect::SummonHawk { hawk_spells, .. }
                if hawk_spells.is_empty()
                    || hawk_spells.iter().any(|&spell| {
                        prepared
                            .player
                            .spells
                            .get(spell)
                            .is_none_or(|spell| spell.dot.is_none())
                    }) =>
            {
                reasons.push("Summon Hawk names a hawk without a dot".into());
            }
            Effect::AspectOfTheHawk {
                proc_aura,
                haste_multiplier,
                proc_chance,
                ..
            } if proc_aura.is_some() != haste_multiplier.is_some()
                || proc_aura.is_some() != proc_chance.is_some() =>
            {
                reasons.push("Deadly Aspects is incomplete".into());
            }
            Effect::HunterPet {
                rotation,
                special_ability,
                focus_dump,
                extra_ability,
                ..
            } => {
                if rotation != "cat" && rotation != "default" {
                    reasons.push(format!("the pet rotation {rotation} is unsupported"));
                }
                let pet_spells = prepared.pets.first().map_or(&[][..], |pet| &pet.spells[..]);
                for slot in [special_ability, focus_dump, extra_ability] {
                    let Ok(position) = usize::try_from(*slot) else {
                        continue;
                    };
                    let strike = pet_spells.get(position).is_some_and(|spell| {
                        let id = spell.action_id.clone().unwrap_or_default();
                        prepared.effects.iter().any(|effect| {
                            matches!(effect, Effect::HunterPetStrike { spell_id, .. } if *spell_id == id.spell_id)
                        })
                    });
                    if !strike {
                        reasons.push(format!("the pet's ability at {position} has no behavior"));
                    }
                }
            }
            _ => {}
        }
    }
    reasons
}
