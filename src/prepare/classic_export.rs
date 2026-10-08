//! The exporter's descriptions of the effects sim/common/classic registers: tools/oracle-v2
//! `melee_procs.go` Crusader (enchants.go), Sulfuras, Hand of Ragnaros and Thunderfury
//! (items_weapons.go).
//! Ironfoe's Fury of Forgewright is described with the extra attack procs in `export_items.rs`.

use serde_json::{json, Value};

use crate::contracts::prepared_v2::ActionId;

use super::classic_weapons::tagged;
use super::classic_whelp::WHELP_NAME;
use super::common_effects::{action_id_string, flat_string, SPELL_BATCH_WINDOW};
use super::env::Environment;
use super::export_items::{aura_named, dpm_chances, spell_position};
use super::pet::summoned_pet;
use super::sim::SECOND;
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

/// melee_procs.go: Dragon's Call, a weapon proc on landed hits, at one proc a minute of the
/// weapon's speed, whose handler summons the Emerald Dragon Whelp for 15 seconds a spell batch
/// window later (emerald_dragon_whelp.go); its rotation spits half the time.
pub(crate) fn dragons_call_effect(
    env: &Environment,
    unrepresented: &mut Vec<String>,
    effects: &mut Vec<Value>,
) {
    let Some(aura) = aura_named(env, "Emerald Dragon Whelp Proc") else {
        return;
    };
    let whelp = env
        .pets()
        .into_iter()
        .rfind(|pet| summoned_pet(env, *pet) && env.sim.pet_data(*pet).name == WHELP_NAME);
    let aura = env.sim.aura(aura);
    let (Some(dpm), Some(whelp)) = (aura.dpm.as_ref(), whelp) else {
        unrepresented.push("Dragon's Call has no proc manager or whelp".to_string());
        return;
    };
    effects.push(json!({
        "kind": "emerald_dragon_whelp", "trigger_aura": aura.label,
        "pet": env.sim.unit(whelp).label,
        "chances": dpm_chances(env, dpm, |spell| {
            !spell.flags.matches(SpellFlag::SUPPRESS_WEAPON_PROCS)
        }),
        "delay_ns": SPELL_BATCH_WINDOW, "duration_ns": 15 * SECOND,
        "acid_spit_spell_id": 9591, "acid_spit_min": 374.0, "acid_spit_max": 503.0,
        "spit_chance": 0.5,
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

/// weapon_procs.go `thunderfuryEffects`: common/classic/items_weapons.go Thunderfury, Blessed
/// Blade of the Windseeker: a weapon proc on landed hits, at the weapon's proc manager, whose
/// handler a spell batch window later casts two spells on the unit hit. The first (tag 1) is a
/// nature hit of 300 on the magic table with a crit, whose landing puts Cyclone on the target: a
/// slow of 20% through `AtkSpeedReductionEffect`, an exclusive effect of the attack speed
/// category Thunder Clap shares. The second (tag 2) deals no damage on the magic hit table to up
/// to five targets from the unit hit, and each it lands on takes the Thunderfury aura: 25 less
/// nature resistance while it lasts. Both auras last 12 seconds. Every number is a Go literal of
/// the item.
pub(crate) fn thunderfury_effect(
    env: &Environment,
    unrepresented: &mut Vec<String>,
    effects: &mut Vec<Value>,
) {
    let Some(trigger) = aura_named(env, "Thunderfury Proc") else {
        return;
    };
    let strike = spell_position(env, &tagged(&ActionId::spell(21992), 1));
    let bounce = spell_position(env, &tagged(&ActionId::spell(21992), 2));
    let target = env.encounter.targets[0];
    let slow = env.sim.get_aura(target, "Cyclone");
    let resistance = env.sim.get_aura(target, "Thunderfury");
    let trigger = env.sim.aura(trigger);
    let (Some(dpm), None, Some(strike), Some(bounce), Some(slow), Some(resistance)) = (
        trigger.dpm.as_ref(),
        trigger.icd,
        strike,
        bounce,
        slow,
        resistance,
    ) else {
        unrepresented.push("Thunderfury's proc has no proc manager, spells or auras".to_string());
        return;
    };
    effects.push(json!({
        "kind": "thunderfury", "trigger_aura": trigger.label,
        "chances": dpm_chances(env, dpm, |spell| {
            !spell.flags.matches(SpellFlag::SUPPRESS_WEAPON_PROCS)
        }),
        "strike_spell": strike, "bounce_spell": bounce, "strike_damage": 300.0,
        "bounce_targets": 5, "slow_aura": env.sim.aura(slow).label,
        "slow_multiplier": super::shared_auras::slowed_time_multiplier(-20.0),
        "resistance_aura": env.sim.aura(resistance).label, "nature_resistance": -25.0,
    }));
}
