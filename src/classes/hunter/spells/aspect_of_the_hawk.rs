//! Aspect of the Hawk (13165 to 25296) and Aspect of the Beast (13161 to 1299447) with Deadly
//! Aspects (19552), from Go sim/hunter/aspects.go. The cast, while the aspect is down,
//! activates its aura, whose attack power is a stat aura (Go `AddStatDynamic`). Hawk's aura
//! rolls Deadly Aspects' chance on every ranged auto hit it hears, landed or not, for Quick
//! Shots' ranged attack speed; Beast's rolls it on landed melee white hits for Quick Strikes'
//! melee speed.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

/// A bound aspect.
#[derive(Clone, Debug)]
pub(crate) struct AspectOfTheHawk {
    pub(crate) aura: AuraRef,
    /// The aura's bit among the stat auras.
    stat_bit: u32,
    /// The haste aura, its multiplier and Deadly Aspects' chance, with the talent.
    pub(crate) quick_shots: Option<(AuraRef, f64, f64)>,
    /// Which spells the proc hears, by spellbook position: Go `ProcMaskRangedAuto` for Hawk,
    /// `ProcMaskMeleeWhiteHit` for Beast.
    trigger_spells: Vec<bool>,
    /// Beast's melee proc, which needs a landed hit and changes melee speed.
    melee: bool,
}

impl AspectOfTheHawk {
    pub(crate) fn bind<A: Agent>(
        fight: &Fight<A>,
        effects: &[crate::contracts::prepared_v2::Effect],
        prepared_spells: &[crate::contracts::prepared_v2::Spell],
        aura: &str,
        quick_shots: Option<(&str, f64, f64)>,
        melee: bool,
    ) -> Result<Self, String> {
        let stat_bit = Fight::<A>::stat_aura_bit(effects, aura)
            .ok_or_else(|| format!("{aura} is not a stat aura"))?;
        let quick_shots = match quick_shots {
            Some((label, haste, chance)) => Some((fight.player_aura(label)?, haste, chance)),
            None => None,
        };
        let masks: &[&str] = if melee {
            &["ProcMaskMeleeMHAuto", "ProcMaskMeleeOHAuto"]
        } else {
            &["ProcMaskRangedAuto"]
        };
        Ok(AspectOfTheHawk {
            aura: fight.player_aura(aura)?,
            stat_bit,
            quick_shots,
            trigger_spells: prepared_spells
                .iter()
                .map(|spell| {
                    spell
                        .proc_mask
                        .iter()
                        .any(|mask| masks.contains(&mask.as_str()))
                })
                .collect(),
            melee,
        })
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.set_stat_aura(self.stat_bit, true);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.set_stat_aura(self.stat_bit, false);
    }

    /// The aura's `OnSpellHitDealt`.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let Some((quick_shots, _, chance)) = self.quick_shots else {
            return;
        };
        if !self.trigger_spells.get(spell).copied().unwrap_or(false)
            || (self.melee && !result.landed())
        {
            return;
        }
        if fight.proc(chance, "Deadly Aspects") {
            fight.activate_aura(quick_shots);
        }
    }

    /// The haste aura's gain and expiry.
    pub(crate) fn quick_shots_changed<A: Agent>(&self, fight: &mut Fight<A>, gained: bool) {
        let (_, haste, _) = self.quick_shots.expect("the haste aura is bound");
        let amount = if gained { haste } else { 1.0 / haste };
        if self.melee {
            fight.multiply_melee_speed(amount);
        } else {
            fight.multiply_ranged_speed(amount);
        }
    }
}
