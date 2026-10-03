//! The Mage build gate for prepared v2 inputs.
//!
//! An effect is required when an active aura that listens to combat events belongs to
//! it, or when the rotation can reach a spell it implements. A prepared input is
//! supported only when every required effect has a Rust implementation, every active
//! listener is claimed and every reachable spell stays inside the runtime's limits.
//! Each gap becomes one stable reason string.

use std::collections::BTreeSet;

use crate::{
    contracts::prepared_v2::{ActionId, Effect, PreparedV2, Spell},
    core::fight::DIRECT_PROC_MASKS,
    rotation::{compile_condition, Action, FoundAura, MissingAura, Rotation, Value},
};

/// Effect kinds implemented in Rust and validated against the pinned Go reference.
pub(crate) const IMPLEMENTED_EFFECTS: &[&str] = &[
    "arcane_blast",
    "arcane_concentration",
    "arcane_missiles",
    "arcane_power",
    "cold_snap",
    "conjured_mana",
    "energize_on_use",
    "evocation",
    "fingers_of_frost",
    "fire_blast",
    "fireball",
    "frostbolt",
    "ice_lance",
    "ignite",
    "inert_listener",
    "judgement_of_wisdom",
    "mana_gems",
    "master_of_elements",
    "missile_barrage",
    "potion_mana",
    "presence_of_mind",
    "scorch",
    "touch_of_the_grave",
    "winters_chill",
];

/// The effect kind whose implementation executes a castable spell.
fn spell_capability(spell: &Spell, prepared: &PreparedV2) -> Option<&'static str> {
    if let Some(class_spell) = &spell.class_spell {
        return match class_spell.as_str() {
            "frostbolt" => Some("frostbolt"),
            "arcane_blast" => Some("arcane_blast"),
            "arcane_power" => Some("arcane_power"),
            "fire_blast" => Some("fire_blast"),
            "fireball" => Some("fireball"),
            "scorch" => Some("scorch"),
            "presence_of_mind" => Some("presence_of_mind"),
            "ice_lance" => Some("ice_lance"),
            "arcane_missiles_cast" => Some("arcane_missiles"),
            "cold_snap" => Some("cold_snap"),
            "evocation" => Some("evocation"),
            "mana_gem" => Some("mana_gems"),
            _ => None,
        };
    }
    let item = spell.action_id.as_ref().map_or(0, |id| id.item_id);
    prepared.effects.iter().find_map(|effect| match effect {
        Effect::PotionMana { item_id, .. } if *item_id == item => Some("potion_mana"),
        Effect::ConjuredMana { item_id, .. } if *item_id == item => Some("conjured_mana"),
        Effect::EnergizeOnUse { item_id, .. } if *item_id == item => Some("energize_on_use"),
        _ => None,
    })
}

/// Aura labels an effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::ArcaneConcentration {
            trigger_aura, aura, ..
        }
        | Effect::MissileBarrage {
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
        Effect::Ignite { trigger_aura, .. }
        | Effect::TouchOfTheGrave { trigger_aura, .. }
        | Effect::MasterOfElements { trigger_aura, .. } => {
            vec![("player", trigger_aura)]
        }
        Effect::MageArmor { aura }
        | Effect::ArcaneBlast { aura, .. }
        | Effect::ArcanePower { aura, .. }
        | Effect::PresenceOfMind { aura, .. } => vec![("player", aura)],
        Effect::JudgementOfWisdom { aura, .. } => vec![("target", aura)],
        Effect::InertListener { unit, aura, .. } => match unit.as_str() {
            "player" => vec![("player", aura)],
            "target" => vec![("target", aura)],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Proc triggers Rust matches as `ProcMaskDirect`; any other mask is unsupported.
fn undirected_procs(prepared: &PreparedV2) -> Vec<String> {
    prepared
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::TouchOfTheGrave {
                trigger_aura,
                proc_mask,
                ..
            } => {
                let mut mask: Vec<&str> = proc_mask.iter().map(String::as_str).collect();
                let mut direct = DIRECT_PROC_MASKS.to_vec();
                mask.sort_unstable();
                direct.sort_unstable();
                (mask != direct)
                    .then(|| format!("{trigger_aura} procs from {proc_mask:?}, not direct hits"))
            }
            _ => None,
        })
        .collect()
}

