//! Shadow Weaving (15257, stacks 15258), from Go sim/priest/talents_shadow.go
//! `applyShadowWeaving`: each landed Shadow spell stacks a debuff on its target, to five, and the
//! target's dynamic damage taken modifier raises this priest's Shadow damage on it by each stack
//! (community #777: each target keeps its own). The trigger is a Go proc trigger on spell hits
//! dealt.

use crate::core::fight::{Agent, AuraRef, DamageTakenModifier, Fight, Side, SpellId, SpellResult};

/// Go `SpellSchoolShadow`.
const SHADOW: u8 = 32;

#[derive(Clone, Debug)]
pub(crate) struct ShadowWeaving {
    /// The debuff on the main target; [`Fight::aura_on`] finds each target's.
    pub(crate) aura: AuraRef,
    trigger: AuraRef,
    proc_chance: f64,
    immediate: bool,
    trigger_spells: Vec<bool>,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    trigger_aura: &str,
    proc_chance: f64,
    immediate: bool,
    trigger_spells: &[usize],
    damage_per_stack: f64,
) -> Result<ShadowWeaving, String> {
    let index = fight.trackers[Side::Target.index()]
        .find(aura)
        .ok_or_else(|| format!("target aura {aura} is not registered"))?;
    let aura = AuraRef {
        side: Side::Target,
        index,
    };
    let trigger = fight.player_aura(trigger_aura)?;
    // Go checks the spell's unit is the priest, so a pet's Shadow damage gains nothing.
    fight.damage_taken_modifiers.push(DamageTakenModifier {
        source: Side::Player,
        school_mask: SHADOW,
        aura,
        multiplier: 1.0,
        per_stack: Some(damage_per_stack),
    });
    let len = fight.spells.len();
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
            self.handler(fight, result.target);
        } else {
            fight.schedule_delayed_proc(self.trigger, spell, *result);
        }
    }

    /// The handler: the struck target's debuff activates, then one more stack.
    pub(crate) fn handler<A: Agent>(&self, fight: &mut Fight<A>, target: Side) {
        let aura = fight.aura_on(self.aura, target);
        fight.activate_aura(aura);
        fight.add_stack(aura);
    }
}
