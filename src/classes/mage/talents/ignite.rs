//! Ignite (talent 11119, dot 412538), from Go sim/mage/talents_fire.go `registerIgnite`.
//! A Fire spell's crit pays a share of its damage over the dot's ticks. A crit landing
//! while the dot runs rolls what it still owes into the new dot, so nothing is paid twice
//! or dropped. The share comes from a hit already through every multiplier, so the ticks
//! skip them, never roll a resist and never crit.

use crate::core::fight::{Agent, DotId, Fight, Side, SpellId, SpellResult};

/// Go `SpellSchoolFire`.
const FIRE: u8 = 4;

#[derive(Clone, Debug)]
pub(crate) struct Ignite {
    spell: SpellId,
    dot: DotId,
    share: f64,
    num_ticks: i32,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    spell_id: i32,
    share: f64,
    num_ticks: i32,
) -> Result<Ignite, String> {
    let spell = fight
        .spells
        .iter()
        .position(|spell| spell.id.spell_id == spell_id && spell.id.tag == 0)
        .ok_or_else(|| format!("Ignite spell {spell_id} is not registered"))?;
    let dot = fight.spells[spell]
        .dot
        .ok_or_else(|| format!("Ignite spell {spell_id} has no dot"))?;
    Ok(Ignite {
        spell,
        dot,
        share,
        num_ticks,
    })
}

impl Ignite {
    /// The dot on a target, whose aura holds the ticks still owed there.
    fn dot_on<A: Agent>(&self, fight: &Fight<A>, target: Side) -> DotId {
        fight.dot_on(self.dot, target)
    }

    /// Go Ignite `ApplyEffects`: `Dot(target).Apply`.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, target: Side) {
        let dot = self.dot_on(fight, target);
        fight.apply_dot(dot);
    }

    /// The tick of a target's dot: the stored amount, as Go `CalcAndDealPeriodicDamage` with
    /// `OutcomeTick`.
    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>, dot: DotId) {
        let base = fight.dots[dot].snapshot_base;
        fight.periodic_damage_tick(dot, base);
    }

    /// The trigger's OnSpellHitDealt: Go's proc trigger can proc from procs and matches
    /// `ProcMaskSpellDamage` crits of any Fire spell but Ignite itself on an enemy. The fork's
    /// patch 88 added the enemy condition, so the Goblin Sapper Charge's crit on the Mage, a
    /// unit with no Ignite dot, no longer reaches the handler.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let state = &fight.spells[spell];
        if !state.proc_spell_damage
            || !result.crit()
            || state.school & FIRE == 0
            || spell == self.spell
            || !result.target.is_target()
        {
            return;
        }
        let dot = self.dot_on(fight, result.target);
        let owed = if fight.aura(fight.dots[dot].aura).active {
            fight.dots[dot].snapshot_base * f64::from(fight.dots[dot].remaining_ticks)
        } else {
            0.0
        };
        fight.cast(self.spell, result.target);
        // Go's arm64 build fuses the share's multiply into the owed amount.
        fight.dots[dot].snapshot_base =
            result.damage.mul_add(self.share, owed) / f64::from(self.num_ticks);
    }
}
