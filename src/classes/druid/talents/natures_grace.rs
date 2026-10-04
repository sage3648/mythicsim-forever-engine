//! Nature's Grace (16880, aura 16886), from Go sim/druid/talents_balance.go
//! `applyNaturesGrace`: a damaging spell crit grants cast speed and a shorter GCD. Forever
//! replaces the cast time cut on the next cast with this haste buff.

use crate::core::fight::{
    Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult, OUTCOME_CRIT,
};

#[derive(Clone, Debug)]
pub(crate) struct NaturesGrace {
    pub(crate) aura: AuraRef,
    haste_multiplier: f64,
    trigger_spells: Vec<bool>,
    gcd_mod: ModId,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    haste_multiplier: f64,
    gcd_reduction: i64,
    gcd_spells: &[usize],
    trigger_spells: &[usize],
) -> Result<NaturesGrace, String> {
    let aura = fight.player_aura(aura)?;
    let len = fight.spells.len();
    let affected: Vec<SpellId> = gcd_spells
        .iter()
        .copied()
        .filter(|&spell| spell < len)
        .collect();
    let gcd_mod = fight.register_mod(ModKind::GlobalCooldownFlat, 0.0, gcd_reduction, affected);
    let mut mask = vec![false; len];
    for &spell in trigger_spells {
        if spell < len {
            mask[spell] = true;
        }
    }
    Ok(NaturesGrace {
        aura,
        haste_multiplier,
        trigger_spells: mask,
        gcd_mod,
    })
}

impl NaturesGrace {
    /// The trigger: a crit from a damaging Druid spell, handled at once.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if self.trigger_spells[spell] && result.outcome & OUTCOME_CRIT != 0 {
            fight.activate_aura(self.aura);
        }
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        // Go OnGain runs before the attached modifier activates.
        fight.multiply_cast_speed(self.haste_multiplier);
        fight.activate_mod(self.gcd_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.multiply_cast_speed(1.0 / self.haste_multiplier);
        fight.deactivate_mod(self.gcd_mod);
    }
}
