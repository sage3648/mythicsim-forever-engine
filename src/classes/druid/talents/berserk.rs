//! Berserk (417141), from Go sim/druid/talents_feral_combat.go `applyBerserk`: a major
//! cooldown whose aura raises the builders' critical strike chance.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Berserk {
    pub(crate) aura: AuraRef,
    crit_mod: ModId,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    crit_percent: f64,
    crit_spells: &[usize],
) -> Result<Berserk, String> {
    let aura = fight.player_aura(aura)?;
    let len = fight.spells.len();
    let spells: Vec<SpellId> = crit_spells
        .iter()
        .copied()
        .filter(|&spell| spell < len)
        .collect();
    let crit_mod = fight.register_mod(ModKind::BonusCritPercent, crit_percent, 0, spells);
    Ok(Berserk { aura, crit_mod })
}

impl Berserk {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.crit_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.crit_mod);
    }
}
