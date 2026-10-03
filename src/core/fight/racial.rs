//! Go racials.go effects with runtime behavior.

use super::{Action, Agent, AuraRef, Fight, Side, SpellId, SpellResult, PRIORITY_DOT};

impl<A: Agent> Fight<A> {
    /// Touch of the Grave's proc trigger: Go `AttachProcTriggerCallback` with
    /// `ProcMaskDirect`, `OutcomeLanded`, an internal cooldown and the spell batch delay.
    pub(crate) fn touch_of_the_grave_callback(
        &mut self,
        aura: AuraRef,
        spell: SpellId,
        result: &SpellResult,
        chance: f64,
        delay: i64,
    ) {
        let state = &self.spells[spell];
        // canProcFrom without CanProcFromProcs, then the proc mask and the outcome.
        if state.flags.proc || !state.direct_proc || !result.landed() {
            return;
        }
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
        let result = *result;
        self.schedule(
            self.now + delay,
            PRIORITY_DOT,
            Action::DelayedProc {
                aura,
                spell,
                result,
            },
        );
    }

    /// The drain's `ApplyEffects`: a hit-only magic roll on a share of maximum health, and
    /// the same amount of health back when it lands.
    pub(crate) fn touch_of_the_grave_drain(
        &mut self,
        spell: SpellId,
        target: Side,
        health_fraction: f64,
        metrics: usize,
    ) {
        let base = health_fraction * self.config.max_health;
        let result = self.calc_damage_hit_only(spell, target, base);
        self.deal_damage(spell, result, false);
        if result.landed() {
            self.gain_health(result.damage, metrics);
        }
    }

    /// Go `healthBar.GainHealth`.
    fn gain_health(&mut self, amount: f64, metrics: usize) {
        let old = self.player.health;
        let new = (old + amount).min(self.config.max_health);
        let resource = &mut self.resources[metrics];
        resource.events += 1;
        resource.gain += amount;
        resource.actual_gain += new - old;
        if self.log.is_some() {
            let line = format!(
                "Gained {amount:.3} health from {} ({old:.3} --> {new:.3}) of {:.0} total.",
                super::log::action_string(&self.resources[metrics].id),
                self.config.max_health
            );
            self.player_log(&line);
        }
        self.player.health = new;
    }
}
