//! The Druid part of the prepared v2 build gate: Druid effects, the effects of Druid spells,
//! the auras Druid effects claim and the forms the supported builds stay in. The shared gate
//! is in `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassDruid",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits,
};

/// Druid effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "druid_forms",
    "eclipse",
    "innervate",
    "insect_swarm",
    "moonfire",
    "moonkin_form",
    "natures_grace",
    "omen_of_clarity",
    "starfire",
    "wrath",
];

/// The effect kind whose implementation executes a Druid spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    match spell.class_spell.as_deref()? {
        "moonkin_form" => Some("moonkin_form"),
        "starfire" if spell.damage_effect.is_some() => Some("starfire"),
        "wrath" if spell.damage_effect.is_some() => Some("wrath"),
        "moonfire" if spell.damage_effect.is_some() => Some("moonfire"),
        "insect_swarm" if spell.dot.is_some() => Some("insect_swarm"),
        "innervate" => Some("innervate"),
        _ => None,
    }
}

/// Aura labels a Druid effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::OmenOfClarity {
            trigger_aura, aura, ..
        }
        | Effect::NaturesGrace {
            trigger_aura, aura, ..
        }
        | Effect::Eclipse {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("player", aura)],
        Effect::Innervate { aura, .. } | Effect::MoonkinForm { aura, .. } => {
            vec![("player", aura)]
        }
        _ => Vec::new(),
    }
}

/// The supported druid starts in Moonkin Form and casts only what that form allows, so no
/// cast changes form. Omen of Clarity's chance is modeled for spells, not melee swings.
fn limits(prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    let mut reasons = Vec::new();
    let spells = &prepared.player.spells;
    for effect in &prepared.effects {
        match effect {
            Effect::DruidForms {
                starting_form,
                spells: forms,
            } => {
                if starting_form != &["moonkin"] {
                    reasons.push(format!(
                        "druid starting form {starting_form:?} is unsupported"
                    ));
                }
                for spell in reachable {
                    let Some(index) = spells.iter().position(|s| std::ptr::eq(s, *spell)) else {
                        continue;
                    };
                    if let Some(entry) = forms.iter().find(|entry| entry.spell == index) {
                        if !entry.forms.iter().any(|form| form == "moonkin") {
                            let id = spell.action_id.clone().unwrap_or_default();
                            reasons.push(format!(
                                "rotation reaches {id}, which Moonkin Form cannot cast"
                            ));
                        }
                    }
                }
            }
            Effect::OmenOfClarity {
                callbacks,
                outcome,
                require_damage_dealt,
                trigger_spells,
                ..
            } => {
                if callbacks
                    .iter()
                    .any(|callback| callback != "on_spell_hit_dealt" && callback != "on_heal_dealt")
                {
                    reasons.push(format!("Omen of Clarity listens to {callbacks:?}"));
                }
                if !(outcome.iter().any(|o| o == "Hit") && outcome.iter().any(|o| o == "Crit")) {
                    reasons.push(format!("Omen of Clarity procs on {outcome:?}"));
                }
                if *require_damage_dealt {
                    reasons.push("Omen of Clarity requires damage dealt".into());
                }
                for &index in trigger_spells {
                    let Some(spell) = spells.get(index) else {
                        reasons.push(format!("Omen of Clarity names spell {index}"));
                        continue;
                    };
                    let spell_mask = spell.proc_mask.iter().any(|mask| {
                        mask == "ProcMaskSpellDamage" || mask == "ProcMaskSpellHealing"
                    });
                    if !spell_mask {
                        let id = spell.action_id.clone().unwrap_or_default();
                        reasons.push(format!(
                            "Omen of Clarity listens to {id}, whose chance comes from a melee swing"
                        ));
                    }
                }
            }
            _ => {}
        }
    }
    reasons
}
