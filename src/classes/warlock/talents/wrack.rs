//! Wrack (1316697): a shadow channel whose ticks Soul Siphon scales.
//!
//! Under Forever's rules since the class tuning of 11 November 2026, while the channel runs the
//! warlock's Siphon Life, Corruption and Bane of Agony on the target "deal their periodic damage
//! 100% faster" (`dot_rate` 2): each ticks twice as often and lasts as long, so it deals twice
//! the ticks while the channel is on it. The effect is a server-side script, so that reading is
//! provisional. The other one, a dot that runs out in half the time with the ticks it had,
//! costs Affliction about 14% on its best rotation, where Blizzard's post says the change is
//! made "without affecting Affliction Warlocks' total damage output"; this one keeps it within
//! a few percent. Combat logs of a dot under the channel settle it.
//!
//! The Go reference, from sim/warlock/wrack.go, is the capstone as it was before: while it runs
//! the warlock's Corruption and Bane of Agony on the target take a dynamic damage taken bonus
//! (`dot_bonus`). Neither has a direct hit, so the bonus reaches only ticks.

use crate::core::fight::{Agent, DotId, Fight, SpellDamageTakenModifier};

#[derive(Clone, Debug)]
pub(crate) struct Wrack {
    pub(crate) dot: DotId,
    pub(crate) soul_siphon: f64,
    /// The first target's dots the channel speeds up, and the rate they tick at under it.
    pub(crate) rated_dots: Vec<DotId>,
    pub(crate) dot_rate: f64,
}

/// Resolve the channel's dot and register the bonus on the target, after Improved Shadow
/// Bolt's, as Go registers talents before spells.
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    dot: DotId,
    soul_siphon: f64,
    dot_spells: &[usize],
    dot_bonus: f64,
    dot_rate: f64,
) -> Wrack {
    if dot_bonus != 1.0 {
        let mut spells = vec![false; fight.spells.len()];
        for &spell in dot_spells {
            spells[spell] = true;
        }
        let aura = fight.dots[dot].aura;
        fight
            .spell_damage_taken_modifiers
            .push(SpellDamageTakenModifier {
                spells,
                auras: vec![aura],
                multiplier: dot_bonus,
            });
    }
    let rated_dots = if dot_rate == 1.0 {
        Vec::new()
    } else {
        dot_spells
            .iter()
            .filter_map(|&spell| fight.spell_dot(spell))
            .collect()
    };
    Wrack {
        dot,
        soul_siphon,
        rated_dots,
        dot_rate,
    }
}

/// `OnTick` of a target's dot: the snapshot damage, Soul Siphon's `soul_siphon`, then the tick
/// dealt.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, dot: DotId, soul_siphon: f64) {
    let mut result = fight.snapshot_dot_tick_calc(dot);
    let spell = fight.dots[dot].spell;
    fight.scale_damage(spell, &mut result, soul_siphon);
    fight.deal_damage(spell, result, true);
}

/// The channel starts (`rate` the channel's) or ends (`rate` 1) on the target `channel` is on:
/// the warlock's dots there take that rate.
pub(crate) fn set_dot_rate<A: Agent>(
    fight: &mut Fight<A>,
    channel: DotId,
    rated_dots: &[DotId],
    rate: f64,
) {
    let target = fight.dots[channel].side;
    for &dot in rated_dots {
        let dot = fight.dot_on(dot, target);
        fight.set_dot_tick_rate(dot, rate);
    }
}
