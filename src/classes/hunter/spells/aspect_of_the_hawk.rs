//! Aspect of the Hawk (13165 to 25296) with Deadly Aspects (19552), from Go
//! sim/hunter/aspects.go. The cast, while the aspect is down, activates its aura, whose ranged
//! attack power is a stat aura (Go `AddStatDynamic`). Every ranged auto hit the aura hears,
//! landed or not, rolls Deadly Aspects' chance for Quick Shots, which multiplies ranged attack
//! speed while it lasts.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId};

/// The bound aspect.
#[derive(Clone, Debug)]
pub(crate) struct AspectOfTheHawk {
    pub(crate) aura: AuraRef,
    /// The aura's bit among the stat auras.
    stat_bit: u32,
    /// Quick Shots, its haste multiplier and Deadly Aspects' chance, with the talent.
    pub(crate) quick_shots: Option<(AuraRef, f64, f64)>,
    /// Which spells carry Go `ProcMaskRangedAuto`, by spellbook position.
    ranged_autos: Vec<bool>,
}

impl AspectOfTheHawk {
    pub(crate) fn bind<A: Agent>(
        fight: &Fight<A>,
        effects: &[crate::contracts::prepared_v2::Effect],
        prepared_spells: &[crate::contracts::prepared_v2::Spell],
        aura: &str,
        quick_shots: Option<(&str, f64, f64)>,
    ) -> Result<Self, String> {
        let stat_bit = Fight::<A>::stat_aura_bit(effects, aura)
            .ok_or_else(|| format!("{aura} is not a stat aura"))?;
        let quick_shots = match quick_shots {
            Some((label, haste, chance)) => Some((fight.player_aura(label)?, haste, chance)),
            None => None,
        };
        Ok(AspectOfTheHawk {
            aura: fight.player_aura(aura)?,
            stat_bit,
            quick_shots,
            ranged_autos: prepared_spells
                .iter()
                .map(|spell| {
                    spell
                        .proc_mask
                        .iter()
                        .any(|mask| mask == "ProcMaskRangedAuto")
                })
                .collect(),
        })
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.set_stat_aura(self.stat_bit, true);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.set_stat_aura(self.stat_bit, false);
    }

    /// The aura's `OnSpellHitDealt`.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let Some((quick_shots, _, chance)) = self.quick_shots else {
            return;
        };
        if !self.ranged_autos[spell] {
            return;
        }
        if fight.proc(chance, "Deadly Aspects") {
            fight.activate_aura(quick_shots);
        }
    }

    /// Quick Shots' gain and expiry.
    pub(crate) fn quick_shots_changed<A: Agent>(&self, fight: &mut Fight<A>, gained: bool) {
        let (_, haste, _) = self.quick_shots.expect("Quick Shots is bound");
        fight.multiply_ranged_speed(if gained { haste } else { 1.0 / haste });
    }
}
