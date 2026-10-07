//! Go tables preparation reads, from `data/go-tables.json`.

use std::{collections::BTreeMap, sync::OnceLock};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Tables {
    /// core.BaseStats: class, then race, then the nonzero stats by Go stat name.
    pub base_stats: BTreeMap<String, BTreeMap<String, BTreeMap<String, f64>>>,
    pub crit_per_agi_max_level: BTreeMap<String, f64>,
    pub crit_per_int_max_level: BTreeMap<String, f64>,
    /// Items whose effects Go registers in code.
    pub item_effect_ids: Vec<i32>,
    /// Enchants whose effects Go registers in code.
    pub enchant_effect_ids: Vec<i32>,
    /// Preset target IDs whose target has an AI.
    pub preset_targets_with_ai: Vec<i32>,
    /// The item sets Go registers, in the order its set bonus search reads them.
    pub item_sets: Vec<ItemSetRow>,
}

/// A set `core.NewItemSet` registered: the names its items carry and the piece counts at which
/// it has a bonus.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ItemSetRow {
    pub id: i32,
    pub name: String,
    pub alternative_name: String,
    pub bonus_pieces: Vec<i32>,
}

pub(crate) fn tables() -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        serde_json::from_str(include_str!("../../data/go-tables.json"))
            .expect("data/go-tables.json parses")
    })
}
