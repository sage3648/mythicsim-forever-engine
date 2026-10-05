//! Volley (1510, 14294, 14295), from Go sim/hunter/volley.go: the channel holds the ranged
//! swing for the rank's duration and applies its area dot on the hunter, which snapshots the
//! Go literal tick of the rank. Every tick deals the snapshot to each target with
//! `OutcomeTick`, in unit index order.

use crate::core::fight::{Agent, DotId, Fight, Side, SpellId};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Volley {
    ranged_delay: i64,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    tick_base: f64,
    ranged_delay: i64,
) -> Result<Volley, String> {
    let dot = fight.spells[spell].dot.ok_or("Volley has no channel")?;
    fight.dots[dot].tick_base = Some(tick_base);
    Ok(Volley { ranged_delay })
}

impl Volley {
    /// The cast's `ApplyEffects`: `DelayRangedUntil`, then `AOEDot().Apply`.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        fight.delay_ranged_until(fight.now + self.ranged_delay);
        let dot = fight.spells[spell].dot.expect("Volley has a channel");
        fight.apply_dot(dot);
    }

    /// The area dot's `OnTick`.
    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>, dot: DotId) {
        let spell = fight.dots[dot].spell;
        for position in 0..fight.targets.len() {
            let result = fight.snapshot_dot_tick_calc_on(dot, Side::target(position));
            fight.deal_damage(spell, result, true);
        }
    }
}
