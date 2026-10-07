//! tools/oracle-v2/melee_procs.go `characterStatAuras` and `statAurasEffect`: the stats that
//! change during a fight because an aura's gain and expiry change them through
//! `AddStatsDynamic`, read as a function of which of those auras are active.

use serde_json::{json, Map, Value};

use super::env::Environment;
use super::sim::UnitId;
use super::stats::Stat;
use super::Refusal;

/// The combination key of Go `Character.SpiritManaRegenPerSecond`, which is not a stat.
const SPIRIT_REGEN_KEY: &str = "SpiritManaRegenPerSecond";

/// The stats the Rust runtime reads during a fight.
const DYNAMIC_READ_STATS: [Stat; 6] = [
    Stat::SpellDamage,
    Stat::AttackPower,
    Stat::RangedAttackPower,
    Stat::SpellCritPercent,
    Stat::PhysicalCritPercent,
    Stat::MP5,
];

/// Stats the Rust runtime reads during a fight that a combination carries only when one changes
/// them: maximum mana, healing power and health, Spirit, the school spell damage stats and the
/// resistances a spell that hits the player rolls against.
const OPTIONAL_READ_STATS: [Stat; 15] = [
    Stat::Mana,
    Stat::HealingPower,
    Stat::Health,
    Stat::Spirit,
    Stat::ArcaneDamage,
    Stat::FireDamage,
    Stat::FrostDamage,
    Stat::HolyDamage,
    Stat::NatureDamage,
    Stat::ShadowDamage,
    Stat::ArcaneResistance,
    Stat::FireResistance,
    Stat::FrostResistance,
    Stat::NatureResistance,
    Stat::ShadowResistance,
];

/// Auras of races, items and raid buffs whose gain and expiry change stats through
/// `AddStatsDynamic`. A class adds its own through `PrepAgent::stat_auras`.
const COMMON_STAT_AURA_LABELS: [&str; 9] = [
    "Blood Fury",
    "Elune's Light",
    "Holy Strength (MH)",
    "Holy Strength (OH)",
    "Windfury Totem (External)",
    "Battle Shout (External)",
    "Headmaster's Charge",
    "Crusader's Wrath",
    "Diamond Flask",
];

/// STUB for the consumables port: the stat buff auras of the potions in the spellbook
/// (consumes.go: a potion's stat buff is a temporary stats aura named for the potion). An
/// equipped consumable is refused until consumables are prepared, so no potion reaches here
/// yet; the consumables port fills this in.
fn potion_stat_auras(_env: &Environment) -> Vec<String> {
    Vec::new()
}

/// STUB for the items port: the auras of `spellDataStatProcAuras`, item procs built from client
/// spell data whose buff changes stats. An item effect not ported yet is refused, so none reaches
/// here yet; the items port fills this in.
fn spell_data_stat_proc_auras(_env: &Environment) -> Vec<String> {
    Vec::new()
}

/// STUB for the items port: `lionHornProcAura`, The Lion Horn of Stormwind's proc aura.
fn lion_horn_proc_aura(_env: &Environment) -> Vec<String> {
    Vec::new()
}

/// STUB for the items port: the related self buffs of the spells `simpleStatActive` recognizes,
/// the on-use stat buffs `shared.NewSimpleStatActive` registers.
fn simple_stat_active_auras(_env: &Environment) -> Vec<String> {
    Vec::new()
}

/// Go `characterStatAuras`: the stat auras of the player, in the order a combination's bits
/// number them.
pub(crate) fn character_stat_auras(env: &Environment) -> Vec<String> {
    let mut candidates: Vec<String> = COMMON_STAT_AURA_LABELS
        .iter()
        .map(|label| label.to_string())
        .collect();
    candidates.extend(potion_stat_auras(env));
    candidates.extend(env.agent.stat_auras(&env.sim, env.player));
    candidates.extend(spell_data_stat_proc_auras(env));
    candidates.extend(lion_horn_proc_aura(env));
    candidates.extend(simple_stat_active_auras(env));
    candidates
        .into_iter()
        .filter(|label| env.sim.get_aura(env.player, label).is_some())
        .collect()
}

