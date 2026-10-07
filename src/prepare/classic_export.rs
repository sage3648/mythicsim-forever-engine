//! The exporter's descriptions of the effects sim/common/classic registers: tools/oracle-v2
//! `melee_procs.go` Crusader (enchants.go) and Sulfuras, Hand of Ragnaros (items_weapons.go).
//! Ironfoe's Fury of Forgewright is described with the extra attack procs in `export_items.rs`.

use serde_json::{json, Value};

use crate::contracts::prepared_v2::ActionId;

use super::classic_weapons::tagged;
use super::common_effects::{action_id_string, flat_string};
use super::env::Environment;
use super::export_items::{aura_named, dpm_chances, spell_position};
use super::spell::SpellFlag;
use super::stats::{Stat, Stats};

/// melee_procs.go: Crusader (1900), a weapon proc on landed hits, at one proc a minute of each
/// hand's speed, that activates that hand's Holy Strength and heals.
pub(crate) fn crusader_effect(
    env: &Environment,
    unrepresented: &mut Vec<String>,
    effects: &mut Vec<Value>,
) {
    let Some(aura) = aura_named(env, "Enchant Weapon - Crusader") else {
        return;
    };
    let aura = env.sim.aura(aura);
    let Some(dpm) = aura.dpm.as_ref() else {
        unrepresented.push("Crusader has no proc manager".to_string());
        return;
    };
    let buffs = Stats::from_pairs(&[(Stat::Strength, 100.0)]);
    let id = ActionId::spell(20007);
    let stats = flat_string(&buffs);
    let gained = |tag: i32| {
        format!(
            "Gained {stats} from {}.",
            action_id_string(&tagged(&id, tag))
        )
    };
    let lost = |tag: i32| {
        format!(
            "Lost {stats} from fading {}.",
            action_id_string(&tagged(&id, tag))
        )
    };
    effects.push(json!({
        "kind": "crusader", "trigger_aura": aura.label,
        "mh_aura": "Holy Strength (MH)", "oh_aura": "Holy Strength (OH)",
        "chances": dpm_chances(env, dpm, |spell| {
            !spell.flags.matches(SpellFlag::SUPPRESS_WEAPON_PROCS)
        }),
        "heal_min": 75.0, "heal_max": 125.0, "heal_metrics_action_id": json!({"spell_id": 20007}),
        "mh_gain_log": gained(1), "mh_expire_log": lost(1),
        "oh_gain_log": gained(2), "oh_expire_log": lost(2),
    }));
}

/// melee_procs.go: Sulfuras, Hand of Ragnaros, a weapon proc at one proc a minute of the
/// weapon's speed that casts its Fireball at once, rolled 273 to 333 on the magic hit table,
/// whose landing applies a burn of 15 every 2 seconds; and Immolation, 5 Fire that always lands
/// on every melee attacker that lands a hit. Go literals.
pub(crate) fn sulfuras_effect(
    env: &Environment,
    unrepresented: &mut Vec<String>,
    effects: &mut Vec<Value>,
) {
    let Some(aura) = aura_named(env, "Sulfuras, Hand of Ragnaros Proc") else {
        return;
    };
    let immolation = aura_named(env, "Immolation (Hand of Ragnaros)");
    let fireball = spell_position(env, &ActionId::spell(21162));
    let burn = spell_position(env, &ActionId::spell(21142));
    let aura = env.sim.aura(aura);
    let (Some(dpm), Some(immolation), Some(fireball), Some(burn)) =
        (aura.dpm.as_ref(), immolation, fireball, burn)
    else {
        unrepresented.push("Sulfuras, Hand of Ragnaros is incomplete".to_string());
        return;
    };
    effects.push(json!({
        "kind": "sulfuras_hand_of_ragnaros", "trigger_aura": aura.label,
        "chances": dpm_chances(env, dpm, |spell| {
            !spell.flags.matches(SpellFlag::SUPPRESS_WEAPON_PROCS)
        }),
        "fireball_spell": fireball, "roll_min": 273.0, "roll_max": 333.0, "dot_base": 15.0,
        "immolation_aura": env.sim.aura(immolation).label, "immolation_spell": burn,
        "immolation_damage": 5.0,
    }));
}
