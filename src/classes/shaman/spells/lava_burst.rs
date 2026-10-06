//! Lava Burst, from Go sim/shaman/lava_burst.go: against a target burning with Flame Shock
//! the spell's damage multiplier takes the bonus for the hit's resolution and gives it back
//! after, in Go's order; the hit is dealt when the missile lands.

use crate::core::fight::{Agent, DotId, Fight, Side, SpellId};

/// Go `registerLavaBurstSpell` ApplyEffects.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    flame_shock: Option<DotId>,
    bonus: f64,
) {
    // Go `FlameShock.Dot(target).IsActive()`: the dot on the target being hit.
    let burning = flame_shock.is_some_and(|dot| {
        let on_target = fight.dot_on(dot, target);
        fight.aura(fight.dots[on_target].aura).active
    });
    if burning {
        fight.spells[spell].damage_multiplier *= bonus;
    }
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage_after_travel(spell, result);
    if burning {
        fight.spells[spell].damage_multiplier /= bonus;
    }
}
