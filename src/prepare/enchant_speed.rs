//! Go sim/core/enchant_speed.go: the haste pseudo stats of an item and of its enchant multiply
//! the wearer's melee, ranged and cast speed while the item is equipped. Each item and each
//! enchanted item applies its own.

use super::items::NUM_ITEM_SLOTS;
use super::sim::{AuraConfig, AuraId, BuildPhase, Sim, UnitId, NEVER_EXPIRES};
use super::spelldata::speed::pseudo_stat;

/// Go `hastePercents`: the melee, ranged and spell haste percents, indexed by `proto.PseudoStat`.
pub(crate) fn haste_percents(pseudo_stats: &[f64]) -> (f64, f64, f64) {
    let value = |index: usize| pseudo_stats.get(index).copied().unwrap_or(0.0);
    (
        value(pseudo_stat::MELEE_HASTE_PERCENT),
        value(pseudo_stat::RANGED_HASTE_PERCENT),
        value(pseudo_stat::SPELL_HASTE_PERCENT),
    )
}

impl Sim {
    /// Go `Aura.AttachHastePseudoStats`: multiplies the unit's melee, ranged and cast speed by
    /// the haste percents `pseudo_stats` states, indexed by `proto.PseudoStat`, while the aura
    /// is up.
    pub(crate) fn attach_haste_pseudo_stats(
        &mut self,
        aura: AuraId,
        pseudo_stats: &[f64],
    ) -> AuraId {
        let (melee, ranged, cast) = haste_percents(pseudo_stats);
        if melee != 0.0 {
            self.attach_multiply_melee_speed(aura, 1.0 + melee / 100.0);
        }
        if ranged != 0.0 {
            self.attach_multiply_ranged_speed(aura, 1.0 + ranged / 100.0);
        }
        if cast != 0.0 {
            self.attach_multiply_cast_speed(aura, 1.0 + cast / 100.0);
        }
        aura
    }

    /// Go `Character.registerEquipSpeedAuras`.
    pub(crate) fn register_equip_speed_auras(&mut self, unit: UnitId) {
        for slot in 0..NUM_ITEM_SLOTS {
            let (item_id, item_pseudo, enchant_id, enchant_pseudo) = {
                let equipped = &self.character(unit).equipment[slot];
                (
                    equipped.id,
                    equipped.pseudo_stats.clone(),
                    equipped.enchant.effect_id,
                    equipped.enchant.pseudo_stats.clone(),
                )
            };
            self.register_speed_aura(unit, slot, "Item", item_id, &item_pseudo);
            self.register_speed_aura(unit, slot, "Enchant", enchant_id, &enchant_pseudo);
        }
    }

    /// Go `Character.registerSpeedAura`, for an item that is equipped at the start.
    fn register_speed_aura(
        &mut self,
        unit: UnitId,
        slot: usize,
        kind: &str,
        id: i32,
        pseudo_stats: &[f64],
    ) {
        let (melee, ranged, cast) = haste_percents(pseudo_stats);
        if melee == 0.0 && ranged == 0.0 && cast == 0.0 {
            return;
        }
        let slot_name = crate::contracts::request::enum_name("proto.ItemSlot", slot as i32)
            .expect("an item slot name");
        let aura = self.get_or_register_aura(
            unit,
            AuraConfig {
                label: format!("{kind} {id} Speed ({slot_name})"),
                build_phase: BuildPhase::GEAR,
                duration: NEVER_EXPIRES,
                ..AuraConfig::default()
            },
        );
        let aura = self.make_permanent(aura);
        self.attach_haste_pseudo_stats(aura, pseudo_stats);
    }
}
