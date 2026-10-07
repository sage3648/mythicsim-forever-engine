//! Go sim/core/spelldata/item_aura.go: what an item's aura effect lands on.
//!
//! `AuraTarget`, `Effect.AuraTarget` and `EffectsOn` live with the effect accessors in
//! `spelldata/effect.rs`. `ItemAuraUnsupported` is the generator's audit of a row and is not part
//! of preparation.

use crate::data::spells::Spell;

use super::dbcenums;
use super::spelldata::find;

/// Go `EquipAuraRow`: the row an equip spell keeps on its targets: the spell itself, or the one
/// its only effect casts on a period short enough to keep it up. Beastmaster's Boots' 27206
/// casts 27205, a 4 s aura on the pet, every 3 s.
pub(crate) fn equip_aura_row(s: &'static Spell) -> &'static Spell {
    if s.effects.len() != 1 {
        return s;
    }
    let e = &s.effects[0];
    let kept = find(e.trigger_id);
    if e.aura != dbcenums::A_PERIODIC_TRIGGER_SPELL
        || e.period_ms <= 0
        || kept.is_nil()
        || (kept.duration_ms >= 0 && kept.duration_ms < e.period_ms)
    {
        return s;
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prepare::spelldata::must_find;

    #[test]
    fn an_equip_spell_that_recasts_a_longer_aura_is_read_as_that_aura() {
        // Beastmaster's Boots: 27206 casts 27205 every 3 s, and 27205 lasts 4 s.
        assert_eq!(equip_aura_row(must_find(27206)).id, 27205);
        // A spell with one aura effect keeps its own row.
        assert_eq!(equip_aura_row(must_find(1291908)).id, 1291908);
    }
}
