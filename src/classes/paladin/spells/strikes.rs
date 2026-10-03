//! Holy Strike, Hammer of Wrath and Consecration, from Go sim/paladin/holy_strike.go,
//! hammer_of_wrath.go and consecration.go.

use crate::{
    contracts::prepared_v2::ConsecrationRank,
    core::fight::{melee::PhysicalOutcome, DotId, Fight, Outcome, Side, SpellId},
};

use super::super::agent::PaladinAgent;

/// Holy Strike: the rank's percent of the normalized swing plus the flat roll, on the melee
/// table.
pub(crate) fn holy_strike(
    fight: &mut Fight<PaladinAgent>,
    spell: SpellId,
    target: Side,
    weapon_percent: f64,
) {
    let attack_power = fight.melee_attack_power();
    let weapon = fight.mh_normalized_weapon_damage(attack_power);
    let flat = fight.roll_damage_effect(spell);
    let base = weapon_percent * (weapon + flat);
    let result = fight.calc_damage_with(
        spell,
        target,
        base,
        Outcome::Table(PhysicalOutcome::MeleeSpecialHitAndCrit { count: true }),
    );
    fight.deal_damage(spell, result, false);
}

/// Hammer of Wrath: the rolled damage on the ranged table, dealt when the hammer lands.
pub(crate) fn hammer_of_wrath(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage_with(
        spell,
        target,
        base,
        Outcome::Table(PhysicalOutcome::RangedHitAndCrit { count: true }),
    );
    fight.deal_damage_after_travel(spell, result);
}

/// Hammer of Wrath's `ModifyCast`: only a cast pauses the swing.
pub(crate) fn hammer_of_wrath_modify_cast(fight: &mut Fight<PaladinAgent>, spell: SpellId) {
    let cast_time = fight.spells[spell].cur_cast.cast_time;
    let cast_time = fight.apply_cast_speed_for_spell(cast_time, spell);
    if cast_time > 0 {
        let until = fight.now + cast_time;
        fight.stop_melee_until(until);
    }
}

/// Consecration's cast: one magic hit check, then the ground effect regardless.
pub(crate) fn consecration(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHit);
    fight.deal_damage(spell, result, false);
    let dot = fight.spells[spell].dot.expect("Consecration has a dot");
    fight.apply_dot(dot);
}

/// A Consecration tick on the one target, which is among the first that take the bonus.
pub(crate) fn consecration_tick(
    fight: &mut Fight<PaladinAgent>,
    dot: DotId,
    rank: &ConsecrationRank,
) {
    let spell = fight.dots[dot].spell;
    let mut damage = rank.tick;
    if rank.bonus_targets > 0 {
        damage += rank.bonus + rank.bonus_coefficient * fight.bonus_damage(spell);
    }
    fight.periodic_damage_tick_with(dot, Side::Target, damage, Outcome::TickMagicHitAndCrit);
}
