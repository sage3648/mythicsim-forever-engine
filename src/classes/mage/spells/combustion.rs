//! Combustion (11129, buff 28682), from Go sim/mage/combustion.go: a major cooldown whose
//! buff adds Fire crit with every landed Fire hit of a damaging Mage spell, until the
//! row's number of crits is spent. The cooldown starts when the buff ends.

use std::cell::Cell;

use crate::{
    classes::mage::masks::{is_class, ALL, DAMAGING},
    core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult},
};

/// Go `SpellSchoolFire`.
const FIRE: u8 = 4;

#[derive(Debug)]
pub(crate) struct Combustion {
    pub(crate) aura: AuraRef,
    spell: SpellId,
    crit_per_stack: f64,
    max_crits: i32,
    crit_mod: ModId,
    /// Go's closure counter `numCrits`.
    crits: Cell<i32>,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    aura: &str,
    crit_per_stack: f64,
    max_crits: i32,
) -> Result<Combustion, String> {
    let aura = fight.player_aura(aura)?;
    let fire = fight.spells_with_class_and_school(ALL, FIRE);
    let crit_mod = fight.register_mod(ModKind::BonusCritPercent, 0.0, 0, fire);
    Ok(Combustion {
        aura,
        spell,
        crit_per_stack,
        max_crits,
        crit_mod,
        crits: Cell::new(0),
    })
}

/// Go Combustion `ApplyEffects`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, combustion: &Combustion) {
    fight.activate_aura(combustion.aura);
    fight.add_stack(combustion.aura);
}

impl Combustion {
    /// Go `ExtraCastCondition`: not while the buff is up.
    pub(crate) fn can_cast<A: Agent>(&self, fight: &Fight<A>) -> bool {
        !fight.aura(self.aura).active
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        self.crits.set(0);
        fight.activate_mod(self.crit_mod);
    }

    /// Go `cd.Use` and `UpdateMajorCooldowns`.
    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.crit_mod);
        if let Some((timer, duration)) = fight.spells[self.spell].cd {
            fight.timers[timer] = fight.now + duration;
        }
        fight.update_major_cooldowns();
    }

    pub(crate) fn on_stacks_change<A: Agent>(&self, fight: &mut Fight<A>, stacks: i32) {
        fight.update_mod_value(self.crit_mod, self.crit_per_stack * f64::from(stacks));
    }

    /// The buff's OnSpellHitDealt: Ignite's ticks are not a cast and never spend a charge.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let state = &fight.spells[spell];
        if !result.landed()
            || !is_class(state.class_spell.as_deref(), DAMAGING)
            || state.school & FIRE == 0
        {
            return;
        }
        fight.add_stack(self.aura);
        if result.crit() {
            self.crits.set(self.crits.get() + 1);
            if self.crits.get() >= self.max_crits {
                fight.deactivate_aura(self.aura);
            }
        }
    }
}
