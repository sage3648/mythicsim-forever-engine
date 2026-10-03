//! Go sim/mage/mage.go class masks, as the class spell names the exporter writes.

/// Go `MageSpellsAllDamaging`.
pub(crate) const DAMAGING: &[&str] = &[
    "arcane_blast",
    "arcane_explosion",
    "arcane_missiles_tick",
    "blizzard",
    "fire_blast",
    "fireball",
    "flamestrike",
    "frostbolt",
    "ice_lance",
    "pyroblast",
    "pyroblast_dot",
    "scorch",
    "blast_wave",
    "cone_of_cold",
    "frost_nova",
    "frostfire_bolt",
];

/// Go `MageSpellsAll`: every Mage class spell.
pub(crate) const ALL: &[&str] = &[
    "arcane_blast",
    "arcane_explosion",
    "arcane_power",
    "arcane_missiles_cast",
    "arcane_missiles_tick",
    "blast_wave",
    "blizzard",
    "cold_snap",
    "cone_of_cold",
    "evocation",
    "fire_blast",
    "fireball",
    "flamestrike",
    "flamestrike_dot",
    "frost_armor",
    "frostbolt",
    "frost_nova",
    "ice_barrier",
    "ice_block",
    "ice_lance",
    "ignite",
    "mage_armor",
    "mana_gems",
    "molten_armor",
    "presence_of_mind",
    "pyroblast",
    "pyroblast_dot",
    "scorch",
    "mana_gem",
    "combustion",
    "improved_blizzard",
    "frostfire_bolt",
];

/// Go `MageSpellInstantCast`.
pub(crate) const INSTANT_CAST: &[&str] = &[
    "arcane_missiles_cast",
    "arcane_missiles_tick",
    "fire_blast",
    "arcane_explosion",
    "pyroblast_dot",
    "combustion",
    "cone_of_cold",
    "ice_lance",
    "mana_gems",
    "presence_of_mind",
];

/// Whether a spell's class mask is in a set.
pub(crate) fn is_class(class: Option<&str>, set: &[&str]) -> bool {
    class.is_some_and(|class| set.contains(&class))
}

/// The damaging spells outside `excluded`, as Go writes `MageSpellsAllDamaging &^ mask`.
pub(crate) fn damaging_except(excluded: &[&str]) -> Vec<&'static str> {
    except(DAMAGING, excluded)
}

/// The spells of `set` outside `excluded`, as Go writes `set &^ mask`.
pub(crate) fn except(set: &[&'static str], excluded: &[&str]) -> Vec<&'static str> {
    set.iter()
        .copied()
        .filter(|class| !excluded.contains(class))
        .collect()
}
