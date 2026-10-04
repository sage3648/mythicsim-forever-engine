//! Shadow Weaving (15257, stacks 15258), from Go sim/priest/talents_shadow.go
//! `applyShadowWeaving`: in Forever each landed Shadow spell stacks a Shadow damage bonus on
//! the priest, to five stacks. The trigger is a Go proc trigger on spell hits dealt.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult};

#[derive(Clone, Debug)]
pub(crate) struct ShadowWeaving {
    pub(crate) aura: AuraRef,
    trigger: AuraRef,
    proc_chance: f64,
    immediate: bool,
    trigger_spells: Vec<bool>,
    damage_per_stack: f64,
    damage_mod: ModId,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    trigger_aura: &str,
    proc_chance: f64,
    immediate: bool,
    trigger_spells: &[usize],
    damage_per_stack: f64,
    damage_spells: &[usize],
) -> Result<ShadowWeaving, String> {
    let aura = fight.player_aura(aura)?;
    let trigger = fight.player_aura(trigger_aura)?;
    let len = fight.spells.len();
    let affected = damage_spells.iter().copied().filter(|&s| s < len).collect();
    let damage_mod = fight.register_mod(ModKind::DamageDonePercent, 0.0, 0, affected);
    let mut mask = vec![false; len];
    for &spell in trigger_spells {
        if spell < len {
            mask[spell] = true;
        }
    }
    Ok(ShadowWeaving {
        aura,
        trigger,
        proc_chance,
        immediate,
        trigger_spells: mask,
        damage_per_stack,
        damage_mod,
    })
}

impl ShadowWeaving {
    /// The proc trigger: a landed hit from a Shadow priest spell, at the talent's chance.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.trigger_spells[spell] || !result.landed() {
            return;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return;
        }
        if self.immediate {
            self.handler(fight);
        } else {
            fight.schedule_delayed_proc(self.trigger, spell, *result);
        }
    }

    /// The handler: activate, then one more stack.
    pub(crate) fn handler<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
        fight.add_stack(self.aura);
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.damage_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.damage_mod);
    }

    pub(crate) fn on_stacks_change<A: Agent>(&self, fight: &mut Fight<A>, new: i32) {
        fight.update_mod_value(self.damage_mod, self.damage_per_stack * f64::from(new));
    }
}
