//! Elemental Devastation (talent 30160), from Go sim/shaman/talents_elemental.go
//! `applyElementalDevastation`: a spell damage crit grants a melee crit buff one spell batch
//! window later. A crit from a spell flagged Proc, such as an overload, counts only where the
//! talent row allows it, and Fire Nova's never does (community #789).

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult};

#[derive(Clone, Debug)]
pub(crate) struct ElementalDevastation {
    pub(crate) trigger: AuraRef,
    pub(crate) aura: AuraRef,
    crit_mod: ModId,
    can_proc_from_procs: bool,
    /// Whether each spell, by spellbook position, never triggers it.
    excluded: Vec<bool>,
}

/// `melee` lists the spells with a melee proc mask, which the buff's modifier names.
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    trigger: &str,
    aura: &str,
    melee_crit: f64,
    melee: Vec<SpellId>,
    can_proc_from_procs: bool,
    excluded_class_spells: &[String],
) -> Result<ElementalDevastation, String> {
    let trigger = fight.player_aura(trigger)?;
    let aura = fight.player_aura(aura)?;
    let crit_mod = fight.register_mod(ModKind::BonusCritPercent, melee_crit, 0, melee);
    let names: Vec<&str> = excluded_class_spells.iter().map(String::as_str).collect();
    let mut excluded = vec![false; fight.spells.len()];
    for spell in fight.spells_with_class(&names) {
        excluded[spell] = true;
    }
    Ok(ElementalDevastation {
        trigger,
        aura,
        crit_mod,
        can_proc_from_procs,
        excluded,
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
        let state = &fight.spells[spell];
        if state.proc_spell_damage
            && (self.can_proc_from_procs || !state.flags.proc)
            && result.crit()
            && !self.excluded[spell]
        {
            fight.schedule_delayed_proc(self.trigger, spell, *result);
        }
    }

    /// The delayed handler.
    pub(crate) fn grant<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }
}