/// Go `SpellSchoolFire`.
const FIRE: u8 = 4;

/// Ignite's trigger is claimed as a listener that never acts, which holds only while no
/// spell Rust can deal damage with is fire: the reachable spells and the missiles their
/// channels cast.
fn active_ignite(prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    if !prepared
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::Ignite { .. }))
    {
        return Vec::new();
    }
    let missiles = prepared.effects.iter().flat_map(|effect| match effect {
        Effect::ArcaneMissiles { ranks } => ranks
            .iter()
            .filter(|rank| {
                reachable.iter().any(|spell| {
                    spell.action_id.as_ref().map(|id| id.spell_id) == Some(rank.channel_spell_id)
                })
            })
            .map(|rank| rank.tick_spell_id)
            .collect(),
        _ => Vec::new(),
    });
    let ticks: Vec<&Spell> = missiles
        .flat_map(|id| {
            prepared
                .player
                .spells
                .iter()
                .filter(move |spell| spell.action_id.as_ref().map(|a| a.spell_id) == Some(id))
        })
        .collect();
    reachable
        .iter()
        .chain(ticks.iter())
        .filter(|spell| spell.school & FIRE != 0)
        .map(|spell| {
            format!(
                "{} is a fire spell, whose crits Ignite would act on",
                spell.action_id.clone().unwrap_or_default()
            )
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Go `GetAPLCastSpell`/`GetAPLSpell`: the first APL-flagged spell with the action ID,
/// otherwise the first registered one. A missing spell drops the rotation action in Go.
pub(crate) fn rotation_spell<'a>(prepared: &'a PreparedV2, id: &ActionId) -> Option<&'a Spell> {
    let spells = &prepared.player.spells;
    spells
        .iter()
        .find(|spell| spell.action_id.as_ref() == Some(id) && spell.has_flag("SpellFlagAPL"))
        .or_else(|| {
            spells
                .iter()
                .find(|spell| spell.action_id.as_ref() == Some(id))
        })
}

/// Spell properties the fight runtime does not implement.
fn runtime_limits(spell: &Spell) -> Vec<&'static str> {
    let mut limits = Vec::new();
    if spell.max_charges != 0 {
        limits.push("charges");
    }
    if spell.has_cast_requirement {
        limits.push("cast requirements");
    }
    if spell.min_range != 0.0 || spell.max_range != 0.0 {
        limits.push("range limits");
    }
    if spell.damage_effect.is_some() && spell.school & 1 != 0 {
        limits.push("physical damage");
    }
    if spell
        .dot
        .as_ref()
        .is_some_and(|dot| dot.affected_by_cast_speed || dot.affected_by_real_haste)
    {
        limits.push("hasted periodic effects");
    }
    limits
}

