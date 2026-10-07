//! The Paladin's heals: Go `holy_light.go`, `flash_of_light.go` and `lay_on_hands.go`.

use crate::prepare::sim::{Sim, UnitId};
use crate::prepare::spell::{ProcMask, SpellConfig, SpellFlag};
use crate::prepare::spelldata::Spell as Row;

use super::masks;
use super::util::{cooldown, gcd_cast, mana_cost, spell_action};
use super::Paladin;

impl Paladin {
    /// Go `registerHolyLight`: heals a friendly target for 1580.
    pub(super) fn register_holy_light(&mut self, sim: &mut Sim, unit: UnitId, rank: &Row) {
        let heal = rank.heal_effect();

        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::SPELL_HEALING,
                flags: SpellFlag::APL | SpellFlag::HELPFUL,
                class_spell_mask: masks::HOLY_LIGHT,
                rank: rank.rank_number(),
                max_range: f64::from(rank.max_range),
                cost: mana_cost(rank),
                cast: gcd_cast(rank.gcd(), rank.cast_time(), None),
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: heal.coeff(),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerFlashOfLight`: heals a friendly target for 308.
    pub(super) fn register_flash_of_light(&mut self, sim: &mut Sim, unit: UnitId, rank: &Row) {
        let heal = rank.heal_effect();

        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::SPELL_HEALING,
                flags: SpellFlag::APL | SpellFlag::HELPFUL,
                class_spell_mask: masks::FLASH_OF_LIGHT,
                rank: rank.rank_number(),
                max_range: f64::from(rank.max_range),
                cost: mana_cost(rank),
                cast: gcd_cast(rank.gcd(), rank.cast_time(), None),
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: heal.coeff(),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerLayOnHands`: heals a friendly target for an amount equal to the Paladin's
    /// maximum health and restores mana, draining all of the Paladin's remaining mana.
    pub(super) fn register_lay_on_hands(&mut self, sim: &mut Sim, unit: UnitId, rank: &Row) {
        let timer = sim.new_timer(unit);

        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                proc_mask: ProcMask::SPELL_HEALING,
                flags: SpellFlag::APL | SpellFlag::HELPFUL,
                class_spell_mask: masks::LAY_ON_HANDS,
                rank: rank.rank_number(),
                max_range: f64::from(rank.max_range),
                cast: gcd_cast(rank.gcd(), 0, Some((timer, cooldown(rank)))),
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                ..SpellConfig::default()
            },
        );
    }
}
