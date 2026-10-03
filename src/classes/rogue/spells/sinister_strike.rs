//! Sinister Strike, from Go sim/rogue/sinister_strike.go: the highest rank's base plus
//! normalized main hand damage on the weapon special table. A landed strike gives a combo
//! point; a missed one refunds most of its energy.

use crate::core::fight::{melee::PhysicalOutcome, Agent, Fight, Side, SpellId};

use super::finisher::combo_point_metrics;

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, base: f64) {
    let attack_power = fight.melee_attack_power();
    let damage = base + fight.mh_normalized_weapon_damage(attack_power);
    let result = fight.calc_physical_damage(
        spell,
        target,
        damage,
        PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
    );
    fight.deal_damage(spell, result, false);
    if result.landed() {
        let metrics = combo_point_metrics(fight, spell);
        fight.add_combo_points(1, metrics);
    } else {
        fight.issue_refund(spell);
    }
}
