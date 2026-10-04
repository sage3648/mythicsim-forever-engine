//! Elemental Focus (talent 16164) and its Clearcasting (16246), from Go
//! sim/shaman/talents_elemental.go `applyElementalFocus`. A completed spell damage cast may
//! roll for Clearcasting, which makes the next Lightning Bolt, Chain Lightning, Lava Burst,
//! Fire Nova or shock free and is spent by it. The roll happens before the handler looks at
//! the spell, so a totem attack's or Flame Shock dot's cast still draws.

use crate::{
    classes::shaman::masks::{
        is_class, FOCUS_CONSUMERS, SCHOOL_ELEMENTAL, TOTEM_OR_FLAME_SHOCK_DOT,
    },
    core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ElementalFocus {
    pub(crate) trigger: AuraRef,
    pub(crate) clearcasting: AuraRef,
    proc_chance: f64,
    max_stacks: i32,
    cost_mod: ModId,
}

/// The cast that last granted Clearcasting, and when. Go keeps it in a closure that no reset
/// clears, so it carries into the next iteration.
pub(crate) type Trigger = Option<(SpellId, i64)>;

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    trigger: &str,
    clearcasting: &str,
    proc_chance: f64,
    cost_percent_add: f64,
    max_stacks: i32,
) -> Result<ElementalFocus, String> {
    let trigger = fight.player_aura(trigger)?;
    let clearcasting = fight.player_aura(clearcasting)?;
    let consumers = fight.spells_with_class(FOCUS_CONSUMERS);
    let cost_mod = fight.register_mod(ModKind::PowerCostPercentAdd, cost_percent_add, 0, consumers);
    Ok(ElementalFocus {
        trigger,
        clearcasting,
        proc_chance,
        max_stacks,
        cost_mod,
    })
}

impl ElementalFocus {
    /// Clearcasting's attached cost modifier.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cost_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cost_mod);
    }

    /// The "Elemental Focus" proc trigger: spell damage casts, procs included, roll first.
    /// Returns the trigger to record when the cast grants Clearcasting.
    pub(crate) fn roll<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) -> Trigger {
        if !fight.spells[spell].proc_spell_damage {
            return None;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return None;
        }
        let state = &fight.spells[spell];
        if state.school & SCHOOL_ELEMENTAL == 0
            || is_class(state.class_spell.as_deref(), TOTEM_OR_FLAME_SHOCK_DOT)
        {
            return None;
        }
        Some((spell, fight.now))
    }

    /// The handler's grant, once the trigger is recorded.
    pub(crate) fn grant<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.clearcasting);
        fight.set_stacks(self.clearcasting, self.max_stacks);
    }

    /// Clearcasting's OnCastComplete: a consuming cast spends a stack, unless it is the cast
    /// that just granted Clearcasting.
    pub(crate) fn consume<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, last: Trigger) {
        if !is_class(fight.spells[spell].class_spell.as_deref(), FOCUS_CONSUMERS) {
            return;
        }
        if last == Some((spell, fight.now)) {
            return;
        }
        fight.remove_stack(self.clearcasting);
    }
}
