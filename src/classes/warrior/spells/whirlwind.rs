//! Whirlwind (1680), from Go sim/warrior/whirlwind.go: a normalized main hand strike on the
//! weapon special table against each target, the one in scope, dealt as a batch; with Raging
//! Blows the off hand's strike follows as its own cast.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

/// The main hand strike. `off_hand` is the off hand's spell when Raging Blows adds it and the
/// warrior holds an off hand weapon.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    off_hand: Option<SpellId>,
) {
    let attack_power = fight.melee_attack_power();
    let base = fight.mh_normalized_weapon_damage(attack_power);
    let outcome = PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
    if let Some(off_hand) = off_hand {
        fight.cast(off_hand, target);
    }
}

/// The off hand strike, Raging Blows' tagged spell.
pub(crate) fn apply_off_hand<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let attack_power = fight.melee_attack_power();
    let base = fight.oh_normalized_weapon_damage(attack_power);
    let outcome = PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true };
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    fight.deal_damage(spell, result, false);
}
