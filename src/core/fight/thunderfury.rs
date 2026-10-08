//! Go common/classic/items_weapons.go Thunderfury, Blessed Blade of the Windseeker: a weapon proc
//! on landed hits whose handler, a spell batch window later, casts two spells on the unit hit.
//!
//! The strike is a hit of 300 on the magic table with a crit; a landed one puts Cyclone on its
//! target, a slow of 20% that is an exclusive effect of the attack speed category Thunder Clap
//! shares. The bounce rolls no damage on the magic hit table against up to five targets from the
//! unit hit, and each it lands on takes the Thunderfury aura, which lowers the target's nature
//! resistance by 25 while it lasts.

use crate::contracts::prepared_v2::Effect;

use super::enemy::EnemySlow;
use super::{Agent, AuraRef, Fight, Outcome, Side, SpellId, SpellResult, OUTCOME_LANDED};

/// The bound proc: its spells and the two auras they put on a target.
#[derive(Clone, Debug)]
pub(crate) struct Thunderfury {
    /// The proc chance of each spell, from the weapon's proc manager.
    pub(crate) chances: Vec<Option<f64>>,
    pub(crate) strike: SpellId,
    pub(crate) bounce: SpellId,
    pub(crate) bounce_targets: usize,
    /// Cyclone, on the first target; the others hold copies at the same position.
    pub(crate) slow: AuraRef,
    /// Thunderfury's resistance aura, likewise.
    pub(crate) resistance: AuraRef,
}

impl<A: Agent> Fight<A> {
    /// Bind Thunderfury's proc to its spells and the target auras it activates.
    pub(crate) fn bind_thunderfury(&mut self, effect: &Effect) -> Result<(), String> {
        let Effect::Thunderfury {
            chances,
            strike_spell,
            bounce_spell,
            bounce_targets,
            slow_aura,
            slow_multiplier,
            resistance_aura,
            ..
        } = effect
        else {
            return Ok(());
        };
        if *strike_spell >= self.spells.len() || *bounce_spell >= self.spells.len() {
            return Err("Thunderfury names spells the player does not have".into());
        }
        let bounce_targets = usize::try_from(*bounce_targets)
            .map_err(|_| format!("Thunderfury bounces to {bounce_targets} targets"))?;
        let mut by_spell = vec![None; self.spells.len()];
        for entry in chances {
            if let Some(slot) = by_spell.get_mut(entry.spell) {
                *slot = Some(entry.chance);
            }
        }
        let target_aura = |fight: &Self, label: &str| {
            fight.trackers[Side::Target.index()]
                .find(label)
                .map(|index| AuraRef {
                    side: Side::Target,
                    index,
                })
                .ok_or_else(|| format!("target aura {label} is not registered"))
        };
        let slow = target_aura(self, slow_aura)?;
        // Go `AtkSpeedReductionEffect`: the attack speed multiplier by one over the slow's
        // multiplier on gain, and by the multiplier on expiry.
        self.register_enemy_slow(
            slow,
            EnemySlow {
                melee: false,
                gain: 1.0 / slow_multiplier,
                expire: *slow_multiplier,
                priority: None,
            },
        );
        self.thunderfury = Some(Thunderfury {
            chances: by_spell,
            strike: *strike_spell,
            bounce: *bounce_spell,
            bounce_targets,
            slow,
            resistance: target_aura(self, resistance_aura)?,
        });
        Ok(())
    }

    /// Go `CreateWeaponProcTrigger` for Thunderfury: landed hits roll the spell's chance, then
    /// the handler runs a spell batch window later.
    pub(crate) fn thunderfury_callback(
        &mut self,
        aura: AuraRef,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if result.outcome & OUTCOME_LANDED == 0 {
            return;
        }
        let Some(chance) = self
            .thunderfury
            .as_ref()
            .and_then(|proc| proc.chances[spell])
        else {
            return;
        };
        if !self.proc_for_aura(chance, aura) {
            return;
        }
        self.schedule_delayed_proc(aura, spell, *result);
    }

    /// The handler: cast the strike, then the bounce, on the unit hit.
    pub(crate) fn thunderfury_handler(&mut self, target: Side) {
        let proc = self.thunderfury.as_ref().expect("Thunderfury is bound");
        let (strike, bounce) = (proc.strike, proc.bounce);
        self.cast(strike, target);
        self.cast(bounce, target);
    }

    /// The strike's `ApplyEffects`: `CalcAndDealDamage` of the literal on the magic table with a
    /// crit, then Cyclone on the target when it landed.
    pub(crate) fn thunderfury_strike(&mut self, spell: SpellId, target: Side, damage: f64) {
        let result = self.calc_damage(spell, target, damage);
        self.deal_damage(spell, result, false);
        if result.landed() {
            let slow = self
                .thunderfury
                .as_ref()
                .expect("Thunderfury is bound")
                .slow;
            let aura = self.aura_on(slow, result.target);
            self.activate_aura(aura);
        }
    }

    /// The bounce's `ApplyEffects`: `CalcCleaveDamage` of nothing on the magic hit table, the
    /// resistance aura on each target it landed on, then `DealBatchedAoeDamage`.
    pub(crate) fn thunderfury_bounce(&mut self, spell: SpellId, target: Side) {
        let proc = self.thunderfury.as_ref().expect("Thunderfury is bound");
        let (targets, resistance) = (proc.bounce_targets, proc.resistance);
        let results: Vec<SpellResult> = self
            .cleave_targets(target, targets)
            .into_iter()
            .map(|hit| self.calc_damage_with(spell, hit, 0.0, Outcome::MagicHit))
            .collect();
        for result in &results {
            if result.landed() {
                let aura = self.aura_on(resistance, result.target);
                self.activate_aura(aura);
            }
        }
        self.deal_batched_aoe_damage(spell, &results, false);
    }

    /// The resistance aura's `OnGain` and `OnExpire`: `AddStatDynamic` of the nature resistance
    /// on its target.
    pub(crate) fn thunderfury_resistance(&mut self, target: Side, delta: f64) {
        self.add_target_resistance(target, super::school_index(8), delta);
    }
}
