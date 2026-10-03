//! Go sim/shaman/shaman.go class masks, as the class spell names the exporter writes.

/// The spells Clearcasting from Elemental Focus makes free and is spent by: Go
/// `SpellMaskLightningBolt | SpellMaskChainLightning | SpellMaskLavaBurst | SpellMaskFireNova
/// | (SpellMaskShock &^ SpellMaskFlameShockDot)`.
pub(crate) const FOCUS_CONSUMERS: &[&str] = &[
    "lightning_bolt",
    "chain_lightning",
    "lava_burst",
    "fire_nova",
    "flame_shock_direct",
    "earth_shock",
    "frost_shock",
];

/// Spells whose casts never proc the shaman's talents: Go `SpellMaskFireTotem |
/// SpellMaskFlameShockDot`.
pub(crate) const TOTEM_OR_FLAME_SHOCK_DOT: &[&str] =
    &["magma_totem", "searing_totem", "flame_shock_dot"];

/// Go `SpellSchoolElemental`: Fire, Nature and Frost.
pub(crate) const SCHOOL_ELEMENTAL: u8 = 4 | 8 | 16;

/// Whether a spell's class mask is in a set.
pub(crate) fn is_class(class: Option<&str>, set: &[&str]) -> bool {
    class.is_some_and(|class| set.contains(&class))
}

/// The spells Stormstrike's debuff raises and that spend its charges: Go `stormstrikeSpells`.
pub(crate) const STORMSTRIKE_SPELLS: &[&str] = &[
    "lightning_bolt",
    "chain_lightning",
    "earth_shock",
    "lightning_bolt_overload",
    "chain_lightning_overload",
];

/// Spells whose hardcast holds the melee swing: Go `holdMeleeForCast` as `ModifyCast`.
pub(crate) const HOLDS_MELEE: &[&str] = &["lightning_bolt", "chain_lightning", "lava_burst"];
