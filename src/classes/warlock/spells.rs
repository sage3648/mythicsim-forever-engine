//! Warlock spell mechanics reusable by any Warlock build that casts them.

pub(crate) mod bane_of_agony;
pub(crate) mod conflagrate;
pub(crate) mod corruption;
pub(crate) mod curse_of_the_elements;
pub(crate) mod immolate;
pub(crate) mod life_tap;
pub(crate) mod searing_pain;
pub(crate) mod shadow_bolt;
pub(crate) mod shadowburn;
pub(crate) mod soul_fire;

use crate::core::fight::{Agent, DotId, Fight, SpellId};

/// The untagged spell with an exported spell ID.
pub(crate) fn find_spell<A: Agent>(fight: &Fight<A>, spell_id: i32) -> Result<SpellId, String> {
    fight
        .spells
        .iter()
        .position(|spell| spell.id.spell_id == spell_id && spell.id.tag == 0)
        .ok_or_else(|| format!("Warlock spell {spell_id} is not registered"))
}

/// Give a spell's snapshot dot the base amount `Dot.Snapshot` starts from and whether its
/// ticks crit, as Go sim/warlock/warlock.go `periodicTickOutcome` picks. Returns the dot.
pub(crate) fn bind_snapshot_dot<A: Agent>(
    fight: &mut Fight<A>,
    spell_id: i32,
    tick_base: f64,
    tick_can_crit: bool,
) -> Result<DotId, String> {
    let spell = find_spell(fight, spell_id)?;
    let dot = fight
        .spell_dot(spell)
        .ok_or_else(|| format!("Warlock spell {spell_id} has no dot"))?;
    fight.dots[dot].tick_base = Some(tick_base);
    fight.dots[dot].tick_can_crit = tick_can_crit;
    Ok(dot)
}
