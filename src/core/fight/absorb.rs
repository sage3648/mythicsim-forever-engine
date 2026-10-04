//! Go common/shared/shared_utils.go `NewSpellDataAbsorbOnUse` and core aura_helpers.go
//! `NewDamageAbsorptionAura`: an item use whose aura shields the wearer, for the amount its
//! absorb effect rolls, against the damage of the schools the effect masks. The shield is one of
//! the player's dynamic damage taken modifiers: while up it takes what it can of a hit that deals
//! damage, its stacks follow the strength left, and it fades when spent. A second use replaces
//! the strength left.
//!
//! Go `NewSpellDataAbsorbProc` raises the same shield from a listener on the melee hits the
//! wearer takes (`applySpellDataSelfProc`), as Uther's Strength and the chest absorption
//! enchants do. Only the target's swings are such hits in scope.

use crate::contracts::prepared_v2::Effect;

use super::{
    damage::{
        OUTCOME_BLOCK, OUTCOME_CRIT, OUTCOME_CRUSH, OUTCOME_DODGE, OUTCOME_GLANCE, OUTCOME_HIT,
        OUTCOME_MISS, OUTCOME_PARRY,
    },
    Agent, AuraRef, BuildError, Fight, Side, SpellId, SpellResult,
};

/// One item's shield.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ItemAbsorb {
    pub(crate) aura: AuraRef,
    /// Go `SpellSchool` bits of the hits it takes.
    pub(crate) schools: u8,
    pub(crate) average: f64,
    pub(crate) variance: f64,
    strength: f64,
}

impl ItemAbsorb {
    pub(crate) fn new(aura: AuraRef, schools: u8, average: f64, variance: f64) -> Self {
        ItemAbsorb {
            aura,
            schools,
            average,
            variance,
            strength: 0.0,
        }
    }
}

/// One absorb proc's listener.
#[derive(Clone, Copy, Debug)]
pub(crate) struct AbsorbProc {
    /// Go `ProcTrigger.Outcome`; zero hears every outcome.
    outcome: u16,
    require_damage: bool,
    chance: f64,
    spell: SpellId,
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
        other => return Err(format!("absorb proc outcome {other} is unsupported")),
    })
}

/// The shield of the absorb proc that casts the spell at a spellbook position, by its position
/// in `Fight::item_absorbs`: after the item uses' shields, in effect order.
pub(crate) fn proc_shield(effects: &[Effect], position: usize) -> Option<usize> {
    let uses = effects
        .iter()
        .filter(|effect| matches!(effect, Effect::AbsorbOnUse { .. }))
        .count();
    effects
        .iter()
        .filter(|effect| matches!(effect, Effect::SpellDataAbsorbProc { .. }))
        .position(|effect| matches!(effect, Effect::SpellDataAbsorbProc { spell, .. } if *spell == position))
        .map(|index| uses + index)
}

impl<A: Agent> Fight<A> {
    /// The absorb procs' listeners and shields, in effect order, after the item uses' shields.
    pub(crate) fn bind_absorb_procs(&mut self, effects: &[Effect]) -> Result<(), BuildError> {
        for effect in effects {
            if let Effect::SpellDataAbsorbProc {
                outcome,
                require_damage,
                proc_chance,
                spell,
                aura,
                schools,
                average,
                variance,
                ..
            } = effect
            {
                if *spell >= self.spells.len() {
                    return Err(format!("absorb proc spell {spell} is not registered"));
                }
                let mut bits = 0;
                for name in outcome {
                    bits |= outcome_bit(name)?;
                }
                let aura = self.player_aura(aura)?;
                self.item_absorbs
                    .push(ItemAbsorb::new(aura, *schools, *average, *variance));
                self.absorb_procs.push(AbsorbProc {
                    outcome: bits,
                    require_damage: *require_damage,
                    chance: *proc_chance,
                    spell: *spell,
                });
            }
        }
        Ok(())
    }

    /// Go `AttachProcTriggerCallback` with `TriggerImmediately` on a hit the player takes: the
    /// outcome, a hit that dealt damage, the cooldown, the chance roll under the trigger's name,
    /// then the absorb spell cast on the wearer.
    pub(crate) fn absorb_proc_callback(
        &mut self,
        aura: AuraRef,
        proc: usize,
        result: &SpellResult,
    ) {
        let state = self.absorb_procs[proc];
        if (state.outcome != 0 && result.outcome & state.outcome == 0)
            || (state.require_damage && result.damage == 0.0)
        {
            return;
        }
        let icd = self.aura(aura).icd;
        if let Some((timer, _)) = icd {
            if self.timers[timer] > self.now {
                return;
            }
        }
        if state.chance != 1.0 && self.random_for_aura(aura) > state.chance {
            return;
        }
        if let Some((timer, duration)) = icd {
            self.timers[timer] = self.now + duration;
        }
        self.cast(state.spell, Side::Player);
    }

    /// The use's `ApplyEffects`: the roll, then `DamageAbsorptionAura.Activate`, which sets the
    /// strength and stacks it as whole points.
    pub(crate) fn apply_item_absorb(&mut self, index: usize) {
        let shield = self.item_absorbs[index];
        let amount = self.effect_roll(shield.average, shield.variance);
        self.activate_aura(shield.aura);
        self.item_absorbs[index].strength = amount;
        let stacks = (amount as i32).max(1);
        self.aura_mut(shield.aura).max_stacks = stacks;
        self.set_stacks(shield.aura, stacks);
    }

    /// The shields' damage taken modifiers on a hit of the school bits, in registration order.
    pub(crate) fn item_absorbs_taken(&mut self, school: u8, result: &mut SpellResult) {
        for index in 0..self.item_absorbs.len() {
            let shield = self.item_absorbs[index];
            if !self.aura(shield.aura).active
                || result.damage <= 0.0
                || school & shield.schools == 0
            {
                continue;
            }
            let absorbed = shield.strength.min(result.damage);
            result.damage -= absorbed;
            let strength = shield.strength - absorbed;
            self.item_absorbs[index].strength = strength;
            if self.log.is_some() {
                let line = format!(
                    "{} absorbed {absorbed:.1} damage, new shield strength: {strength:.1}",
                    self.aura(shield.aura).label
                );
                self.player_log(&line);
            }
            if strength <= 0.0 {
                self.deactivate_aura(shield.aura);
                continue;
            }
            self.set_stacks(shield.aura, strength as i32);
        }
    }
}
