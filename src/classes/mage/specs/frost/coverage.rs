//! The Frost build gate for prepared v2 inputs.
//!
//! A prepared input is supported only when every effect it declares, every active aura
//! that listens to combat events and every spell its rotation can reach has a Rust
//! implementation. Each gap becomes one stable reason string.

use std::collections::BTreeSet;

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell},
    rotation::{Action, Rotation},
};

/// Effect kinds implemented in Rust and validated against the pinned Go reference.
pub(crate) const IMPLEMENTED_EFFECTS: &[&str] = &[];

/// The effect kind whose implementation executes a castable spell.
fn spell_capability(spell: &Spell, prepared: &PreparedV2) -> Option<&'static str> {
    if let Some(class_spell) = &spell.class_spell {
        return match class_spell.as_str() {
            "frostbolt" => Some("frostbolt"),
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
        Effect::MageArmor { aura } => vec![("player", aura)],
        Effect::JudgementOfWisdom { aura, .. } => vec![("target", aura)],
        Effect::InertListener { unit, aura, .. } => match unit.as_str() {
            "player" => vec![("player", aura)],
            "target" => vec![("target", aura)],
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

/// Go `GetAPLCastSpell`/`GetAPLSpell`: the first APL-flagged spell with the action ID,
/// otherwise the first registered one. A missing spell drops the rotation action in Go.
pub(crate) fn rotation_spell<'a>(
    prepared: &'a PreparedV2,
    id: &crate::contracts::prepared_v2::ActionId,
) -> Option<&'a Spell> {
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

    let mut missing: BTreeSet<&str> = BTreeSet::new();
    for effect in &prepared.effects {
        if !IMPLEMENTED_EFFECTS.contains(&effect.kind()) {
            missing.insert(effect.kind());
        }
    }
    reasons.extend(
        missing
            .iter()
            .map(|kind| format!("effect {kind} is not implemented")),
    );

    let claimed: BTreeSet<(&str, &str)> = prepared.effects.iter().flat_map(claims).collect();
    for (unit, auras) in [
        ("player", &player.auras),
        ("target", &prepared.target.auras),
    ] {
        for aura in auras {
            if aura.active
                && aura.has_event_callbacks()
                && !claimed.contains(&(unit, aura.label.as_str()))
            {
                reasons.push(format!(
                    "{unit} aura {:?} listens to combat events without an effect",
                    aura.label
                ));
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
        let mut unimplemented = BTreeSet::new();
        for spell in reachable {
            let id = spell.action_id.clone().unwrap_or_default();
            // A declared but unimplemented effect is already reported above.
            match spell_capability(spell, prepared) {
                Some(kind) if prepared.effects.iter().any(|effect| effect.kind() == kind) => {}
                _ => {
                    unimplemented.insert(id.to_string());
                }
            }
        }
        reasons.extend(
            unimplemented
                .into_iter()
                .map(|id| format!("rotation reaches {id} without a known behavior")),
        );
    }
    reasons
}

/// Go resolves `auraIsActive` on the casting player with `GetAuraByID`: the first aura
/// with the same action ID, tag included.
fn player_has_aura(prepared: &PreparedV2, id: &crate::contracts::prepared_v2::ActionId) -> bool {
    prepared
        .player
        .auras
        .iter()
        .any(|aura| aura.action_id.as_ref() == Some(id))
}

/// Pinned Go returns no value for `auraIsActive` on an aura the character cannot have,
/// which drops the condition so the action fires whenever it is reached. Community fix
/// ElliotWood/Forever#622 (252f57aa8) reads such an aura as inactive instead. Until the
/// reference adopts the fix, reject these rotations rather than copy either behavior.
/// See upstream/changes.json.
fn unknown_aura_conditions(prepared: &PreparedV2, rotation: &Rotation) -> Vec<String> {
    let mut reasons = Vec::new();
    for item in &rotation.priority_list {
        let mut unknown = Vec::new();
        if let Some(condition) = &item.condition {
            condition.visit(&mut |value| {
                if let crate::rotation::Value::AuraIsActive(id) = value {
                    if !player_has_aura(prepared, id) {
                        unknown.push(id.to_string());
                    }
                }
            });
        }
        for id in unknown {
            reasons.push(format!(
                "rotation item {}: auraIsActive names {id}, which the character lacks; \
                 the pinned reference drops the condition and community #622 reads it as inactive",
                item.position
            ));
        }
    }
    reasons
}
