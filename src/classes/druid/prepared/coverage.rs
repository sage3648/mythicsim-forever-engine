//! The Druid part of the prepared v2 build gate: Druid effects, the effects of Druid spells,
//! the auras Druid effects claim and the forms the supported builds use. The shared gate is
//! in `engine/coverage.rs`.

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
    "berserk",
    "blood_frenzy",
    "cat_builders",
    "cat_form",
    "druid_forms",
    "eclipse",
    "faerie_fire",
    "ferocious_bite",
    "innervate",
    "insect_swarm",
    "moonfire",
    "moonkin_form",
    "natures_grace",
    "omen_of_clarity",
    "prowl",
    "rend_and_tear",
    "rip",
    "shifting_power",
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
        "cat_form" => Some("cat_form"),
        "shred" | "claw" | "ravage" => Some("cat_builders"),
        "rip" if spell.dot.is_some() => Some("rip"),
        "ferocious_bite" if spell.damage_effect.is_some() => Some("ferocious_bite"),
        "shifting_power" => Some("shifting_power"),
        "faerie_fire" => Some("faerie_fire"),
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
        Effect::Innervate { aura, .. }
        | Effect::MoonkinForm { aura, .. }
        | Effect::CatForm { aura, .. }
        | Effect::Prowl { aura, .. }
        | Effect::Berserk { aura, .. } => vec![("player", aura)],
        Effect::BloodFrenzy {
            trigger_aura,
            bear_trigger_aura,
            ..
        } => vec![("player", trigger_aura), ("player", bear_trigger_aura)],
        _ => Vec::new(),
    }
}

/// The supported druids start in Moonkin Form and cast only what that form allows, or start
/// in Cat Form and leave it only for caster form, whose spells clear it. The cat's builders
/// and Blood Frenzy read only what Rust models.
fn limits(prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    let mut reasons = Vec::new();
    let spells = &prepared.player.spells;
    let cat = prepared
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::CatForm { .. }));
    for effect in &prepared.effects {
        match effect {
            Effect::DruidForms {
                starting_form,
                spells: forms,
            } => {
                let allowed: &[&str] = match starting_form.as_slice() {
                    [form] if form == "moonkin" => &["moonkin"],
                    [form] if form == "cat" && cat => &["cat", "humanoid"],
                    _ => {
                        reasons.push(format!(
                            "druid starting form {starting_form:?} is unsupported"
                        ));
                        continue;
                    }
                };
                for spell in reachable {
                    let Some(index) = spells.iter().position(|s| std::ptr::eq(s, *spell)) else {
                        continue;
                    };
                    if let Some(entry) = forms.iter().find(|entry| entry.spell == index) {
                        if !entry
                            .forms
                            .iter()
                            .any(|form| allowed.contains(&form.as_str()))
                        {
                            let id = spell.action_id.clone().unwrap_or_default();
                            reasons.push(format!(
                                "rotation reaches {id}, which the supported forms cannot cast"
                            ));
                        }
                    }
                }
            }
            Effect::OmenOfClarity {
                callbacks,
                outcome,
                require_damage_dealt,
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
            }
            Effect::BloodFrenzy { outcome, .. } if outcome != &["Crit"] => {
                reasons.push(format!("Blood Frenzy procs on {outcome:?}"));
            }
            Effect::Rip {
                tick_magic: true, ..
            } => {
                reasons.push("Rip ticks on the magic table".into());
            }
            Effect::FaerieFire { refresh, .. } if refresh.iter().any(|mode| mode != "never") => {
                reasons.push(format!(
                    "Faerie Fire's armor reduction can take effect ({refresh:?}), which is not modeled"
                ));
            }
            _ => {}
        }
    }
    reasons
}
