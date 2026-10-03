//! Inner Focus (14751), from Go sim/priest/talents_discipline.go `applyInnerFocus`: its aura
//! makes the next priest spell free and adds crit to the spells the client lists. Any priest
//! spell's completed cast spends it, and its cooldown restarts when the aura ends.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct InnerFocus {
    pub(crate) aura: AuraRef,
    spell: SpellId,
    cost_percent: i32,
    crit_mod: ModId,
    spenders: Vec<bool>,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    aura: &str,
    cost_percent: i32,
    crit_percent: f64,
    crit_spells: &[usize],
    spender_spells: &[usize],
) -> Result<InnerFocus, String> {
    let aura = fight.player_aura(aura)?;
    let len = fight.spells.len();
    let affected = crit_spells.iter().copied().filter(|&s| s < len).collect();
    let crit_mod = fight.register_mod(ModKind::BonusCritPercent, crit_percent, 0, affected);
    let mut spenders = vec![false; len];
    for &spender in spender_spells {
        if spender < len {
            spenders[spender] = true;
        }
    }
    Ok(InnerFocus {
        aura,
        spell,
        cost_percent,
        crit_mod,
        spenders,
    })
}

impl InnerFocus {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.player.spell_cost_percent_modifier += self.cost_percent;
        fight.activate_mod(self.crit_mod);
    }

    /// The aura's OnExpire: Go `CD.Use` restarts the cooldown from now.
    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.player.spell_cost_percent_modifier -= self.cost_percent;
        fight.deactivate_mod(self.crit_mod);
        if let Some((timer, duration)) = fight.spells[self.spell].cd {
            fight.timers[timer] = fight.now + duration;
        }
    }

    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        if self.spenders[spell] {
            fight.deactivate_aura(self.aura);
        }
    }
}
