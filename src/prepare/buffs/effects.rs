//! The exporter's descriptions of the raid buffs and debuffs: the parts of tools/oracle-v2
//! `commonEffects` and aura_refresh.go that describe what Go keeps in closures here.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::contracts::prepared_v2::ActionId;
use crate::contracts::request::Message;

use super::super::agent::proto_to_action_id;
use super::super::env::Environment;
use super::super::export::action_id;
use super::super::sim::{AuraId, Sim, UnitId, NEVER_EXPIRES};
use super::super::spell::{ProcMask, GCD_DEFAULT};
use super::generated::BATTLE_SHOUT;
use super::paladin;

/// Go `SpellBatchWindow`: the delay a proc handler waits before it runs.
const SPELL_BATCH_WINDOW: i64 = 10_000_000;

/// How exclusive_effect.go `ShouldRefreshExclusiveEffects` reads an aura in this fight, effect by
/// effect: "own" when no other effect shares its category, so it refreshes when inactive or about
/// to expire; "never" when a permanent effect of another aura holds the category for the whole
/// fight and the effect never takes it, so the aura still activates but its effect never
/// applies. Anything else is "unknown". A stacking aura weighs its priority by stacks, which
/// neither reading covers. Go `exclusiveRefresh`.
fn exclusive_refresh(sim: &Sim, aura: AuraId) -> Vec<&'static str> {
    let aura = sim.aura(aura);
    let mut modes = Vec::new();
    for effect in &aura.exclusive_effects {
        let ee = &sim.effects[effect.0];
        let category = &sim.categories[ee.category.0];
        let active = category.active_effect;
        let permanent = active.is_some_and(|active| {
            let holder = sim.aura(sim.effects[active.0].aura);
            active != *effect && holder.active && holder.duration == NEVER_EXPIRES
        });
        modes.push(if aura.max_stacks > 0 {
            "unknown"
        } else if category.effects.len() == 1 && (active.is_none() || active == Some(*effect)) {
            "own"
        } else if permanent
            && !category.single_aura
            // A permanent effect that outranks this one, or bids the same for another spell and
            // so keeps the tie, never lets it take the category. One that bids the same for the
            // same spell gives the category up while this one is up and takes it back after, at
            // the same value; either way it outlasts any refresh window, so the aura never needs
            // refreshing. A single-aura category would block the aura itself instead.
            && active.is_some_and(|active| sim.effects[active.0].priority >= ee.priority)
        {
            "never"
        } else {
            "unknown"
        });
    }
    modes
}

/// tools/oracle-v2/aura_refresh.go `auraShouldRefreshEffects`: every aura an `auraShouldRefresh`
/// value names, on the player or its current target (the default), with how its exclusive effects
/// read. A value naming an aura the unit lacks has no aura and exports nothing.
pub(crate) fn aura_should_refresh_effects(env: &Environment) -> Vec<Value> {
    let Some(rotation) = env.sim.character(env.player).player.message("rotation") else {
        return Vec::new();
    };
    let tree = rotation.to_protojson();
    let mut found: BTreeMap<String, Value> = BTreeMap::new();
    let target = env.encounter.targets[0];
    collect_aura_should_refresh(env, &tree, target, &mut found);
    found.into_values().collect()
}

fn collect_aura_should_refresh(
    env: &Environment,
    node: &Value,
    target: UnitId,
    found: &mut BTreeMap<String, Value>,
) {
    match node {
        Value::Object(map) => {
            if let Some(config) = map.get("auraShouldRefresh").filter(|v| v.is_object()) {
                let parsed = Message::from_json_text(
                    "proto.APLValueAuraShouldRefresh",
                    &config.to_string(),
                )
                .expect("an auraShouldRefresh value parses");
                // apl_helpers.go GetTargetUnit: no unit reference means the current target.
                let (unit, name) = match parsed.message("source_unit") {
                    // UnitReference_Self.
                    Some(source) if source.enum_number("type") == 4 => (env.player, "player"),
                    _ => (target, "target"),
                };
                let id = parsed
                    .message("aura_id")
                    .map(proto_to_action_id)
                    .unwrap_or_default();
                let aura = env
                    .sim
                    .unit(unit)
                    .auras
                    .iter()
                    .copied()
                    .find(|aura| env.sim.aura(*aura).action_id.clone().unwrap_or_default() == id);
                if let Some(aura) = aura {
                    let label = env.sim.aura(aura).label.clone();
                    found.insert(
                        format!("{name}\0{label}"),
                        json!({
                            "kind": "aura_should_refresh", "unit": name, "aura": label,
                            "modes": exclusive_refresh(&env.sim, aura),
                        }),
                    );
                }
            }
            for child in map.values() {
                collect_aura_should_refresh(env, child, target, found);
            }
        }
        Value::Array(children) => {
            for child in children {
                collect_aura_should_refresh(env, child, target, found);
            }
        }
        _ => {}
    }
}

