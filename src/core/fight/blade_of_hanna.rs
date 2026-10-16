//! Go common/forever/items_70425.go Blade of Hanna (2801): each landed main hand swing adds a
//! stack of Improving Form; at its most the stacks go, Perfected Form strikes the target for its
//! flat Physical damage on the melee special table, and its stats aura comes up.

use crate::contracts::prepared_v2::Effect;

use super::{Agent, AuraRef, Fight, Side, SpellId, SpellResult, OUTCOME_LANDED};

/// The bound trigger and its auras.
#[derive(Clone, Debug)]
pub(crate) struct BladeOfHanna {
    /// Whether each spell, by spellbook position, is a hit the trigger hears.
    pub(crate) triggers: Vec<bool>,
    pub(crate) stacks: AuraRef,
    pub(crate) buff: AuraRef,
    pub(crate) strike: SpellId,
}

impl<A: Agent> Fight<A> {
    pub(crate) fn bind_blade_of_hanna(&mut self, effect: &Effect) -> Result<(), String> {
        let Effect::BladeOfHanna {
            stacks_aura,
            buff_aura,
            strike_spell,
            trigger_spells,
            ..
        } = effect
        else {
            return Ok(());
        };
        if *strike_spell >= self.spells.len() {
            return Err("Blade of Hanna names a spell the player does not have".into());
        }
        let mut triggers = vec![false; self.spells.len()];
        for &spell in trigger_spells {
            if let Some(slot) = triggers.get_mut(spell) {
                *slot = true;
            }
        }
        self.blade_of_hanna = Some(BladeOfHanna {
            triggers,
            stacks: self.player_aura(stacks_aura)?,
            buff: self.player_aura(buff_aura)?,
            strike: *strike_spell,
        });
        Ok(())
    }

    /// The trigger's handler, at once: a stack more, and at the most the strike and the buff.
    pub(crate) fn blade_of_hanna_callback(&mut self, spell: SpellId, result: &SpellResult) {
        let bound = self
            .blade_of_hanna
            .as_ref()
            .expect("Blade of Hanna is bound");
        if !bound.triggers[spell] || result.outcome & OUTCOME_LANDED == 0 {
            return;
        }
        let (stacks, buff, strike) = (bound.stacks, bound.buff, bound.strike);
        self.activate_aura(stacks);
        self.add_stack(stacks);
        if self.aura(stacks).stacks < self.aura(stacks).max_stacks {
            return;
        }
        self.deactivate_aura(stacks);
        let target = if result.target == Side::Player {
            Side::Target
        } else {
            result.target
        };
        self.cast(strike, target);
        self.activate_aura(buff);
    }
}
