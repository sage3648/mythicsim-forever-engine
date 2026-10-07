//! The Shaman's shields: Go `sim/shaman` `shields.go`.

use crate::prepare::aura_helpers::{CallbackMask, ProcTrigger};
use crate::prepare::sim::{AuraConfig, Cooldown, Sim, UnitId, SECOND};
use crate::prepare::spell::{
    school, CastConfig, DefenseType, ProcMask, SpellConfig, SpellFlag, GCD_DEFAULT,
};

use super::spell_data::spell_data;
use super::spells::{default_cast, flat_cost, spell_action};
use super::{flags, masks, Shaman};

/// Water Shield (408510) sits on the Restoration talent line with no rank subtext, so
/// gen_spelldata makes no table for it and the ids and values are pinned from the client: three
/// globes of 2% maximum mana, one every 3.5 sec at most, no mana cost, 15 sec cooldown.
const WATER_SHIELD_SPELL_ID: i32 = 408510;
const WATER_SHIELD_GLOBES: i32 = 3;

impl Shaman {
    /// Go `registerShieldsSpells`.
    pub(super) fn register_shields_spells(&self, sim: &mut Sim, unit: UnitId) {
        self.register_water_shield_spell(sim, unit);
        self.register_lightning_shield_spell(sim, unit);
        self.register_shield_effect_trigger_spell(sim, unit);
    }

    /// Go `registerShieldEffectTriggerSpell`.
    fn register_shield_effect_trigger_spell(&self, sim: &mut Sim, unit: UnitId) {
        sim.register_spell(
            unit,
            SpellConfig {
                flags: SpellFlag::NO_METRICS | SpellFlag::NO_LOGS,
                class_spell_mask: masks::SHIELD_SELF_PROC,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerWaterShieldSpell`.
    fn register_water_shield_spell(&self, sim: &mut Sim, unit: UnitId) {
        if !self.has_talent("water_shield") {
            return;
        }
        let action_id = spell_action(WATER_SHIELD_SPELL_ID);
        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Water Shield".to_string(),
                action_id: Some(action_id.clone()),
                duration: 10 * 60 * SECOND,
                max_stacks: WATER_SHIELD_GLOBES,
                ..AuraConfig::default()
            },
        );
        sim.attach_proc_trigger(
            aura,
            &ProcTrigger {
                name: "Water Shield Trigger".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                icd: 3500 * crate::prepare::sim::MILLISECOND,
                class_spell_mask: masks::SHIELD_SELF_PROC,
                ..ProcTrigger::default()
            },
        );

        let timer = sim.new_timer(unit);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id,
                spell_school: school::NATURE,
                defense_type: DefenseType::Magic,
                flags: SpellFlag::APL | flags::INSTANT,
                cast: CastConfig {
                    cd: Cooldown {
                        timer: Some(timer),
                        duration: 15 * SECOND,
                    },
                    ..default_cast(GCD_DEFAULT, 0)
                },
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerLightningShieldSpell`.
    fn register_lightning_shield_spell(&self, sim: &mut Sim, unit: UnitId) {
        let data = spell_data();
        let rank = data.lightning_shield.highest();
        // The shield spell itself carries no damage; the orb that fires is a separate spell, and
        // rank 7's is 26363.
        let orb = data.lightning_shield_triggered.by_id(26363);
        let action_id = spell_action(rank.id);

        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(orb.id),
                spell_school: orb.spell_school(),
                defense_type: orb.defense_type_core(),
                proc_mask: ProcMask::EMPTY,
                flags: flags::SHAMAN_SPELL | SpellFlag::PASSIVE_SPELL,
                class_spell_mask: masks::LIGHTNING_SHIELD,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: orb.damage_effect().coeff(),
                ..SpellConfig::default()
            },
        );

        let aura = sim.register_aura(
            unit,
            AuraConfig {
                label: "Lightning Shield".to_string(),
                action_id: Some(action_id.clone()),
                duration: rank.duration(),
                max_stacks: i32::from(rank.proc_charges),
                ..AuraConfig::default()
            },
        );
        sim.attach_proc_trigger(
            aura,
            &ProcTrigger {
                name: "Lightning Shield Trigger".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_TAKEN,
                icd: 3500 * crate::prepare::sim::MILLISECOND,
                class_spell_mask: masks::SHIELD_SELF_PROC,
                ..ProcTrigger::default()
            },
        );

        sim.register_spell(
            unit,
            SpellConfig {
                action_id,
                spell_school: school::NATURE,
                defense_type: DefenseType::Magic,
                flags: SpellFlag::APL | flags::INSTANT,
                cost: flat_cost(rank.cost() as i32),
                cast: default_cast(rank.gcd(), 0),
                related_self_buff: Some(aura),
                ..SpellConfig::default()
            },
        );
    }
}