pub(crate) fn prepared_coverage(prepared: &PreparedV2, rotation: Option<&Rotation>) -> Vec<String> {
    let mut reasons = Vec::new();
    let player = &prepared.player;
    if player.class != "ClassMage" {
        reasons.push(format!("class {} is not supported", player.class));
    }
    if player.level != 60 {
        reasons.push(format!("player level {} is not supported", player.level));
    }
    if !(60..=63).contains(&prepared.target.level) {
        reasons.push(format!(
            "target level {} is not supported",
            prepared.target.level
        ));
    }

    let mut required: BTreeSet<&str> = BTreeSet::new();
    for (unit, auras) in [
        ("player", &player.auras),
        ("target", &prepared.target.auras),
    ] {
        for aura in auras {
            if !aura.active || !aura.has_event_callbacks() {
                continue;
            }
            let claimant = prepared.effects.iter().find(|effect| {
                claims(effect)
                    .iter()
                    .any(|(u, label)| *u == unit && *label == aura.label)
            });
            match claimant {
                Some(effect) => {
                    required.insert(effect.kind());
                }
                None => reasons.push(format!(
                    "{unit} aura {:?} listens to combat events without an effect",
                    aura.label
                )),
            }
        }
    }

    if let Some(rotation) = rotation {
        reasons.extend(unknown_aura_conditions(prepared, rotation));
        let mut reachable = Vec::new();
        for item in &rotation.priority_list {
            match &item.action {
                Action::CastSpell(id) => reachable.extend(rotation_spell(prepared, id)),
                Action::AutocastOtherCooldowns => {
                    for cooldown in &player.major_cooldowns {
                        reachable.extend(rotation_spell(prepared, &cooldown.action_id));
                    }
                }
            }
        }
        reasons.extend(active_ignite(prepared, &reachable));
        reasons.extend(undirected_procs(prepared));
        let mut unknown = BTreeSet::new();
        let mut limited = BTreeSet::new();
        for spell in reachable {
            let id = spell.action_id.clone().unwrap_or_default().to_string();
            match spell_capability(spell, prepared) {
                Some(kind) if prepared.effects.iter().any(|effect| effect.kind() == kind) => {
                    required.insert(kind);
                }
                _ => {
                    unknown.insert(id.clone());
                }
            }
            for limit in runtime_limits(spell) {
                limited.insert(format!(
                    "rotation reaches {id}, which uses unsupported {limit}"
                ));
            }
        }
        reasons.extend(
            unknown
                .into_iter()
                .map(|id| format!("rotation reaches {id} without a known behavior")),
        );
        reasons.extend(limited);
    }

    let missing: Vec<String> = required
        .into_iter()
        .filter(|kind| !IMPLEMENTED_EFFECTS.contains(kind))
        .map(|kind| format!("effect {kind} is not implemented"))
        .collect();
    let mut all = missing;
    all.extend(reasons);
    all
}

/// Go resolves `auraIsActive` on the casting player with `GetAuraByID`: the first aura
/// with the same action ID, tag included.
fn player_has_aura(prepared: &PreparedV2, id: &ActionId) -> bool {
    find_aura(prepared, id).is_some()
}

fn find_aura(prepared: &PreparedV2, id: &ActionId) -> Option<FoundAura<ActionId>> {
    prepared
        .player
        .auras
        .iter()
        .find(|aura| aura.action_id.as_ref() == Some(id))
        .map(|aura| FoundAura {
            aura: id.clone(),
            max_stacks: aura.max_stacks,
        })
}

/// Pinned Go gives `auraIsActive` and `auraNumStacks` on an aura the character cannot have
/// no value, which drops the term from its condition; community fix ElliotWood/Forever#622
/// (252f57aa8) reads the aura as inactive, with no stacks, instead. Where both compile to the same action, as when an
/// `auraIsKnown` guard already prunes it, the input is unaffected. Otherwise reject it
/// until the reference adopts the fix. See upstream/changes.json.
fn unknown_aura_conditions(prepared: &PreparedV2, rotation: &Rotation) -> Vec<String> {
    let known = |id: &ActionId| player_has_aura(prepared, id);
    let aura = |id: &ActionId| find_aura(prepared, id);
    let mut reasons = Vec::new();
    for item in &rotation.priority_list {
        let pinned = compile_condition(item.condition.as_ref(), &aura, MissingAura::Dropped);
        let fixed = compile_condition(item.condition.as_ref(), &aura, MissingAura::Inactive);
        if pinned.same_meaning(&fixed) {
            continue;
        }
        let mut unknown = Vec::new();
        if let Some(condition) = &item.condition {
            condition.visit(&mut |value| match value {
                Value::AuraIsActive(id) if !known(id) => {
                    unknown.push(("auraIsActive", id.to_string()))
                }
                Value::AuraNumStacks(id) if !known(id) => {
                    unknown.push(("auraNumStacks", id.to_string()))
                }
                Value::AuraRemainingTime(id) if !known(id) => {
                    unknown.push(("auraRemainingTime", id.to_string()))
                }
                _ => {}
            });
        }
        for (operator, id) in unknown {
            reasons.push(format!(
                "rotation item {}: {operator} names {id}, which the character lacks; \
                 the pinned reference drops the condition and community #622 reads it as inactive",
                item.position
            ));
        }
    }
    reasons
}
