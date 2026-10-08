//! Go common/shared/shared_utils.go `NewSpellDataHealProc`: an enchant's listener, resolved from
//! its trigger row, that casts the heal row on the wearer at once (`applySpellDataSelfProc`), as
//! Recovery does when the wearer's melee attack is dodged or parried. The heal is a share of
//! maximum health or a rolled amount, through `CalcAndDealHealing`.

use crate::contracts::prepared_v2::Effect;

use super::{
    damage::{
        OUTCOME_BLOCK, OUTCOME_CRIT, OUTCOME_CRUSH, OUTCOME_DODGE, OUTCOME_GLANCE, OUTCOME_HIT,
        OUTCOME_MISS, OUTCOME_PARRY,
    },
    healing::Healing,
    Agent, AuraRef, BuildError, Fight, Side, SpellId, SpellResult,
};

/// One heal proc's listener.
#[derive(Clone, Debug)]
pub(crate) struct HealProc {
    trigger_spells: Vec<bool>,
    /// A "when struck" proc, on the melee hits the player takes.
    pub(crate) struck: bool,
    /// Go `ProcTrigger.Outcome`; zero hears every outcome.
    outcome: u16,
    require_damage: bool,
    chance: f64,
    spell: SpellId,
}

/// What the heal spell does: a share of the wearer's maximum health or a rolled amount.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SelfHeal {
    pub(crate) max_health_share: Option<f64>,
    pub(crate) average: f64,
    pub(crate) variance: f64,
    pub(crate) can_crit: bool,
    pub(crate) healing: Healing,
    pub(crate) bonus_healing_taken: f64,
}

/// Go `HitOutcome` names, as the exporter writes them.
fn outcome_bit(name: &str) -> Result<u16, BuildError> {
    Ok(match name {
        "Miss" => OUTCOME_MISS,
        "Hit" => OUTCOME_HIT,
        "Crit" => OUTCOME_CRIT,
        "Dodge" => OUTCOME_DODGE,
        "Parry" => OUTCOME_PARRY,
        "Block" => OUTCOME_BLOCK,
        "Glance" => OUTCOME_GLANCE,
        "Crush" => OUTCOME_CRUSH,
        other => return Err(format!("heal proc outcome {other} is unsupported")),
    })
}

/// The heal behavior of an item use, if its effect heals the wearer.
pub(crate) fn heal_on_use(effects: &[Effect], item: i32) -> Option<SelfHeal> {
    effects.iter().find_map(|effect| match effect {
        Effect::HealOnUse {
            item_id,
            can_crit,
            healing_dealt_multiplier,
            healing_taken_multiplier,
            table_healing_dealt_multiplier,
            bonus_healing_taken,
            max_health_share,
            average,
            variance,
        } if *item_id == item => Some(SelfHeal {
            max_health_share: *max_health_share,
            average: average.unwrap_or(0.0),
            variance: variance.unwrap_or(0.0),
            can_crit: *can_crit,
            healing: Healing {
                dealt_multiplier: *healing_dealt_multiplier,
                taken_multiplier: *healing_taken_multiplier,
                table_multiplier: *table_healing_dealt_multiplier,
                healing_power: 0.0,
            },
            bonus_healing_taken: *bonus_healing_taken,
        }),
        _ => None,
    })
}

/// The heal behavior of an exported spell, if a heal proc casts it.
pub(crate) fn self_heal(effects: &[Effect], position: usize) -> Option<SelfHeal> {
    effects.iter().find_map(|effect| match effect {
        Effect::SpellDataHealProc {
            spell,
            can_crit,
            healing_dealt_multiplier,
            healing_taken_multiplier,
            table_healing_dealt_multiplier,
            bonus_healing_taken,
            max_health_share,
            average,
            variance,
            ..
        } if *spell == position => Some(SelfHeal {
            max_health_share: *max_health_share,
            average: average.unwrap_or(0.0),
            variance: variance.unwrap_or(0.0),
            can_crit: *can_crit,
            healing: Healing {
                dealt_multiplier: *healing_dealt_multiplier,
                taken_multiplier: *healing_taken_multiplier,
                table_multiplier: *table_healing_dealt_multiplier,
                healing_power: 0.0,
            },
            bonus_healing_taken: *bonus_healing_taken,
        }),
        _ => None,
    })
}

impl<A: Agent> Fight<A> {
    /// The heal procs' listeners, in effect order, which their trigger auras index.
    pub(crate) fn bind_heal_procs(&mut self, effects: &[Effect]) -> Result<(), BuildError> {
        for effect in effects {
            if let Effect::SpellDataHealProc {
                trigger_spells,
                struck,
                outcome,
                require_damage,
                proc_chance,
                spell,
                ..
            } = effect
            {
                let mut mask = vec![false; self.spells.len()];
                for &trigger in trigger_spells {
                    if let Some(slot) = mask.get_mut(trigger) {
                        *slot = true;
                    }
                }
                let mut bits = 0;
                for name in outcome {
                    bits |= outcome_bit(name)?;
                }
                self.heal_procs.push(HealProc {
                    trigger_spells: mask,
                    struck: *struck,
                    outcome: bits,
                    require_damage: *require_damage,
                    chance: *proc_chance,
                    spell: *spell,
                });
            }
        }
        Ok(())
    }

    /// Go `AttachProcTriggerCallback` with `TriggerImmediately`: the spell and outcome, the
    /// cooldown, the chance roll under the trigger's name, then the heal cast on the wearer.
    pub(crate) fn heal_proc_callback(
        &mut self,
        aura: AuraRef,
        proc: usize,
        spell: Option<SpellId>,
        result: &SpellResult,
    ) {
        let state = &self.heal_procs[proc];
        // A struck proc hears the target's swing, which is no spell of the player's.
        let heard = match spell {
            Some(spell) => state.trigger_spells[spell],
            None => state.struck,
        };
        if !heard
            || (state.outcome != 0 && result.outcome & state.outcome == 0)
            || (state.require_damage && result.damage == 0.0)
        {
            return;
        }
        let (chance, heal) = (state.chance, state.spell);
        let icd = self.aura(aura).icd;
        if let Some((timer, _)) = icd {
            if self.timers[timer] > self.now {
                return;
            }
        }
        if chance != 1.0 && self.random_for_aura(aura) > chance {
            return;
        }
        if let Some((timer, duration)) = icd {
            self.timers[timer] = self.now + duration;
        }
        self.cast(heal, Side::Player);
    }

    /// The heal spell's `ApplyEffects`: the amount, then `CalcAndDealHealing` with a crit
    /// roll unless the row rules crits out.
    pub(crate) fn apply_self_heal(&mut self, spell: SpellId, heal: SelfHeal) {
        let amount = match heal.max_health_share {
            Some(share) => self.player_max_health() * share,
            None => self.effect_roll(heal.average, heal.variance),
        };
        if heal.can_crit {
            self.calc_and_deal_healing(
                spell,
                Side::Player,
                amount,
                heal.healing,
                heal.bonus_healing_taken,
            );
        } else {
            // Go Spell.HealingPower, which the debug line prints.
            let mut healing = heal.healing;
            healing.healing_power = self.player.powers.healing_power + heal.bonus_healing_taken;
            self.calc_and_deal_self_healing(spell, amount, healing);
        }
    }
}
