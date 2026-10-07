//! Go's Mage class masks, from sim/mage/mage.go: the single bits and the unions the talents
//! and spells name. The bit positions are Go's, as its `iota` numbers them.

pub(crate) const ARCANE_BLAST: i64 = 1 << 1;
pub(crate) const ARCANE_EXPLOSION: i64 = 1 << 2;
pub(crate) const ARCANE_POWER: i64 = 1 << 3;
pub(crate) const ARCANE_MISSILES_CAST: i64 = 1 << 4;
pub(crate) const ARCANE_MISSILES_TICK: i64 = 1 << 5;
pub(crate) const BLAST_WAVE: i64 = 1 << 6;
pub(crate) const BLIZZARD: i64 = 1 << 7;
pub(crate) const COLD_SNAP: i64 = 1 << 8;
pub(crate) const CONE_OF_COLD: i64 = 1 << 9;
pub(crate) const EVOCATION: i64 = 1 << 10;
pub(crate) const FIRE_BLAST: i64 = 1 << 11;
pub(crate) const FIREBALL: i64 = 1 << 12;
pub(crate) const FLAMESTRIKE: i64 = 1 << 13;
pub(crate) const FLAMESTRIKE_DOT: i64 = 1 << 14;
pub(crate) const FROST_ARMOR: i64 = 1 << 15;
pub(crate) const FROSTBOLT: i64 = 1 << 16;
pub(crate) const FROST_NOVA: i64 = 1 << 17;
pub(crate) const ICE_BARRIER: i64 = 1 << 18;
pub(crate) const ICE_BLOCK: i64 = 1 << 19;
pub(crate) const ICE_LANCE: i64 = 1 << 20;
pub(crate) const IGNITE: i64 = 1 << 21;
pub(crate) const MAGE_ARMOR: i64 = 1 << 22;
pub(crate) const MANA_GEMS: i64 = 1 << 23;
pub(crate) const MOLTEN_ARMOR: i64 = 1 << 24;
pub(crate) const PRESENCE_OF_MIND: i64 = 1 << 25;
pub(crate) const PYROBLAST: i64 = 1 << 26;
pub(crate) const PYROBLAST_DOT: i64 = 1 << 27;
pub(crate) const SCORCH: i64 = 1 << 28;
pub(crate) const MANA_GEM: i64 = 1 << 29;
pub(crate) const COMBUSTION: i64 = 1 << 30;
pub(crate) const IMPROVED_BLIZZARD: i64 = 1 << 31;
pub(crate) const FROSTFIRE_BOLT: i64 = 1 << 32;

/// `MageSpellLast`.
const LAST: i64 = 1 << 33;
/// `MageSpellsAll`.
pub(crate) const ALL: i64 = (LAST << 1) - 1;
/// `MageSpellsAllDamaging`.
pub(crate) const ALL_DAMAGING: i64 = ARCANE_BLAST
    | ARCANE_EXPLOSION
    | ARCANE_MISSILES_TICK
    | BLIZZARD
    | FIRE_BLAST
    | FIREBALL
    | FLAMESTRIKE
    | FROSTBOLT
    | ICE_LANCE
    | PYROBLAST
    | PYROBLAST_DOT
    | SCORCH
    | BLAST_WAVE
    | CONE_OF_COLD
    | FROST_NOVA
    | FROSTFIRE_BOLT;
/// `MageSpellInstantCast`.
pub(crate) const INSTANT_CAST: i64 = ARCANE_MISSILES_CAST
    | ARCANE_MISSILES_TICK
    | FIRE_BLAST
    | ARCANE_EXPLOSION
    | PYROBLAST_DOT
    | COMBUSTION
    | CONE_OF_COLD
    | ICE_LANCE
    | MANA_GEMS
    | PRESENCE_OF_MIND;
/// `MageSpellArcaneMissiles`.
pub(crate) const ARCANE_MISSILES: i64 = ARCANE_MISSILES_CAST | ARCANE_MISSILES_TICK;
/// `MageSpellChill`.
pub(crate) const CHILL: i64 = FROSTBOLT | CONE_OF_COLD | FROSTFIRE_BOLT | IMPROVED_BLIZZARD;
