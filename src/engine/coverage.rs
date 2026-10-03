//! The prepared v2 build gate, shared by every class.
//!
//! An effect is required when an active aura that listens to combat events belongs to
//! it, or when the rotation can reach a spell it implements. A prepared input is
//! supported only when every required effect has a Rust implementation, every active
//! listener is claimed and every reachable spell stays inside the runtime's limits.
//! Each gap becomes one stable reason string. A class supplies its own effects, the
//! effects of its spells and the auras its effects claim through a [`ClassGate`].

use std::{collections::BTreeSet, sync::OnceLock};

use crate::{
    classes,
    contracts::prepared_v2::{ActionId, Effect, PreparedV2, Spell},
    core::fight::DIRECT_PROC_MASKS,
    rotation::{compile_condition, Action, FoundAura, Lookup, MissingAura, Rotation, Value},
};

/// A class's part of the gate.
pub(crate) struct ClassGate {
    /// Go's `Class` enum name, as the exporter writes it.
    pub(crate) class: &'static str,
    /// The class's effect kinds implemented in Rust and validated against Go.
    pub(crate) effects: &'static [&'static str],
    /// The effect kind whose implementation executes a class spell, by its class spell name
    /// and shape.
    pub(crate) spell: fn(&Spell) -> Option<&'static str>,
    /// The aura labels a class effect takes responsibility for, as (unit, label).
    pub(crate) claims: for<'a> fn(&'a Effect) -> Vec<(&'static str, &'a str)>,
    /// Class limits on the input and the spells the rotation can reach.
    pub(crate) limits: fn(&PreparedV2, &[&Spell]) -> Vec<String>,
}

/// Effects of races, items and raid buffs, which every class shares.
const COMMON_EFFECTS: &[&str] = &[
    "berserking",
    "blood_fury",
    "conjured_mana",
    "energize_on_use",
    "eureka",
    "inert_listener",
    "judgement_of_wisdom",
    "potion_mana",
    "read_ley_line",
    "shatter_curse",
    "temporary_stats",
    "touch_of_the_grave",
];

/// Every class with an implemented gate.
fn gates() -> [&'static ClassGate; 3] {
    [
        &classes::mage::prepared::GATE,
        &classes::druid::prepared::GATE,
        &classes::shaman::prepared::GATE,
    ]
}

/// Every effect kind implemented in Rust, sorted.
pub(crate) fn implemented_effects() -> &'static [&'static str] {
    static EFFECTS: OnceLock<Vec<&'static str>> = OnceLock::new();
    EFFECTS.get_or_init(|| {
        let mut effects: BTreeSet<&'static str> = COMMON_EFFECTS.iter().copied().collect();
        for gate in gates() {
            effects.extend(gate.effects.iter().copied());
        }
        effects.into_iter().collect()
    })
}

/// The gate of the prepared player's class, if Rust implements it.
pub(crate) fn class_gate(class: &str) -> Option<&'static ClassGate> {
    gates().into_iter().find(|gate| gate.class == class)
}

/// The effect kind of a spell that is not a class spell: items and racials.
fn common_spell_capability(spell: &Spell, prepared: &PreparedV2) -> Option<&'static str> {
    let id = spell.action_id.clone().unwrap_or_default();
    let item = id.item_id;
    prepared.effects.iter().find_map(|effect| match effect {
        Effect::PotionMana { item_id, .. } if *item_id == item => Some("potion_mana"),
        Effect::ConjuredMana { item_id, .. } if *item_id == item => Some("conjured_mana"),
        Effect::EnergizeOnUse { item_id, .. } if *item_id == item => Some("energize_on_use"),
        Effect::Eureka { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("eureka")
        }
        Effect::Berserking { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("berserking")
        }
        Effect::BloodFury { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("blood_fury")
        }
        Effect::ShatterCurse { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("shatter_curse")
        }
        Effect::ReadLeyLine { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("read_ley_line")
        }
        Effect::TemporaryStats { spell_id, .. } if *spell_id == id.spell_id && id.tag == 0 => {
            Some("temporary_stats")
        }
        _ => None,
    })
}

