//! Go racials.go effects with runtime behavior.

use super::{
    Action, Agent, AuraRef, Fight, ModId, ModKind, Side, SpellId, SpellResult, PRIORITY_DOT,
};

/// Go racials.go `applyEureka`: its aura and modifiers, and the casts that spend a stack.
#[derive(Clone, Debug)]
pub(crate) struct Eureka {
    pub(crate) aura: AuraRef,
    /// Cost, damage and the tick cancel, in Go's registration and activation order.
    mods: Vec<ModId>,
    spends: Vec<bool>,
}

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

    /// Register Eureka!'s modifiers on the spells the exporter resolved from the class masks.
    pub(crate) fn bind_eureka(
        &mut self,
        aura: &str,
        values: [f64; 3],
        spells: [&Vec<usize>; 3],
        spending: &[usize],
    ) -> Result<Eureka, String> {
        let aura = self.player_aura(aura)?;
        let count = self.spells.len();
        if spells
            .iter()
            .flat_map(|list| list.iter())
            .chain(spending)
            .any(|&spell| spell >= count)
        {
            return Err("Eureka! names a spell position outside the spellbook".into());
        }
        let kinds = [
            ModKind::PowerCostPercent,
            ModKind::DamageDonePercent,
            ModKind::DotDamageDonePercent,
        ];
        let mods = kinds
            .into_iter()
            .zip(values)
            .zip(spells)
            .map(|((kind, value), spells)| self.register_mod(kind, value, 0, spells.clone()))
            .collect();
        let mut spends = vec![false; count];
        for &spell in spending {
            spends[spell] = true;
        }
        Ok(Eureka { aura, mods, spends })
    }

    /// Eureka!'s OnGain: three stacks, then its modifiers.
    pub(crate) fn eureka_gain(&mut self) {
        let eureka = self.eureka.clone().expect("Eureka! is bound");
        self.set_stacks(eureka.aura, 3);
        for modifier in eureka.mods {
            self.activate_mod(modifier);
        }
    }

    pub(crate) fn eureka_expire(&mut self) {
        let eureka = self.eureka.clone().expect("Eureka! is bound");
        for modifier in eureka.mods {
            self.deactivate_mod(modifier);
        }
    }

    /// Eureka!'s OnCastComplete: a named spell spends a stack.
    pub(crate) fn eureka_cast_complete(&mut self, spell: SpellId) {
        let eureka = self.eureka.as_ref().expect("Eureka! is bound");
        if eureka.spends[spell] {
            let aura = eureka.aura;
            self.remove_stack(aura);
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
