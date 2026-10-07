//! tools/oracle-v2/damage_taken.go: the attack table of a hit that lands on the player, which a
//! spell that hits the player rolls on, and the refusals that keep its reading sound.
//! `player_takes_damage` is in export.rs.

use serde_json::{json, Value};

use super::enemy::acting_damage_taken_modifiers;
use super::env::Environment;

/// `selfAttackTable`: the player's attack table against itself, which a spell that hits the
/// player (the Goblin Sapper Charge's self hit) rolls on.
#[allow(dead_code)]
pub(crate) fn self_attack_table(env: &Environment, unrepresented: &mut Vec<String>) -> Value {
    let table = env.attack_table(env.player, env.player);
    if table.damage_done_by_caster {
        unrepresented.push("caster damage callbacks on the player are unsupported".to_string());
    }
    // Absorb shields register a damage taken modifier that acts only while their aura is up;
    // what activates the shields is a spell or listener the gate checks.
    if acting_self_damage_taken_modifiers(env) > 0 {
        unrepresented
            .push("dynamic damage taken modifiers on the player are unsupported".to_string());
    }
    json!({
        "base_spell_miss_chance": table.base_spell_miss_chance,
        "spell_crit_suppression": table.spell_crit_suppression,
        "bonus_spell_crit_percent": table.bonus_spell_crit_percent,
        "crit_multiplier": table.crit_multiplier,
        "damage_dealt_multiplier": table.damage_dealt_multiplier,
        "damage_taken_multiplier": table.damage_taken_multiplier,
    })
}

/// `actingSelfDamageTakenModifiers`: how many of the player's dynamic damage taken modifiers
/// change a hit on the player at reset. The only modifiers a player registers are the
/// absorption shields', which act only while their aura is up.
pub(crate) fn acting_self_damage_taken_modifiers(env: &Environment) -> usize {
    acting_damage_taken_modifiers(env)
}
