//! Wrack (1316697), from Go sim/warlock/wrack.go: a shadow channel whose ticks Soul Siphon
//! scales, and while it runs the warlock's Corruption and Bane of Agony on the target take a
//! dynamic damage taken bonus. Neither has a direct hit, so the bonus reaches only ticks.

use crate::core::fight::{Agent, DotId, Fight, SpellDamageTakenModifier};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Wrack {
    pub(crate) dot: DotId,
    soul_siphon: f64,
}

/// Resolve the channel's dot and register the bonus on the target, after Improved Shadow
/// Bolt's, as Go registers talents before spells.
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    dot: DotId,
    soul_siphon: f64,
    dot_spells: &[usize],
    dot_bonus: f64,
) -> Wrack {
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
    Wrack { dot, soul_siphon }
}

impl Wrack {
    /// `OnTick` of a target's dot: the snapshot damage, Soul Siphon, then the tick dealt.
    pub(crate) fn tick<A: Agent>(&self, fight: &mut Fight<A>, dot: DotId) {
        let mut result = fight.snapshot_dot_tick_calc(dot);
        result.damage *= self.soul_siphon;
        let spell = fight.dots[dot].spell;
        fight.deal_damage(spell, result, true);
    }
}
