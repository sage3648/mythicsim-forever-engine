//! Thousand Cuts (1310721), from Go sim/rogue/talents_subtlety.go `registerThousandCuts`: its
//! aura lowers the energy cost of Backstab and Hemorrhage by a flat amount a stack, through a
//! dynamic modifier updated on every stack change, and the next of them to apply its effects
//! spends the aura. The stacks come from the proc trigger on Rupture ticks.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct ThousandCuts {
    pub(crate) aura: AuraRef,
    cost_per_stack: i32,
    cost_mod: ModId,
    spends: Vec<bool>,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    cost_per_stack: i32,
    class_spells: &[String],
) -> Result<ThousandCuts, String> {
    let aura = fight.player_aura(aura)?;
    let names: Vec<&str> = class_spells.iter().map(String::as_str).collect();
    let cost_mod = fight.register_mod(
        ModKind::PowerCostFlat,
        0.0,
        0,
        fight.spells_with_class(&names),
    );
    let spends = fight
        .spells
        .iter()
        .map(|spell| {
            spell
                .class_spell
                .as_deref()
                .is_some_and(|class| names.contains(&class))
        })
        .collect();
    Ok(ThousandCuts {
        aura,
        cost_per_stack,
        cost_mod,
        spends,
    })
}

impl ThousandCuts {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cost_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cost_mod);
    }

    /// Go `UpdateIntValue` with the new stack count's discount.
    pub(crate) fn on_stacks_change<A: Agent>(&self, fight: &mut Fight<A>, stacks: i32) {
        fight.update_mod_value(self.cost_mod, f64::from(self.cost_per_stack * stacks));
    }

    /// The aura's `OnApplyEffects`.
    pub(crate) fn on_apply_effects<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        if self.spends[spell] {
            fight.deactivate_aura(self.aura);
        }
    }
}
