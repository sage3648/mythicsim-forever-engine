//! Go's Shaman class masks, from sim/shaman/shaman.go: the single bits and the unions the
//! talents and spells name. The bit positions are Go's, as its `iota` numbers them (the const
//! block starts with `SpellMaskNone` at 0, so the first spell is bit 1).

pub(crate) const FLAME_SHOCK_DIRECT: i64 = 1 << 1;
pub(crate) const FLAME_SHOCK_DOT: i64 = 1 << 2;
pub(crate) const LIGHTNING_BOLT: i64 = 1 << 3;
pub(crate) const LIGHTNING_BOLT_OVERLOAD: i64 = 1 << 4;
pub(crate) const CHAIN_LIGHTNING: i64 = 1 << 5;
pub(crate) const CHAIN_LIGHTNING_OVERLOAD: i64 = 1 << 6;
pub(crate) const EARTH_SHOCK: i64 = 1 << 7;
pub(crate) const LIGHTNING_SHIELD: i64 = 1 << 8;
pub(crate) const MAGMA_TOTEM: i64 = 1 << 9;
pub(crate) const SEARING_TOTEM: i64 = 1 << 10;
pub(crate) const FIRE_NOVA: i64 = 1 << 11;
pub(crate) const FLAMETONGUE_TOTEM: i64 = 1 << 12;
pub(crate) const STORMSTRIKE_CAST: i64 = 1 << 13;
pub(crate) const STORMSTRIKE_DAMAGE: i64 = 1 << 14;
pub(crate) const EARTH_SHIELD: i64 = 1 << 15;
pub(crate) const FROST_SHOCK: i64 = 1 << 16;
pub(crate) const FLAMETONGUE_WEAPON: i64 = 1 << 17;
pub(crate) const WINDFURY_WEAPON: i64 = 1 << 18;
pub(crate) const FROSTBRAND_WEAPON: i64 = 1 << 19;
pub(crate) const ROCKBITER_WEAPON: i64 = 1 << 20;
pub(crate) const ELEMENTAL_MASTERY: i64 = 1 << 21;
pub(crate) const SHAMANISTIC_RAGE: i64 = 1 << 22;
pub(crate) const BASIC_TOTEM: i64 = 1 << 23;
pub(crate) const SHIELD_SELF_PROC: i64 = 1 << 24;
pub(crate) const LAVA_BURST: i64 = 1 << 25;

pub(crate) const FLAME_SHOCK: i64 = FLAME_SHOCK_DIRECT | FLAME_SHOCK_DOT;
pub(crate) const FIRE: i64 = FLAME_SHOCK | FIRE_NOVA | LAVA_BURST;
pub(crate) const NATURE: i64 = LIGHTNING_BOLT
    | LIGHTNING_BOLT_OVERLOAD
    | CHAIN_LIGHTNING
    | CHAIN_LIGHTNING_OVERLOAD
    | EARTH_SHOCK;
pub(crate) const FROST: i64 = FROST_SHOCK;
pub(crate) const OVERLOAD: i64 = LIGHTNING_BOLT_OVERLOAD | CHAIN_LIGHTNING_OVERLOAD;
pub(crate) const SHOCK: i64 = FLAME_SHOCK | EARTH_SHOCK | FROST_SHOCK;
pub(crate) const FIRE_TOTEM: i64 = MAGMA_TOTEM | SEARING_TOTEM;
pub(crate) const TOTEM: i64 = FIRE_TOTEM | FLAMETONGUE_TOTEM | BASIC_TOTEM;
#[allow(dead_code)]
pub(crate) const IMBUE: i64 =
    FROSTBRAND_WEAPON | WINDFURY_WEAPON | FLAMETONGUE_WEAPON | ROCKBITER_WEAPON;

/// `stormstrikeSpells`: what the debuff raises and what spends its charges.
#[allow(dead_code)]
pub(crate) const STORMSTRIKE_SPELLS: i64 =
    LIGHTNING_BOLT | CHAIN_LIGHTNING | EARTH_SHOCK | OVERLOAD;

/// The spells Clearcasting makes free (applyElementalFocus's `canConsumeSpells`).
pub(crate) const CLEARCASTING_SPELLS: i64 =
    LIGHTNING_BOLT | CHAIN_LIGHTNING | LAVA_BURST | FIRE_NOVA | (SHOCK & !FLAME_SHOCK_DOT);
