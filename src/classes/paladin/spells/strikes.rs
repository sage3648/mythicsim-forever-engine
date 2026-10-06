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

/// Exorcism: the rolled damage, a magic hit and crit; it casts only at an Undead or Demon.
pub(crate) fn exorcism(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
}

/// Holy Wrath: a roll of the rank's damage for each Undead or Demon target in unit index
/// order, every hit calculated before any is dealt when the bolts arrive; any other target
/// takes nothing.
pub(crate) fn holy_wrath(fight: &mut Fight<PaladinAgent>, spell: SpellId, hits: bool) {
    if !hits {
        return;
    }
    let sides: Vec<Side> = fight.target_sides().collect();
    let results: Vec<_> = sides
        .into_iter()
        .map(|side| {
            let base = fight.roll_damage_effect(spell);
            fight.calc_damage(spell, side, base)
        })
        .collect();
    fight.deal_damage_after_travel_batch(spell, &results);
}

/// Holy Wrath's `ModifyCast`: the cast pauses the swing.
pub(crate) fn holy_wrath_modify_cast(fight: &mut Fight<PaladinAgent>, spell: SpellId) {
    let cast_time = fight.spells[spell].cur_cast.cast_time;
    let cast_time = fight.apply_cast_speed_for_spell(cast_time, spell);
    let until = fight.now + cast_time;
    fight.stop_melee_until(until);
}

/// Consecration's cast: one magic hit check, then the ground effect regardless.
pub(crate) fn consecration(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHit);
    fight.deal_damage(spell, result, false);
    let dot = fight.spells[spell].dot.expect("Consecration has a dot");
    fight.apply_dot(dot);
}

/// A Consecration tick, which Go deals to each target in unit index order: the first targets
/// take the bonus and have Consecrated Ground marked before their tick lands, and each hit is
/// dealt before the next target's is calculated.
pub(crate) fn consecration_tick(
    fight: &mut Fight<PaladinAgent>,
    dot: DotId,
    rank: &ConsecrationRank,
) {
    let spell = fight.dots[dot].spell;
    let sides: Vec<Side> = fight.target_sides().collect();
    for (position, side) in sides.into_iter().enumerate() {
        let mut damage = rank.tick;
        if i32::try_from(position).is_ok_and(|position| position < rank.bonus_targets) {
            // Go's arm64 build fuses the bonus's coefficient into its average, then adds it.
            damage += rank
                .bonus_coefficient
                .mul_add(fight.bonus_damage(spell, side), rank.bonus);
            if let Some((aura, _)) = fight.agent.consecrated_ground {
                let marked = fight.aura_on(aura, side);
                fight.activate_aura(marked);
            }
        }
        fight.periodic_damage_tick_with(dot, side, damage, Outcome::TickMagicHitAndCrit);
    }
}
