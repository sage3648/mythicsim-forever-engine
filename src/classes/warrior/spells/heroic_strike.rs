//! Heroic Strike (25286) and Cleave (20569), from Go sim/warrior/heroic_strike_cleave.go. A
//! tagged queue cast arms the strike's queue aura after a realism delay; the next main hand
//! swing then casts the strike in its place on the weapon special table, provided it can
//! still be paid for. The dual wield miss penalty the strike lifts never applies to that table.

use crate::core::fight::{
    melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId, SpellResult, TimerId,
};

/// One strike: the strike spell and its queue aura.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Strike {
    pub(crate) spell: SpellId,
    pub(crate) queue_aura: AuraRef,
    pub(crate) base_damage: f64,
    pub(crate) cleave: bool,
}

/// The queue state Go keeps in the `Warrior` struct and the queue closures.
#[derive(Clone, Debug, Default)]
pub(crate) struct Queue {
    pub(crate) strikes: Vec<Strike>,
    /// Go `queuedRealismICD`: its timer and duration.
    pub(crate) realism: Option<(TimerId, i64)>,
    /// Go `curQueueAura`, by strike.
    pub(crate) current: Option<usize>,
    /// Go `isQueueQueued`, by strike.
    pub(crate) queued: Vec<bool>,
    /// The `results` slice of Go's `registerCleave`, which every cast of Cleave shares: a cast
    /// that begins while another deals its results overwrites the ones that cast has yet to
    /// deal, as Weaponmaster's extra attack replaced by a queued Cleave does.
    pub(crate) cleave_results: Vec<SpellResult>,
}

impl Queue {
    fn realism_ready<A: Agent>(&self, fight: &Fight<A>) -> bool {
        self.realism
            .is_none_or(|(timer, _)| fight.timers[timer] <= fight.now)
    }
}

/// The queue cast's `ExtraCastCondition`.
pub(crate) fn queue_condition<A: Agent>(fight: &Fight<A>, queue: &Queue, strike: usize) -> bool {
    let spell = queue.strikes[strike].spell;
    queue.current.is_none()
        && !queue.queued[strike]
        && fight.current_rage() >= fight.current_cost(spell)
        && fight.player.hardcast.expires <= fight.now
        && queue.realism_ready(fight)
}

/// The queue cast's `ApplyEffects`: arm the aura after the realism delay. Returns the delay
/// to schedule, if any, and leaves `queued` set until it runs.
pub(crate) fn queue<A: Agent>(
    fight: &mut Fight<A>,
    queue: &mut Queue,
    strike: usize,
) -> Option<i64> {
    if !queue.realism_ready(fight) {
        return None;
    }
    queue.queued[strike] = true;
    let (timer, delay) = queue.realism.expect("the queue has a realism delay");
    fight.timers[timer] = fight.now + delay;
    Some(fight.now + delay)
}

/// Go `registerCleave`'s cap on the targets a Cleave hits.
const CLEAVE_TARGETS: usize = 2;

/// Go `registerCleave`'s `ApplyEffects` up to its deals: a hit on up to two targets from the
/// cast target on, each rolling its own weapon damage, all calculated before any is dealt.
/// The caller deals them from [`Queue::cleave_results`], as Go does from its shared slice.
pub(crate) fn cleave_results<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    params: Strike,
) -> Vec<SpellResult> {
    let outcome = PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true };
    let mut results = Vec::with_capacity(CLEAVE_TARGETS);
    for hit in fight.cleave_targets(target, CLEAVE_TARGETS) {
        let attack_power = fight.melee_attack_power();
        let base = params.base_damage + fight.mh_weapon_damage(attack_power);
        results.push(fight.calc_physical_damage(spell, hit, base, outcome));
    }
    results
}

/// Heroic Strike's `ApplyEffects`: it refunds a miss.
pub(crate) fn strike<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, params: Strike) {
    let outcome = PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true };
    let attack_power = fight.melee_attack_power();
    let base = params.base_damage + fight.mh_weapon_damage(attack_power);
    let result = fight.calc_physical_damage(spell, target, base, outcome);
    if !result.landed() {
        fight.issue_refund(spell);
    }
    fight.deal_damage(spell, result, false);
}
