//! Go `sim/paladin/item_librams.go`: the item effects the Paladin package registers. Item
//! effects apply before the spells register, so a libram that changes a number a spell reads at
//! registration sets it on the paladin and the spell picks it up; one that changes a spell's
//! cost, cooldown or crit chance is a spell mod.

use crate::contracts::prepared_v2::ActionId;
use crate::prepare::aura_helpers::{CallbackMask, ProcTrigger};
use crate::prepare::character::{cooldown_type, MajorCooldown};
use crate::prepare::sim::{AuraConfig, Cooldown, Sim, UnitId, MILLISECOND, SECOND};
use crate::prepare::spell::{Cast, CastConfig, ProcMask, SpellConfig, SpellFlag};
use crate::prepare::spell_mod::{SpellModConfig, SpellModType};

use super::masks;
use super::util::spell_action;
use super::Paladin;

impl Paladin {
    /// The effects of the Paladin package's `core.NewItemEffect` registrations: applies the
    /// effect and answers true, or answers false for an item the package registers none for.
    pub(super) fn apply_class_item_effect(
        &mut self,
        sim: &mut Sim,
        unit: UnitId,
        item: i32,
    ) -> bool {
        match item {
            // Libram of Fervor: increases the melee attack power bonus of your Seal of the
            // Crusader by 48 and the Holy damage increase of your Judgement of the Crusader
            // by 33.
            23203 => {
                self.seal_of_the_crusader_bonus_attack_power += 48.0;
                self.judgement_of_the_crusader_bonus += 33.0;
            }
            // Libram of Hope: increases the duration of your Seal spells by 4 sec.
            22401 => sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::ALL_SEALS,
                    kind: SpellModType::BuffDurationFlat,
                    time_value: 4 * SECOND,
                    ..SpellModConfig::default()
                },
            ),
            // Libram of Light: increases healing done by Flash of Light by up to 83.
            23006 => self.flash_of_light_bonus_healing += 83.0,
            // Libram of Holy Alacrity: causes Holy Shock to reduce the cast time of your next
            // Holy Light cast within 10 sec by 0.2 sec.
            228175 => {
                let label = sim.unit(unit).label.clone();
                let alacrity = sim.register_aura(
                    unit,
                    AuraConfig {
                        label: format!("Holy Alacrity{label}"),
                        action_id: Some(spell_action(449982)),
                        duration: 10 * SECOND,
                        ..AuraConfig::default()
                    },
                );
                sim.attach_spell_mod(
                    alacrity,
                    SpellModConfig {
                        class_mask: masks::HOLY_LIGHT,
                        kind: SpellModType::CastTimeFlat,
                        time_value: -200 * MILLISECOND,
                        ..SpellModConfig::default()
                    },
                );
                sim.attach_proc_trigger(
                    alacrity,
                    &ProcTrigger {
                        callback: CallbackMask::ON_CAST_COMPLETE,
                        class_spell_mask: masks::HOLY_LIGHT,
                        trigger_immediately: true,
                        ..ProcTrigger::default()
                    },
                );

                sim.make_proc_trigger_aura(
                    unit,
                    &ProcTrigger {
                        name: format!("Libram of Holy Alacrity{label}"),
                        action_id: spell_action(449980),
                        callback: CallbackMask::ON_CAST_COMPLETE,
                        class_spell_mask: masks::HOLY_SHOCK | masks::HOLY_SHOCK_HEAL,
                        ..ProcTrigger::default()
                    },
                );
            }
            // Libram of Invocation: reduces the mana cost of your Seal spells by 5%.
            249442 => sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::ALL_SEALS,
                    kind: SpellModType::PowerCostPct,
                    float_value: -0.05,
                    ..SpellModConfig::default()
                },
            ),
            // Sentinel's Libram: reduces the cooldown of your Swift Judgement talent by 10 sec.
            272434 => sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::SWIFT_JUDGEMENT,
                    kind: SpellModType::CooldownFlat,
                    time_value: -10 * SECOND,
                    ..SpellModConfig::default()
                },
            ),
            // Libram of Law: increases the damage of your Judgement ability by 4%.
            272435 => sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::JUDGEMENT_OF_RIGHTEOUSNESS | masks::JUDGEMENT_OF_FURY,
                    kind: SpellModType::DamageDonePct,
                    float_value: 0.04,
                    ..SpellModConfig::default()
                },
            ),
            // Libram of Economy: reduces the Mana cost of your Holy Light ability by 5%.
            272436 => sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::HOLY_LIGHT,
                    kind: SpellModType::PowerCostPct,
                    float_value: -0.05,
                    ..SpellModConfig::default()
                },
            ),
            // Steadfast Libram: increases the Block Value of your shield by 30% while Holy
            // Shield is active.
            279247 => self.holy_shield_block_value_multiplier *= 1.3,
            // Libram of Infusion: increases the critical strike chance of your Holy Shock
            // spell by 6%.
            279248 => sim.add_static_mod(
                unit,
                SpellModConfig {
                    class_mask: masks::HOLY_SHOCK | masks::HOLY_SHOCK_HEAL,
                    kind: SpellModType::BonusCritPercent,
                    float_value: 6.0,
                    ..SpellModConfig::default()
                },
            ),
            // Sanctified Orb: use to restore 340 Mana, with a 5 min cooldown.
            20512 => {
                let action_id = ActionId {
                    item_id: 20512,
                    ..ActionId::default()
                };
                let timer = sim.new_timer(unit);
                let spell = sim.register_spell(
                    unit,
                    SpellConfig {
                        action_id,
                        spell_school: crate::prepare::spell::school::HOLY,
                        proc_mask: ProcMask::EMPTY,
                        flags: SpellFlag::NO_ON_CAST_COMPLETE | SpellFlag::HELPFUL,
                        cast: CastConfig {
                            cd: Cooldown {
                                timer: Some(timer),
                                duration: 5 * 60 * SECOND,
                            },
                            default_cast: Cast::default(),
                            ..CastConfig::default()
                        },
                        ..SpellConfig::default()
                    },
                );
                sim.add_major_cooldown(
                    unit,
                    MajorCooldown {
                        spell,
                        priority: 0,
                        cooldown_type: cooldown_type::MANA,
                        allow_spell_queueing: false,
                        timings: Vec::new(),
                    },
                );
            }
            // Tenets of the Silver Hand changes the attack tables, which exist only once
            // the environment is finalized; it is not prepared yet.
            _ => return false,
        }
        true
    }
}
