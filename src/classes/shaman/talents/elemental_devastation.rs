//! Elemental Devastation (talent 30160), from Go sim/shaman/talents_elemental.go
//! `applyElementalDevastation`: a spell damage crit, procs included, grants a melee crit buff
//! one spell batch window later.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ElementalDevastation {
    pub(crate) trigger: AuraRef,
    pub(crate) aura: AuraRef,
    crit_mod: ModId,
}

/// `melee` lists the spells with a melee proc mask, which the buff's modifier names.
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    trigger: &str,
    aura: &str,
    melee_crit: f64,
    melee: Vec<SpellId>,
) -> Result<ElementalDevastation, String> {
    let trigger = fight.player_aura(trigger)?;
    let aura = fight.player_aura(aura)?;
    let crit_mod = fight.register_mod(ModKind::BonusCritPercent, melee_crit, 0, melee);
    Ok(ElementalDevastation {
        trigger,
        aura,
        crit_mod,
    })
}

impl ElementalDevastation {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.crit_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.crit_mod);
    }

    /// The trigger: a spell damage crit schedules the buff.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if fight.spells[spell].proc_spell_damage && result.crit() {
            fight.schedule_delayed_proc(self.trigger, spell, *result);
        }
    }

    /// The delayed handler.
    pub(crate) fn grant<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }
}