/// The `Judgement of Wisdom (External)` loop over the target's auras: buffs/paladin.go
/// `AttachJudgementOfWisdomMana`.
pub(crate) fn judgement_of_wisdom_effects(env: &Environment) -> Vec<Value> {
    let target = env.encounter.targets[0];
    let rank = paladin::judgement_of_wisdom_max_rank();
    env.sim
        .unit(target)
        .auras
        .iter()
        .filter(|aura| env.sim.aura(**aura).label == "Judgement of Wisdom (External)")
        .map(|aura| {
            json!({
                "kind": "judgement_of_wisdom", "aura": env.sim.aura(*aura).label,
                "proc_chance": paladin::JUDGEMENT_PROC_CHANCE,
                "proc_mask": ProcMask::DIRECT.names(), "mana": rank.value,
                "metrics_action_id": action_id(Some(&ActionId::spell(rank.spell_id))),
                "delay_ns": SPELL_BATCH_WINDOW,
            })
        })
        .collect()
}

/// buffs.go `ApplyFixedShoutAura`: the party's Battle Shout is up for good, through
/// `ApplyFixedUptimeAura`'s rolls: a period of its duration and a nanosecond, and a first try a
/// nanosecond before the pull with a rolled duration. Behind the player's own shout it chains
/// instead: each time the player's own shout gains, the party's comes back a reaction time after
/// it runs out, and once more a duration and a nanosecond later.
pub(crate) fn battle_shout_effect(env: &Environment) -> Option<Value> {
    let player = env.player;
    let aura = env.sim.get_aura(player, "Battle Shout (External)")?;
    let aura = env.sim.aura(aura);
    let mut effect = json!({
        "kind": "fixed_uptime_aura", "aura": aura.label, "uptime": 1.0,
        "tick_length_ns": aura.duration + 1, "start_time_ns": -1,
    });
    for own in env.sim.auras_with_tag(player, BATTLE_SHOUT.category) {
        let own = env.sim.aura(own);
        if own.action_id.as_ref().map_or(0, |id| id.tag) == 0 {
            effect["chained_by"] = json!(own.label);
        }
    }
    Some(effect)
}

/// buffs/drivers.go `driveSunderArmor`: the raid's Sunder Armor ramps to its maximum stacks, one a
/// default GCD from the pull, Go literals. Target armor at each stack count is read from separate
/// reset simulations, since the stacks act through exclusive armor effects. A stronger permanent
/// member of its exclusive category, such as the raid's Expose Armor, blocks every activation,
/// which Go still counts as a proc, and the armor never changes.
pub(crate) fn sunder_armor_effect(
    env: &Environment,
    unrepresented: &mut Vec<String>,
) -> Option<Value> {
    let target = env.encounter.targets[0];
    let label = "Sunder Armor (External)";
    let aura = env.sim.get_aura(target, label)?;
    let blocked = sunder_blocked(env, label);
    let (tag, max_stacks) = {
        let a = env.sim.aura(aura);
        (a.tag.clone(), a.max_stacks)
    };
    for other in &env.sim.unit(target).auras {
        let o = env.sim.aura(*other);
        if *other != aura && o.tag == tag && o.active && o.duration != NEVER_EXPIRES {
            unrepresented.push(format!(
                "{label} shares its category with expiring {}",
                o.label
            ));
        }
    }
    let armor: Vec<f64> = (0..=max_stacks)
        .map(|stacks| target_armor_with_stacks(env, label, if blocked { 0 } else { stacks }))
        .collect();
    let mut ramp = json!({
        "kind": "sunder_armor_ramp", "aura": label, "period_ns": GCD_DEFAULT,
        "ticks": 5, "armor_by_stacks": armor,
    });
    if blocked {
        ramp["blocked"] = json!(true);
    }
    Some(ramp)
}

/// The target's armor with the named aura active at the given stacks, from a separate reset
/// simulation so the exported one is untouched: Go `targetArmorWithStacks`.
fn target_armor_with_stacks(env: &Environment, label: &str, stacks: i32) -> f64 {
    let mut fresh = env.fresh();
    let target = fresh.encounter.targets[0];
    if stacks > 0 {
        let aura = fresh
            .sim
            .get_aura(target, label)
            .expect("the aura of the exported simulation");
        fresh.sim.activate(aura);
        // A stronger aura in the exclusive armor category, such as Expose Armor, keeps it out.
        if !fresh.sim.aura(aura).active {
            return f64::NAN;
        }
        fresh.sim.set_stacks(aura, stacks);
    }
    let unit = fresh.sim.unit(target);
    unit.pseudo_stats.armor_multiplier * unit.stats[super::super::stats::Stat::Armor]
}

/// Whether activating the aura right after a reset fails, as an exclusive category with a stronger
/// active member makes it: Go `sunderBlocked`.
fn sunder_blocked(env: &Environment, label: &str) -> bool {
    let mut fresh = env.fresh();
    let target = fresh.encounter.targets[0];
    let aura = fresh
        .sim
        .get_aura(target, label)
        .expect("the aura of the exported simulation");
    fresh.sim.activate(aura);
    !fresh.sim.aura(aura).active
}
