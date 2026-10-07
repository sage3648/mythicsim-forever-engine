//! Go's Warlock class masks, from sim/warlock/warlock.go: the single bits and the unions the
//! talents and spells name. The bit positions are Go's, as its `iota` numbers them (the const
//! block starts at 0 with `WarlockSpellFlagNone`, so the first spell is bit 1).

pub(crate) const CONFLAGRATE: i64 = 1 << 1;
pub(crate) const SHADOW_BOLT: i64 = 1 << 2;
pub(crate) const IMMOLATE: i64 = 1 << 3;
pub(crate) const IMMOLATE_DOT: i64 = 1 << 4;
pub(crate) const INCINERATE: i64 = 1 << 5;
pub(crate) const SOUL_FIRE: i64 = 1 << 6;
pub(crate) const SHADOW_BURN: i64 = 1 << 7;
pub(crate) const LIFE_TAP: i64 = 1 << 8;
pub(crate) const CORRUPTION: i64 = 1 << 9;
pub(crate) const CURSE_OF_AGONY: i64 = 1 << 10;
pub(crate) const CURSE_OF_ELEMENTS: i64 = 1 << 11;
pub(crate) const DRAIN_LIFE: i64 = 1 << 12;
pub(crate) const HELLFIRE: i64 = 1 << 13;
pub(crate) const IMMOLATION_AURA: i64 = 1 << 14;
pub(crate) const SEARING_PAIN: i64 = 1 << 15;
pub(crate) const SUMMON_DOOMGUARD: i64 = 1 << 16;
pub(crate) const DOOMGUARD_DOOM_BOLT: i64 = 1 << 17;
pub(crate) const SUMMON_IMP: i64 = 1 << 18;
pub(crate) const IMP_FIRE_BOLT: i64 = 1 << 19;
pub(crate) const SUMMON_FELHUNTER: i64 = 1 << 20;
pub(crate) const FELHUNTER_SHADOW_BITE: i64 = 1 << 21;
pub(crate) const SUMMON_SUCCUBUS: i64 = 1 << 22;
pub(crate) const SUCCUBUS_LASH_OF_PAIN: i64 = 1 << 23;
pub(crate) const VOIDWALKER_TORMENT: i64 = 1 << 24;
pub(crate) const SUMMON_INFERNAL: i64 = 1 << 25;
pub(crate) const RAIN_OF_FIRE: i64 = 1 << 26;
pub(crate) const CURSE_OF_DOOM: i64 = 1 << 27;
pub(crate) const CURSE_OF_RECKLESSNESS: i64 = 1 << 28;
pub(crate) const CURSE_OF_WEAKNESS: i64 = 1 << 29;
pub(crate) const SIPHON_LIFE: i64 = 1 << 30;
pub(crate) const DRAIN_SOUL: i64 = 1 << 31;
pub(crate) const DEATH_COIL: i64 = 1 << 32;
pub(crate) const WRACK: i64 = 1 << 33;

/// `WarlockSpellAll`: `1<<iota - 1`, with iota at 34.
pub(crate) const ALL: i64 = (1 << 34) - 1;

/// `WarlockShadowDamage`.
pub(crate) const SHADOW_DAMAGE: i64 = CORRUPTION
    | DRAIN_LIFE
    | CURSE_OF_AGONY
    | CURSE_OF_DOOM
    | SHADOW_BOLT
    | SHADOW_BURN
    | SIPHON_LIFE
    | DEATH_COIL
    | DRAIN_SOUL
    | WRACK;

/// `WarlockPeriodicShadowDamage`.
pub(crate) const PERIODIC_SHADOW_DAMAGE: i64 =
    CORRUPTION | DRAIN_LIFE | CURSE_OF_AGONY | CURSE_OF_DOOM | SIPHON_LIFE | DRAIN_SOUL | WRACK;

/// `WarlockDrainSpells`: the drain effects Improved Drains and Soul Siphon pay out on.
pub(crate) const DRAIN_SPELLS: i64 = DRAIN_LIFE | DRAIN_SOUL | WRACK;

/// `WarlockNightfallSpells`: Nightfall rolls off the periodic damage of these.
pub(crate) const NIGHTFALL_SPELLS: i64 = CORRUPTION | DRAIN_LIFE | DRAIN_SOUL | WRACK;

/// `WarlockDestructionSpells`.
pub(crate) const DESTRUCTION_SPELLS: i64 = HELLFIRE
    | IMMOLATE
    | IMMOLATE_DOT
    | INCINERATE
    | RAIN_OF_FIRE
    | SEARING_PAIN
    | SHADOW_BOLT
    | SOUL_FIRE
    | CONFLAGRATE
    | SHADOW_BURN;
