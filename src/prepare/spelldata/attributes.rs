//! Go `spelldata/attributes.go`: the named attribute bits a spell row carries.

use super::super::dbcenums;
use super::Spell;

impl Spell {
    /// Whether the spell sets `bit` in `Attributes[word]`. A word outside the client's 17 reads
    /// as unset. (Go takes an `int`, so a negative word reads as unset too; `usize` cannot be
    /// negative.)
    pub(crate) fn has_attr(&self, word: usize, bit: u32) -> bool {
        match self.attr.get(word) {
            Some(attributes) => attributes & bit != 0,
            None => false,
        }
    }

    pub(crate) fn is_passive(&self) -> bool {
        self.has_attr(dbcenums::ATTR_INDEX_BASE, dbcenums::ATTR_PASSIVE)
    }

    pub(crate) fn not_shapeshifted(&self) -> bool {
        self.has_attr(dbcenums::ATTR_INDEX_BASE, dbcenums::ATTR_NOT_SHAPESHIFTED)
    }

    pub(crate) fn castable_in_caster_form(&self) -> bool {
        self.has_attr(
            dbcenums::ATTR_INDEX_EX_2,
            dbcenums::ATTR_EX_2_CASTABLE_IN_CASTER_FORM,
        )
    }

    pub(crate) fn is_channeled(&self) -> bool {
        self.has_attr(
            dbcenums::ATTR_INDEX_EX_1,
            dbcenums::ATTR_EX_1_IS_CHANNELLED | dbcenums::ATTR_EX_1_IS_SELF_CHANNELLED,
        )
    }

    /// Whether damage taken while casting the spell pushes its cast back.
    pub(crate) fn pushed_back(&self) -> bool {
        self.interrupt_flags & dbcenums::SPELL_INTERRUPT_FLAG_PUSHBACK != 0
    }

    pub(crate) fn is_bleed(&self) -> bool {
        self.mechanic == dbcenums::MECHANIC_BLEED
    }

    pub(crate) fn refunds_on_miss(&self) -> bool {
        self.has_attr(
            dbcenums::ATTR_INDEX_EX_1,
            dbcenums::ATTR_EX_1_DISCOUNT_POWER_ON_MISS,
        )
    }

    pub(crate) fn periodic_can_crit(&self) -> bool {
        self.has_attr(
            dbcenums::ATTR_INDEX_EX_8,
            dbcenums::ATTR_EX_8_PERIODIC_CAN_CRIT,
        )
    }

    pub(crate) fn can_proc_from_procs(&self) -> bool {
        self.has_attr(
            dbcenums::ATTR_INDEX_EX_3,
            dbcenums::ATTR_EX_3_CAN_PROC_FROM_PROCS,
        )
    }

    pub(crate) fn class_spells_only(&self) -> bool {
        self.has_attr(
            dbcenums::ATTR_INDEX_EX_12,
            dbcenums::ATTR_EX_12_ONLY_PROC_FROM_CLASS_ABILITIES,
        )
    }

    /// Whether the client bars the spell from missing (and from being dodged, parried or
    /// blocked).
    pub(crate) fn always_hits(&self) -> bool {
        self.has_attr(dbcenums::ATTR_INDEX_EX_3, dbcenums::ATTR_EX_3_ALWAYS_HIT)
    }

    /// Whether the client bars the spell from critting, which picks the no-crit hit table.
    pub(crate) fn cannot_crit(&self) -> bool {
        self.has_attr(dbcenums::ATTR_INDEX_EX_2, dbcenums::ATTR_EX_2_CANT_CRIT)
    }

    /// Whether the spell's hits count as a proc, which is what a listener that cannot proc from
    /// procs skips. The client states the negative, so this answers it as the sim's
    /// `SpellFlagProc` reads.
    pub(crate) fn is_a_proc(&self) -> bool {
        !self.has_attr(dbcenums::ATTR_INDEX_EX_3, dbcenums::ATTR_EX_3_NOT_A_PROC)
    }

    /// Whether the client flags the spell No Threat: nothing it does, mana it restores included,
    /// puts the caster on a threat table.
    pub(crate) fn no_threat(&self) -> bool {
        self.has_attr(dbcenums::ATTR_INDEX_EX_1, dbcenums::ATTR_EX_1_NO_THREAT)
    }

    pub(crate) fn suppresses_weapon_procs(&self) -> bool {
        self.has_attr(
            dbcenums::ATTR_INDEX_EX_4,
            dbcenums::ATTR_EX_4_SUPPRESS_WEAPON_PROCS,
        )
    }

    /// Whether the spell's aura hears hits the way a weapon proc does, which means skipping the
    /// hits of a spell flagged Suppress Weapon Procs. The flag sits on the listener;
    /// [`Spell::suppresses_weapon_procs`] sits on the spell whose hits are skipped.
    pub(crate) fn is_weapon_proc_aura(&self) -> bool {
        self.has_attr(
            dbcenums::ATTR_INDEX_EX_6,
            dbcenums::ATTR_EX_6_AURA_IS_WEAPON_PROC,
        )
    }

    /// The share of the cost a miss refunds, for `RageCostOptions.Refund`: 80% where the client
    /// flags Discount Power On Miss, nothing otherwise.
    pub(crate) fn miss_refund(&self) -> f64 {
        if self.refunds_on_miss() {
            return 0.8;
        }
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::spelldata::{nil, store::must_find};

    /// Go `TestAttributes`.
    #[test]
    fn named_accessors_read_their_word() {
        let mut attr = [0u32; 17];
        attr[1] = dbcenums::ATTR_EX_1_DISCOUNT_POWER_ON_MISS;
        let refunding = Spell {
            attr,
            ..Spell::default()
        };
        assert!(refunding.refunds_on_miss());
        assert_eq!(refunding.miss_refund(), 0.8);
        assert!(!must_find(116).refunds_on_miss());
        assert_eq!(must_find(116).miss_refund(), 0.0);

        let mut attr = [0u32; 17];
        attr[1] = dbcenums::ATTR_EX_1_IS_SELF_CHANNELLED;
        let channel = Spell {
            attr,
            ..Spell::default()
        };
        assert!(channel.is_channeled());
        assert!(!nil().is_passive());
        assert!(!nil().has_attr(20, 1));
        // The zero spell is a proc: the client states the negative.
        assert!(nil().is_a_proc());
    }

    #[test]
    fn real_rows() {
        // Frostbolt pushes back and does not channel.
        assert!(must_find(116).pushed_back());
        assert!(!must_find(116).is_channeled());
        // Blizzard rank 1 channels.
        assert!(must_find(10).is_channeled());
    }
}