/// The effect kind whose implementation executes a castable spell.
fn spell_capability(
    spell: &Spell,
    prepared: &PreparedV2,
    gate: &ClassGate,
) -> Option<&'static str> {
    if spell.class_spell.is_some() {
        return (gate.spell)(spell);
    }
    common_spell_capability(spell, prepared)
}

/// Aura labels a race, item or raid buff effect takes responsibility for.
fn common_claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::TouchOfTheGrave { trigger_aura, .. } => vec![("player", trigger_aura)],
        Effect::Eureka { aura, .. }
        | Effect::Berserking { aura, .. }
        | Effect::BloodFury { aura, .. }
        | Effect::ShatterCurse { aura, .. }
        | Effect::ReadLeyLine { aura, .. }
        | Effect::TemporaryStats { aura, .. } => vec![("player", aura)],
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

/// Stats a temporary stat change may set: the runtime reads spell damage, attack powers and
/// spell crit during a fight, and nothing in scope reads healing power or physical crit.
const DYNAMIC_STATS: &[&str] = &[
    "SpellDamage",
    "AttackPower",
    "RangedAttackPower",
    "HealingPower",
    "SpellCritPercent",
    "PhysicalCritPercent",
];

/// Temporary stat changes to stats the runtime holds fixed.
fn fixed_stat_changes(prepared: &PreparedV2) -> Vec<String> {
    prepared
        .effects
        .iter()
        .filter_map(|effect| match effect {
            Effect::BloodFury {
                aura, active_stats, ..
            }
            | Effect::TemporaryStats {
                aura, active_stats, ..
            } => Some((aura, active_stats)),
            _ => None,
        })
        .flat_map(|(aura, stats)| {
            stats
                .keys()
                .filter(|stat| !DYNAMIC_STATS.contains(&stat.as_str()))
                .map(move |stat| format!("{aura} changes {stat}, which the runtime holds fixed"))
        })
        .collect()
}

/// Go `GetAPLSpell`: the first APL-flagged spell with the action ID, otherwise the first
/// registered one, as a spellbook position. A missing spell drops the rotation action in Go.
pub(crate) fn rotation_spell_index(prepared: &PreparedV2, id: &ActionId) -> Option<usize> {
    let spells = &prepared.player.spells;
    spells
        .iter()
        .position(|spell| spell.action_id.as_ref() == Some(id) && spell.has_flag("SpellFlagAPL"))
        .or_else(|| {
            spells
                .iter()
                .position(|spell| spell.action_id.as_ref() == Some(id))
        })
}

/// Go `Spell.Dot`: the spell holding the dot a spell names, its own or its related dot
/// spell's.
pub(crate) fn dot_owner(prepared: &PreparedV2, index: usize) -> Option<usize> {
    let spells = &prepared.player.spells;
    match &spells[index] {
        spell if spell.dot.is_some() => Some(index),
        spell => spell
            .related_dot_spell
            .filter(|&related| spells.get(related).is_some_and(|s| s.dot.is_some())),
    }
}

