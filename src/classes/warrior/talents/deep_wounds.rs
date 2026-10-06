//! Deep Wounds (12834, bleed 412609), from Go sim/warrior/talents_arms.go
//! `registerDeepWounds`: a landed physical crit, other than a proc's or an empty proc mask's,
//! casts the bleed at once. The cast lands without a hit counter, then the bleed restarts and
//! carries what the running one still owed plus a share of the main hand's average weapon
//! damage, spread over its ticks.

use crate::core::fight::{Agent, DotId, Fight, Side, SpellId, SpellResult, OUTCOME_CRIT};

/// Go `OutcomeHit`.
const OUTCOME_HIT: u16 = 1 << 1;

#[derive(Clone, Copy, Debug)]
pub(crate) struct DeepWounds {
    /// The bleed spell and its dot.
    pub(crate) spell: SpellId,
    pub(crate) dot: DotId,
    pub(crate) share: f64,
    /// spelldata `TickOutcome`: the client row lets its ticks roll the physical crit.
    pub(crate) tick_can_crit: bool,
}

/// The trigger's OnSpellHitDealt, which acts at once. `empty_mask` is whether the spell's proc
/// mask holds `ProcMaskEmpty`, which the trigger excludes.
pub(crate) fn on_hit<A: Agent>(
    fight: &mut Fight<A>,
    params: DeepWounds,
    spell: SpellId,
    empty_mask: bool,
    result: &SpellResult,
) {
    let state = &fight.spells[spell];
    if state.flags.proc || empty_mask || result.outcome & OUTCOME_CRIT == 0 {
        return;
    }
    if state.school & 1 == 0 {
        return;
    }
    fight.cast(params.spell, result.target);
}

/// The bleed's `ApplyEffects`.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    params: DeepWounds,
) {
    // Go CalcAndDealOutcome with OutcomeAlwaysHitNoHitCounter: no damage and no counter.
    let state = &fight.spells[spell];
    let threat = state.flat_threat_bonus * fight.player.threat_multiplier;
    let result = SpellResult {
        armor_multiplier: 0.0,
        target,
        attacker: fight.spells[spell].caster,
        outcome: OUTCOME_HIT,
        damage: 0.0,
        threat,
    };
    fight.deal_damage(spell, result, false);
    let dot = fight.dot_on(params.dot, target);
    let aura = fight.dots[dot].aura;
    let owed = if fight.aura(aura).active {
        fight.dots[dot].snapshot_base * f64::from(fight.dots[dot].remaining_ticks)
    } else {
        0.0
    };
    fight.deactivate_aura(aura);
    fight.apply_dot(dot);
    let weapon = &fight.autos.mh.weapon;
    let average = (weapon.base_damage_min + weapon.base_damage_max) / 2.0;
    let ticks = fight.dots[dot].hasted_tick_count();
    // The pinned Go engine runs on arm64, where the compiler fuses `owed + average*share`
    // into one multiply-add; the stored amount is read on every tick, so the rounding shows in
    // the log. Two sweep variants without a main hand weapon round differently otherwise.
    fight.dots[dot].snapshot_base = average.mul_add(params.share, owed) / f64::from(ticks);
}

/// A tick: the stored amount on the physical periodic path.
pub(crate) fn tick<A: Agent>(fight: &mut Fight<A>, dot: DotId, tick_can_crit: bool) {
    let state = &fight.dots[dot];
    let (spell, side, base) = (state.spell, state.side, state.snapshot_base);
    let result = fight.calc_physical_periodic_damage(spell, side, base, tick_can_crit);
    fight.deal_damage(spell, result, true);
}
