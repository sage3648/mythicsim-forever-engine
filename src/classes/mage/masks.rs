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

/// Whether a spell's class mask is in a set.
pub(crate) fn is_class(class: Option<&str>, set: &[&str]) -> bool {
    class.is_some_and(|class| set.contains(&class))
}

/// The damaging spells outside `excluded`, as Go writes `MageSpellsAllDamaging &^ mask`.
pub(crate) fn damaging_except(excluded: &[&str]) -> Vec<&'static str> {
    DAMAGING
        .iter()
        .copied()
        .filter(|class| !excluded.contains(class))
        .collect()
}