/// [`rotation_spell_index`] as the exported spell.
pub(crate) fn rotation_spell<'a>(prepared: &'a PreparedV2, id: &ActionId) -> Option<&'a Spell> {
    rotation_spell_index(prepared, id).map(|index| &prepared.player.spells[index])
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
    let Some(gate) = class_gate(&player.class) else {
        return vec![format!("class {} is not supported", player.class)];
    };
    if player.level != 60 {
        reasons.push(format!("player level {} is not supported", player.level));
    }
    if !(60..=63).contains(&prepared.target.level) {
        reasons.push(format!(
            "target level {} is not supported",
            prepared.target.level
        ));
    }

    let claims = |effect: &Effect| {
        let mut claimed = common_claims(effect);
        claimed.extend((gate.claims)(effect));
        claimed
            .into_iter()
            .map(|(unit, label)| (unit, label.to_string()))
            .collect::<Vec<_>>()
    };
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
        let mut registered_prepull = 0;
        for prepull in &rotation.prepull {
            match &prepull.action {
                Action::CastSpell(id) => {
                    if let Some(spell) = rotation_spell(prepared, id) {
                        registered_prepull += 1;
                        reachable.push(spell);
                    }
                }
                Action::AutocastOtherCooldowns => {}
            }
        }
        if registered_prepull != player.prepull_actions {
            reasons.push(format!(
                "Go registered {} prepull actions and the rotation {registered_prepull}; \
                 prepull actions outside the rotation are unsupported",
                player.prepull_actions
            ));
        }
        let unreachable = unreachable_with_one_target(prepared, rotation);
        for item in &rotation.priority_list {
            if unreachable.contains(&item.position) {
                continue;
            }
            match &item.action {
                Action::CastSpell(id) => reachable.extend(rotation_spell(prepared, id)),
                Action::AutocastOtherCooldowns => {
                    for cooldown in &player.major_cooldowns {
                        reachable.extend(rotation_spell(prepared, &cooldown.action_id));
                    }
                }
            }
        }
        reasons.extend((gate.limits)(prepared, &reachable));
        reasons.extend(undirected_procs(prepared));
        reasons.extend(fixed_stat_changes(prepared));
        let mut unknown = BTreeSet::new();
        let mut limited = BTreeSet::new();
        for spell in reachable {
            let id = spell.action_id.clone().unwrap_or_default().to_string();
            match spell_capability(spell, prepared, gate) {
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
    } else if player.prepull_actions != 0 {
        reasons.push("prepull actions outside the rotation are unsupported".into());
    }

    let implemented = implemented_effects();
    let missing: Vec<String> = required
        .into_iter()
        .filter(|kind| !implemented.contains(kind))
        .map(|kind| format!("effect {kind} is not implemented"))
        .collect();
    let mut all = missing;
    all.extend(reasons);
    all
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

/// Rotation items, by position, whose condition can never hold against the one target the
/// runtime supports, such as a `numberTargets` of two or more. Go still evaluates them, without
/// side effects, but never runs their action.
fn unreachable_with_one_target(prepared: &PreparedV2, rotation: &Rotation) -> BTreeSet<usize> {
    let aura = |id: &ActionId| find_aura(prepared, id);
    let spell = |id: &ActionId| rotation_spell_index(prepared, id);
    let dot = |id: &ActionId| {
        rotation_spell_index(prepared, id)
            .filter(|&index| prepared.player.spells[index].dot.is_some())
    };
    let lookup = Lookup {
        aura: &aura,
        spell: &spell,
        dot: &dot,
    };
    rotation
        .priority_list
        .iter()
        .filter(|item| {
            let condition = item.condition.as_ref().map(Value::with_one_target);
            compile_condition(condition.as_ref(), &lookup, MissingAura::Dropped).never_holds()
        })
        .map(|item| item.position)
        .collect()
}

/// Pinned Go gives `auraIsActive` and `auraNumStacks` on an aura the character cannot have
/// no value, which drops the term from its condition; community fix ElliotWood/Forever#622
/// (252f57aa8) reads the aura as inactive, with no stacks, instead. Where both compile to the
/// same action, as when an `auraIsKnown` guard already prunes it, the input is unaffected.
/// Otherwise reject it until the reference adopts the fix. See upstream/changes.json.
fn unknown_aura_conditions(prepared: &PreparedV2, rotation: &Rotation) -> Vec<String> {
    let known = |id: &ActionId| find_aura(prepared, id).is_some();
    // Go resolves rotation names on the casting player: `GetAuraByID` finds the first aura
    // with the same action ID, tag included; `GetAPLSpell` and `GetAPLDot` find spells.
    let aura = |id: &ActionId| find_aura(prepared, id);
    let spell = |id: &ActionId| rotation_spell_index(prepared, id);
    let dot = |id: &ActionId| {
        rotation_spell_index(prepared, id).and_then(|index| dot_owner(prepared, index))
    };
    let lookup = Lookup {
        aura: &aura,
        spell: &spell,
        dot: &dot,
    };
    let mut reasons = Vec::new();
    for item in &rotation.priority_list {
        let pinned = compile_condition(item.condition.as_ref(), &lookup, MissingAura::Dropped);
        let fixed = compile_condition(item.condition.as_ref(), &lookup, MissingAura::Inactive);
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
