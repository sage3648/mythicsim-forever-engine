//! The Warrior part of the prepared v2 build gate: Warrior effects, the effects of Warrior
//! spells and the auras Warrior effects claim. The shared gate is in `engine/coverage.rs`.

use crate::{
    contracts::prepared_v2::{Effect, PreparedV2, Spell},
    engine::coverage::ClassGate,
};

pub(crate) const GATE: ClassGate = ClassGate {
    class: "ClassWarrior",
    effects: EFFECTS,
    spell: spell_capability,
    claims,
    limits,
};

/// Warrior effect kinds implemented in Rust and validated against the pinned Go reference.
const EFFECTS: &[&str] = &[
    "anger_management",
    "battle_shout",
    "berserker_rage",
    "bloodthrill",
    "bloodrage",
    "bloodthirst",
    "death_wish",
    "deep_wounds",
    "execute",
    "warrior_flurry",
    "hamstring",
    "heroic_strike_queue",
    "mortal_strike",
    "overpower",
    "overpower_window",
    "recklessness",
    "rend",
    "slam",
    "spearing_strike",
    "sunder_armor",
    "unbridled_wrath",
    "warrior_stances",
    "weaponmaster_sword",
    "whirlwind",
];

/// The effect kind whose implementation executes a Warrior spell.
fn spell_capability(spell: &Spell) -> Option<&'static str> {
    // Bloodrage and the strikes' queue casts carry no class mask; the shared gate names them
    // by their effects' spells.
    match spell.class_spell.as_deref()? {
        "bloodthirst" => Some("bloodthirst"),
        "whirlwind" | "whirlwind_off_hand" => Some("whirlwind"),
        "execute" => Some("execute"),
        "hamstring" => Some("hamstring"),
        "berserker_rage" => Some("berserker_rage"),
        "death_wish" => Some("death_wish"),
        "recklessness" => Some("recklessness"),
        "sunder_armor" => Some("sunder_armor"),
        "deep_wounds" => Some("deep_wounds"),
        "heroic_strike" | "cleave" => Some("heroic_strike_queue"),
        "battle_shout" => Some("battle_shout"),
        "rend" => Some("rend"),
        "overpower" => Some("overpower"),
        "mortal_strike" => Some("mortal_strike"),
        "spearing_strike" => Some("spearing_strike"),
        "slam" => Some("slam"),
        "battle_stance" | "berserker_stance" | "defensive_stance" | "retaliation"
        | "shield_wall" => Some("warrior_stances"),
        _ => None,
    }
}

/// Aura labels a Warrior effect takes responsibility for, as (unit, label).
fn claims(effect: &Effect) -> Vec<(&'static str, &str)> {
    match effect {
        Effect::DeepWounds { trigger_aura, .. }
        | Effect::UnbridledWrath { trigger_aura, .. }
        | Effect::Bloodthrill { trigger_aura, .. }
        | Effect::WeaponmasterSword { trigger_aura, .. }
        | Effect::OverpowerWindow { trigger_aura, .. } => vec![("player", trigger_aura)],
        Effect::WarriorFlurry {
            trigger_aura, aura, ..
        } => vec![("player", trigger_aura), ("player", aura)],
        _ => Vec::new(),
    }
}

/// Spells whose Rust behavior never runs: Retaliation and Shield Wall, which a DPS warrior
/// casts only by hand, and the warrior's own Sunder Armor while its category is held for good.
/// The gate rejects a rotation that casts the first two itself or times their major cooldowns,
/// a stance change without the stance categories, Slam that stops the swings and Sunder Armor
/// that could stack.
fn limits(prepared: &PreparedV2, reachable: &[&Spell]) -> Vec<String> {
    let has = |kind: &str| prepared.effects.iter().any(|effect| effect.kind() == kind);
    let default_stance = prepared.effects.iter().find_map(|effect| match effect {
        Effect::WarriorStances { default_stance, .. } => Some(default_stance.as_str()),
        _ => None,
    });
    let stance_category = prepared.effects.iter().any(|effect| {
        matches!(effect, Effect::ExclusiveCategory { category, .. } if category == "Stance")
    });
    let sunder_blocked = prepared
        .effects
        .iter()
        .any(|effect| matches!(effect, Effect::SunderArmor { blocked: true, .. }));
    let slam_stops_swings = prepared.effects.iter().any(|effect| {
        matches!(
            effect,
            Effect::Slam {
                stops_swings: true,
                ..
            }
        )
    });
    // The spells the rotation names itself, prepull included, and the timed major cooldowns.
    let mut cast_by_hand = Vec::new();
    if let Ok(rotation) = crate::rotation::parse(&prepared.player.rotation) {
        for prepull in &rotation.prepull {
            if let crate::rotation::Action::CastSpell(id) = &prepull.action {
                cast_by_hand.push(id.clone());
            }
        }
        for item in &rotation.priority_list {
            if let crate::rotation::Action::CastSpell(id) = &item.action {
                cast_by_hand.push(id.clone());
            }
        }
    }
    for cooldown in &prepared.player.major_cooldowns {
        if !cooldown.timings_ns.is_empty() {
            cast_by_hand.push(cooldown.action_id.clone());
        }
    }
    let mut reasons = Vec::new();
    // Magic ticks would roll a magic hit or crit, which the bleeds' path lacks.
    for effect in &prepared.effects {
        match effect {
            Effect::Rend {
                tick_magic: true, ..
            }
            | Effect::DeepWounds {
                tick_magic: true, ..
            } => reasons.push(format!("{} ticks roll on the magic table", effect.kind())),
            _ => {}
        }
    }
    for spell in reachable {
        let id = spell.action_id.clone().unwrap_or_default();
        let Some(class_spell) = spell.class_spell.as_deref() else {
            continue;
        };
        match class_spell {
            "battle_stance" | "berserker_stance" | "defensive_stance"
                if stance_spell(class_spell) != default_stance
                    && !(stance_category && has("pseudo_stat_auras")) =>
            {
                reasons.push(format!("rotation reaches {id}, a stance change"));
            }
            "retaliation" | "shield_wall" if cast_by_hand.contains(&id) => {
                reasons.push(format!("rotation casts {id}, which has no behavior"));
            }
            "sunder_armor" if !sunder_blocked => {
                reasons.push(format!(
                    "rotation reaches {id}, which stacks the warrior's own Sunder Armor"
                ));
            }
            "slam" if slam_stops_swings => {
                reasons.push(format!(
                    "rotation reaches {id}, whose cast stops the swings"
                ));
            }
            _ => {}
        }
    }
    reasons.sort();
    reasons.dedup();
    reasons
}

/// The stance a stance cast enters.
fn stance_spell(class_spell: &str) -> Option<&'static str> {
    match class_spell {
        "battle_stance" => Some("battle"),
        "berserker_stance" => Some("berserker"),
        "defensive_stance" => Some("defensive"),
        _ => None,
    }
}
