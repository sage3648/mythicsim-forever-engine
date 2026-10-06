//! Sweeping Strikes (12723, row 12292), from Go sim/warrior/talents_arms.go
//! `registerSweepingStrikes` and warrior.go `CastNormalizedSweepingStrikesAttack`: a major
//! cooldown that needs Battle Stance and a second target and activates an aura with the
//! row's charges. While two
//! targets are active a landed melee hit that dealt damage casts a copy on the next target and
//! spends a charge; Whirlwind and Thunder Clap spend one on a normalized main hand attack on
//! the target after the first landed hit instead.

use crate::core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SweepingStrikes {
    pub(crate) aura: AuraRef,
    pub(crate) charges: i32,
    /// Go `hitSpell`: a copy of a hit's damage before the target's armor.
    pub(crate) hit: SpellId,
    /// Go `SweepingStrikesNormalizedAttack`.
    pub(crate) normalized: SpellId,
}

/// The cast's `ApplyEffects`: activate the aura, then set its stacks to the charges.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, params: SweepingStrikes) {
    fight.activate_aura(params.aura);
    fight.set_stacks(params.aura, params.charges);
}

/// Go `CastNormalizedSweepingStrikesAttack`: with a second target and the aura up, the first
/// landed result of a cleave casts the normalized attack on the next target and spends a
/// charge. With one target the "additional nearby opponent" would wrap around to the same one,
/// so it does nothing.
pub(crate) fn cast_normalized<A: Agent>(
    fight: &mut Fight<A>,
    params: Option<SweepingStrikes>,
    results: &[SpellResult],
) {
    let Some(params) = params else {
        return;
    };
    if fight.targets.len() < 2 || !fight.aura(params.aura).active {
        return;
    }
    if let Some(result) = results.iter().find(|result| result.landed()) {
        let next = fight.next_target(result.target);
        fight.cast(params.normalized, next);
        fight.remove_stack(params.aura);
    }
}

/// The aura's `OnSpellHitDealt`, a proc trigger for landed melee hits that acts at once.
/// `copy_damage` is the damage the copy deals, which Go keeps in a closure for the hit spell's
/// next cast; the caller stores it.
pub(crate) fn on_hit<A: Agent>(
    fight: &mut Fight<A>,
    params: SweepingStrikes,
    spell: SpellId,
    result: &SpellResult,
) -> Option<f64> {
    let state = &fight.spells[spell];
    // Go's proc trigger skips a spell flagged a proc and wants a landed melee hit.
    if state.flags.proc || !state.melee_proc || !result.landed() {
        return None;
    }
    // PostOutcomeDamage is the damage before the target's dynamic modifiers, which no target
    // here has, so it is positive when the damage is.
    if fight.targets.len() < 2 || fight.aura(params.aura).stacks == 0 || result.damage <= 0.0 {
        return None;
    }
    if spell == params.hit || spell == params.normalized {
        return None;
    }
    let class_spell = state.class_spell.as_deref();
    if matches!(
        class_spell,
        Some("thunder_clap" | "whirlwind" | "whirlwind_off_hand")
    ) {
        return None;
    }
    let next = fight.next_target(result.target);
    if class_spell == Some("execute") && fight.is_execute_phase_20() {
        fight.cast(params.normalized, next);
        fight.remove_stack(params.aura);
        return None;
    }
    // Go divides by the armor and resistance multiplier the result was calculated with.
    Some(result.damage / result.armor_multiplier)
}

/// The copy's cast on the next target, then the spent charge.
pub(crate) fn copy<A: Agent>(fight: &mut Fight<A>, params: SweepingStrikes, hit_target: Side) {
    let next = fight.next_target(hit_target);
    fight.cast(params.hit, next);
    fight.remove_stack(params.aura);
}

/// The hit spell's `ApplyEffects`: `CalcAndDealDamage` of the copied damage with
/// `OutcomeAlwaysHit`.
pub(crate) fn apply_hit<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, damage: f64) {
    let result = fight.calc_damage_with(spell, target, damage, Outcome::AlwaysHit);
    fight.deal_damage(spell, result, false);
}

/// The normalized attack's `ApplyEffects`: the main hand's normalized weapon damage on
/// `OutcomeAlwaysHit`.
pub(crate) fn apply_normalized<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let attack_power = fight.melee_attack_power();
    let base = fight.mh_normalized_weapon_damage(attack_power);
    let result = fight.calc_damage_with(spell, target, base, Outcome::AlwaysHit);
    fight.deal_damage(spell, result, false);
}
