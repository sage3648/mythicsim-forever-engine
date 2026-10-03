//! Strict prepared v1 wire types. Character preparation is outside this contract.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema_version: u32,
    pub source_revision: String,
    pub scenario_id: String,
    pub iterations: u32,
    pub seed: u64,
    pub duration_ns: u64,
    pub reaction_ns: u64,
    pub caster: Caster,
    pub target: Target,
    pub spell: Spell,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Caster {
    pub level: u32,
    pub max_mana: f64,
    pub spell_power: f64,
    pub hit_percent: f64,
    pub crit_percent: f64,
    pub spell_penetration: f64,
    pub regen_casting_per_second: f64,
    pub regen_idle_per_second: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    pub level: u32,
    pub frost_resistance: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Spell {
    pub id: u32,
    pub min_damage: f64,
    pub max_damage: f64,
    pub coefficient: f64,
    pub damage_multiplier: f64,
    pub crit_multiplier: f64,
    pub mana_cost: f64,
    pub cast_ns: u64,
    pub gcd_ns: u64,
    pub travel_ns: u64,
}
