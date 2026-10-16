//! Heating Up (talent 400624, buff 400625), from Go sim/mage/talents_fire.go
//! `registerHotStreak`: crits of Fireball, Frostfire Bolt, Fire Blast and Scorch stack
//! a buff that cuts Pyroblast's cast time per stack; the next Pyroblast spends them all. Since
//! the 16 October 2026 beta notes a Fire Blast crit adds 2 stacks, and a Pyroblast cast under
//! Presence of Mind leaves them in place.

use crate::{
    classes::mage::masks::is_class,
    core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult},
};

/// Go `MageSpellFireball | MageSpellFrostfireBolt | MageSpellFireBlast | MageSpellScorch`.
const STACKERS: &[&str] = &["fireball", "frostfire_bolt", "fire_blast", "scorch"];

#[derive(Clone, Debug)]
pub(crate) struct HeatingUp {
    pub(crate) aura: AuraRef,
    cast_time_per_stack: f64,
    cast_time_mod: ModId,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    cast_time_per_stack: f64,
) -> Result<HeatingUp, String> {
    let aura = fight.player_aura(aura)?;
    let pyroblasts = fight.spells_with_class(&["pyroblast"]);
    let cast_time_mod = fight.register_mod(ModKind::CastTimePercent, 0.0, 0, pyroblasts);
    Ok(HeatingUp {
        aura,
        cast_time_per_stack,
        cast_time_mod,
    })
}

impl HeatingUp {
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cast_time_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cast_time_mod);
    }

    pub(crate) fn on_stacks_change<A: Agent>(&self, fight: &mut Fight<A>, stacks: i32) {
        fight.update_mod_value(
            self.cast_time_mod,
            self.cast_time_per_stack * f64::from(stacks),
        );
    }

    /// The buff's OnCastComplete: Pyroblast spends every stack, unless it was cast under
    /// Presence of Mind, which drops its own aura in the same callback round, so the cast's start
    /// recorded it.
    pub(crate) fn on_cast_complete<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        under_presence_of_mind: bool,
    ) {
        if fight.spells[spell].class_spell.as_deref() == Some("pyroblast")
            && !under_presence_of_mind
        {
            fight.deactivate_aura(self.aura);
        }
    }

    /// The trigger's OnSpellHitDealt: every crit of a stacking spell adds a stack.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let state = &fight.spells[spell];
        if state.flags.proc || !is_class(state.class_spell.as_deref(), STACKERS) || !result.crit() {
            return;
        }
        let fire_blast = state.class_spell.as_deref() == Some("fire_blast");
        fight.activate_aura(self.aura);
        // A Fire Blast crit grants 2 charges in one change, every other crit 1 (cap 3).
        if fire_blast {
            let stacks = fight.aura(self.aura).stacks;
            fight.set_stacks(self.aura, stacks + 2);
        } else {
            fight.add_stack(self.aura);
        }
    }
}
