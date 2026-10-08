//! Go common/shared/shared_utils.go `NewSpellDataDamageOnUse`: an on-use item whose spell deals
//! its row's damage on the one target. The direct hit rolls once and takes the scale the area
//! rule leaves for one target; a row with only damage over time rolls its application on the
//! hit table without a crit. The hit lands after the row's travel, and the damage over time
//! goes on with it when it landed, ticking the row's amount on current stats.

use crate::contracts::prepared_v2::{OnUseDirect, OnUsePeriodic};

use super::{
    melee::PhysicalOutcome, proc_damage::ProcDamage, Agent, Fight, Outcome, Side, SpellId,
    SpellResult,
};

/// An outcome applier by the name the exporter writes.
#[derive(Clone, Copy, Debug)]
pub(crate) enum OnUseOutcome {
    MagicHitAndCrit,
    MagicHit,
    MeleeSpecialHitAndCrit,
    MeleeSpecialHit,
}

impl OnUseOutcome {
    pub(crate) fn parse(name: &str) -> Result<Self, String> {
        match name {
            "magic_hit_and_crit" => Ok(Self::MagicHitAndCrit),
            "magic_hit" => Ok(Self::MagicHit),
            "melee_special_hit_and_crit" => Ok(Self::MeleeSpecialHitAndCrit),
            "melee_special_hit" => Ok(Self::MeleeSpecialHit),
            _ => Err(format!("unknown on-use outcome {name}")),
        }
    }
}

/// The spell's direct hit and damage over time.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OnUseDamage {
    direct: Option<(f64, f64, f64, OnUseOutcome)>,
    /// A hit past one target is the damage proc spell, which spreads it as an area or a chain.
    spread: Option<ProcDamage>,
    tick_base: Option<f64>,
    application: Option<OnUseOutcome>,
}

impl OnUseDamage {
    pub(crate) fn new(
        direct: &Option<OnUseDirect>,
        periodic: &Option<OnUsePeriodic>,
    ) -> Result<Self, String> {
        let spread = match direct {
            Some(direct) if direct.area.is_some() || direct.chain.is_some() => {
                let outcome = OnUseOutcome::parse(&direct.outcome)?;
                let (name, can_crit) = match outcome {
                    OnUseOutcome::MagicHitAndCrit => (None, true),
                    OnUseOutcome::MagicHit => (None, false),
                    OnUseOutcome::MeleeSpecialHitAndCrit => {
                        (Some("melee_special_hit_and_crit"), true)
                    }
                    OnUseOutcome::MeleeSpecialHit => (Some("melee_special_hit"), false),
                };
                Some(ProcDamage::new(
                    direct.average,
                    direct.variance,
                    can_crit,
                    name,
                    direct.chain.as_ref(),
                    direct.area.as_ref(),
                    None,
                )?)
            }
            _ => None,
        };
        Ok(Self {
            spread,
            direct: direct
                .as_ref()
                .map(|direct| {
                    Ok::<_, String>((
                        direct.average,
                        direct.variance,
                        direct.scale,
                        OnUseOutcome::parse(&direct.outcome)?,
                    ))
                })
                .transpose()?,
            tick_base: periodic.as_ref().map(|periodic| periodic.tick_base),
            application: periodic
                .as_ref()
                .and_then(|periodic| periodic.application_outcome.as_deref())
                .map(OnUseOutcome::parse)
                .transpose()?,
        })
    }

    pub(crate) fn tick_base(&self) -> Option<f64> {
        self.tick_base
    }
}

/// Whether the runtime knows every outcome applier the effect names.
pub(crate) fn known(direct: &Option<OnUseDirect>, periodic: &Option<OnUsePeriodic>) -> bool {
    OnUseDamage::new(direct, periodic).is_ok()
}

impl<A: Agent> Fight<A> {
    /// The ticks' crit: spelldata `TickOutcomeHitRolled` crits only where the row states
    /// Periodic Can Crit, on the magic table the exporter requires for it.
    pub(crate) fn bind_on_use_damage(&mut self, effects: &[crate::contracts::prepared_v2::Effect]) {
        for effect in effects {
            if let crate::contracts::prepared_v2::Effect::DamageOnUse {
                spell,
                periodic: Some(periodic),
                ..
            } = effect
            {
                if let Some(dot) = self.spells.get(*spell).and_then(|spell| spell.dot) {
                    self.dots[dot].tick_can_crit = periodic.tick_can_crit;
                }
            }
        }
    }

    /// The spell's `ApplyEffects`.
    pub(crate) fn apply_on_use_damage(
        &mut self,
        spell: SpellId,
        target: Side,
        params: OnUseDamage,
    ) {
        if let Some(spread) = params.spread {
            self.apply_proc_damage(spell, target, spread);
            return;
        }
        let result = match (params.direct, params.application) {
            (Some((average, variance, scale, outcome)), _) => {
                let base = self.effect_roll(average, variance) * scale;
                self.on_use_hit(spell, target, base, outcome)
            }
            (None, Some(outcome)) => {
                let outcome = match outcome {
                    OnUseOutcome::MagicHit | OnUseOutcome::MagicHitAndCrit => Outcome::MagicHit,
                    OnUseOutcome::MeleeSpecialHit | OnUseOutcome::MeleeSpecialHitAndCrit => {
                        Outcome::Table(PhysicalOutcome::MeleeSpecialHit { count: true })
                    }
                };
                self.calc_outcome(spell, target, outcome)
            }
            (None, None) => panic!("an on-use damage spell deals damage"),
        };
        let dot = params.tick_base.and(self.spells[spell].dot);
        if self.spells[spell].missile_speed > 0.0 {
            match dot {
                Some(dot) => self.deal_damage_after_travel_then_dot(spell, result, dot),
                None => self.deal_damage_after_travel(spell, result),
            }
            return;
        }
        self.deal_damage(spell, result, false);
        if let (Some(dot), true) = (dot, result.landed()) {
            self.apply_dot(dot);
        }
    }

    fn on_use_hit(
        &mut self,
        spell: SpellId,
        target: Side,
        base: f64,
        outcome: OnUseOutcome,
    ) -> SpellResult {
        match outcome {
            OnUseOutcome::MagicHitAndCrit => self.calc_damage(spell, target, base),
            OnUseOutcome::MagicHit => self.calc_damage_hit_only(spell, target, base),
            OnUseOutcome::MeleeSpecialHitAndCrit => self.calc_physical_damage(
                spell,
                target,
                base,
                PhysicalOutcome::MeleeSpecialHitAndCrit { count: true },
            ),
            OnUseOutcome::MeleeSpecialHit => self.calc_physical_damage(
                spell,
                target,
                base,
                PhysicalOutcome::MeleeSpecialHit { count: true },
            ),
        }
    }
}