/// The name to value map Go's `statValues` builds.
fn stat_map(env: &Environment, unit: UnitId) -> Vec<(&'static str, f64)> {
    Stat::ALL
        .iter()
        .map(|stat| (stat.name(), env.sim.unit(unit).stats[*stat]))
        .collect()
}

/// Go `statAurasEffect`: each combination of the labels' auras is read from a separate reset
/// simulation with exactly those auras active: combination i has aura j active when bit j is set,
/// and an aura active after the reset, as a druid's starting form, is deactivated when its bit is
/// clear.
pub(crate) fn stat_auras_effect(env: &Environment) -> Result<Option<Value>, Refusal> {
    let labels = character_stat_auras(env);
    if labels.is_empty() {
        return Ok(None);
    }
    if labels.len() > 10 {
        return Err(Refusal::new(
            "stat_auras",
            format!("{} stat auras exceed the combination limit", labels.len()),
        ));
    }
    let mut combos: Vec<Map<String, Value>> = Vec::new();
    let mut spirit_regens: Vec<f64> = Vec::new();
    let mut changed_spirit_regen = false;
    let mut changed: Vec<&'static str> = Vec::new();
    let mut base: Vec<(&'static str, f64)> = Vec::new();
    for mask in 0..(1usize << labels.len()) {
        let mut fresh = env.fresh();
        let player = fresh.player;
        // An aura up from the reset, such as the default stance, is down where its bit is clear.
        for (bit, label) in labels.iter().enumerate() {
            let aura = fresh.sim.get_aura(player, label).expect("the aura exists");
            if mask & (1 << bit) == 0 && fresh.sim.aura(aura).active {
                fresh.sim.deactivate(aura);
            }
        }
        for (bit, label) in labels.iter().enumerate() {
            let aura = fresh.sim.get_aura(player, label).expect("the aura exists");
            let want = mask & (1 << bit) != 0;
            let active = fresh.sim.aura(aura).active;
            if want && !active {
                fresh.sim.activate(aura);
            } else if !want && active {
                fresh.sim.deactivate(aura);
            }
        }
        let values = stat_map(&fresh, player);
        if mask == 0 {
            base = values.clone();
        }
        let mut combo = Map::new();
        for stat in DYNAMIC_READ_STATS {
            combo.insert(
                stat.name().to_string(),
                json!(fresh.sim.unit(player).stats[stat]),
            );
        }
        for stat in OPTIONAL_READ_STATS {
            combo.insert(
                stat.name().to_string(),
                json!(fresh.sim.unit(player).stats[stat]),
            );
        }
        // Spirit regeneration follows Intellect and Spirit, which mana.go UpdateManaRegenRates
        // rereads when a stat aura changes either.
        let character = fresh.sim.character(player);
        let spirit_regen = fresh.sim.unit(player).stats[Stat::Spirit].mul_add(
            character.spirit_regen_per_spirit,
            character.spirit_regen_base,
        );
        combo.insert(SPIRIT_REGEN_KEY.to_string(), json!(spirit_regen));
        if mask > 0 && spirit_regen != spirit_regens[0] {
            changed_spirit_regen = true;
        }
        spirit_regens.push(spirit_regen);
        combos.push(combo);
        for ((name, value), (_, base_value)) in values.iter().zip(&base) {
            if value != base_value && !changed.contains(name) {
                changed.push(name);
            }
        }
    }
    changed.sort_unstable();
    // The optional stats stay out of the combinations unless one changes them, and so does
    // spirit regeneration.
    for stat in OPTIONAL_READ_STATS {
        if !changed.contains(&stat.name()) {
            for combo in &mut combos {
                combo.remove(stat.name());
            }
        }
    }
    if !changed_spirit_regen {
        for combo in &mut combos {
            combo.remove(SPIRIT_REGEN_KEY);
        }
    }
    Ok(Some(json!({"kind": "stat_auras", "auras": labels,
        "combos": combos, "changed": changed})))
}
