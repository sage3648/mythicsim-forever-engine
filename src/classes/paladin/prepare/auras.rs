//! The Paladin's auras: Go `auras.go` and the `*_aura.go` files. The aura each cast turns on is
//! one of the self-cast paladin auras in sim/core/buffs.

use crate::prepare::buffs::paladin::{
    concentration_aura, devotion_aura_buff, fire_resistance_aura, frost_resistance_aura,
    retribution_aura_buff, shadow_resistance_aura, PaladinAuraRank,
};
use crate::prepare::character::constants::CHARACTER_LEVEL;
use crate::prepare::sim::{AuraId, Sim, UnitId};
use crate::prepare::spell::{ProcMask, SpellConfig, SpellFlag};
use crate::prepare::spelldata::Spell as Row;

use super::masks;
use super::spell_data::spell_data;
use super::util::gcd_cast;
use super::Paladin;

/// Go `auraRank`: the rank of a paladin aura as sim/core/buffs wants it, the spell the paladin
/// cast and its rank.
fn aura_rank(rank: &Row) -> PaladinAuraRank {
    PaladinAuraRank {
        spell_id: rank.id,
        rank: rank.rank_number(),
        value: 0.0,
    }
}

impl Paladin {
    /// Go `registerAuras`.
    pub(super) fn register_auras(&mut self, sim: &mut Sim, unit: UnitId) {
        self.register_devotion_aura(sim, unit);
        self.register_retribution_aura(sim, unit);
        self.register_concentration_aura(sim, unit);
        self.register_fire_resistance_aura(sim, unit);
        self.register_frost_resistance_aura(sim, unit);
        self.register_shadow_resistance_aura(sim, unit);
    }

    /// Go `registerAuraSpell`: the castable aura spell, instant, on the GCD, free.
    fn register_aura_spell(
        &self,
        sim: &mut Sim,
        unit: UnitId,
        rank: &Row,
        aura: AuraId,
        class_mask: i64,
    ) {
        let action_id = sim
            .aura(aura)
            .action_id
            .clone()
            .expect("a paladin aura has an action");
        sim.register_spell(
            unit,
            SpellConfig {
                action_id,
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::APL | SpellFlag::HELPFUL,
                class_spell_mask: class_mask,
                rank: rank.rank_number(),
                cast: gcd_cast(rank.gcd(), 0, None),
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerDevotionAura`: gives 735 additional armor to party members.
    fn register_devotion_aura(&mut self, sim: &mut Sim, unit: UnitId) {
        spell_data().devotion_aura.each(|_, rank| {
            let aura = devotion_aura_buff(sim, unit, true, &aura_rank(rank));
            self.register_aura_spell(sim, unit, rank, aura, masks::DEVOTION_AURA);
        });
    }

    /// Go `registerRetributionAura`: causes 30 Holy damage to any creature that strikes a
    /// party member.
    fn register_retribution_aura(&mut self, sim: &mut Sim, unit: UnitId) {
        spell_data().retribution_aura.each(|_, rank| {
            // The damage shield's number is the rank's first effect.
            let mut aura_rank = aura_rank(rank);
            aura_rank.value = rank.effect_n(1).average(CHARACTER_LEVEL);
            let aura = retribution_aura_buff(sim, unit, true, &aura_rank, 0.0);
            self.register_aura_spell(sim, unit, rank, aura, masks::RETRIBUTION_AURA);
        });
    }

    /// Go `registerConcentrationAura`.
    fn register_concentration_aura(&mut self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().concentration_aura.highest();
        let aura = concentration_aura(sim, unit, true, &aura_rank(rank));
        self.register_aura_spell(sim, unit, rank, aura, masks::CONCENTRATION_AURA);
    }

    /// Go `registerFireResistanceAura`.
    fn register_fire_resistance_aura(&mut self, sim: &mut Sim, unit: UnitId) {
        spell_data().fire_resistance_aura.each(|_, rank| {
            let aura = fire_resistance_aura(sim, unit, true, &aura_rank(rank));
            self.register_aura_spell(sim, unit, rank, aura, masks::FIRE_RESISTANCE_AURA);
        });
    }

    /// Go `registerFrostResistanceAura`.
    fn register_frost_resistance_aura(&mut self, sim: &mut Sim, unit: UnitId) {
        spell_data().frost_resistance_aura.each(|_, rank| {
            let aura = frost_resistance_aura(sim, unit, true, &aura_rank(rank));
            self.register_aura_spell(sim, unit, rank, aura, masks::FROST_RESISTANCE_AURA);
        });
    }

    /// Go `registerShadowResistanceAura`.
    fn register_shadow_resistance_aura(&mut self, sim: &mut Sim, unit: UnitId) {
        spell_data().shadow_resistance_aura.each(|_, rank| {
            let aura = shadow_resistance_aura(sim, unit, true, &aura_rank(rank));
            self.register_aura_spell(sim, unit, rank, aura, masks::SHADOW_RESISTANCE_AURA);
        });
    }
}
