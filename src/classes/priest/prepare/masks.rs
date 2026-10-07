//! Go's Priest class masks, from sim/priest/priest.go: the single bits and the unions the talents
//! and spells name. The bit positions are Go's, as its `iota` numbers them (the first constant,
//! `PriestSpellFlagNone`, takes iota 0, so Devouring Plague is bit 1).

pub(crate) const DEVOURING_PLAGUE: i64 = 1 << 1;
pub(crate) const DEVOURING_PLAGUE_DOT: i64 = 1 << 2;
pub(crate) const DEVOURING_PLAGUE_HEAL: i64 = 1 << 3;
pub(crate) const HOLY_NOVA: i64 = 1 << 4;
pub(crate) const HOLY_FIRE: i64 = 1 << 5;
pub(crate) const MIND_BLAST: i64 = 1 << 6;
pub(crate) const MIND_FLAY: i64 = 1 << 7;
pub(crate) const PENANCE: i64 = 1 << 8;
pub(crate) const POWER_INFUSION: i64 = 1 << 9;
pub(crate) const STARSHARDS: i64 = 1 << 10;
pub(crate) const SHADOWFORM: i64 = 1 << 11;
pub(crate) const SHADOW_WORD_DEATH: i64 = 1 << 12;
pub(crate) const SHADOW_WORD_PAIN: i64 = 1 << 13;
pub(crate) const SHADOWFIEND: i64 = 1 << 14;
pub(crate) const VAMPIRIC_EMBRACE: i64 = 1 << 15;
pub(crate) const FADE: i64 = 1 << 16;
pub(crate) const SMITE: i64 = 1 << 17;
pub(crate) const CHASTISE: i64 = 1 << 18;
pub(crate) const CONFOUNDING_FLASH: i64 = 1 << 19;
pub(crate) const CONTINGENCY_PLAN: i64 = 1 << 20;
pub(crate) const DARK_SACRIFICE: i64 = 1 << 21;
pub(crate) const DIVINE_GRACE: i64 = 1 << 22;

/// `PriestSpellLast`.
const LAST: i64 = 1 << 23;
/// `PriestSpellsAll`.
pub(crate) const ALL: i64 = (LAST << 1) - 1;
/// `PriestShadowSpells`.
pub(crate) const SHADOW_SPELLS: i64 = DEVOURING_PLAGUE
    | SHADOW_WORD_DEATH
    | SHADOWFORM
    | SHADOW_WORD_PAIN
    | MIND_FLAY
    | MIND_BLAST
    | SHADOWFIEND
    | VAMPIRIC_EMBRACE;
