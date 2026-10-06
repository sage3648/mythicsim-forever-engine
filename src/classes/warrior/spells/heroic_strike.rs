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
    /// The `results` slice of Go's `registerCleave`, which every cast of Cleave shares, as
    /// pool objects: a cast that begins while another deals its results overwrites the ones
    /// that cast has yet to deal, as Weaponmaster's extra attack replaced by a queued Cleave
    /// does.
    pub(crate) cleave_slice: Vec<usize>,
    /// Go's cache of Cleave's result objects, which lives as long as the simulation does.
    pub(crate) pool: ResultPool,
    /// The objects of the Cleave hits being dealt, the innermost last.
    pub(crate) dealing: Vec<usize>,
}

/// One of Go's `SpellResult` objects.
#[derive(Clone, Copy, Debug)]
struct PoolObject {
    in_use: bool,
    result: SpellResult,
}

/// Go's `SpellResultCache` of one spell: the first object made for a target is kept for it and
/// handed out again once disposed of, and a request that finds it in use gets a new object
/// that nothing keeps. A result that is never disposed of, as one a later cast overwrote before
/// dealing it, keeps its target's object in use for good. Cleave's casts and the clones that
/// delayed procs take of its hits all draw on it.
#[derive(Clone, Debug, Default)]
pub(crate) struct ResultPool {
    cache: Vec<Option<usize>>,
    objects: Vec<PoolObject>,
}

impl ResultPool {
    /// Go `NewResult`: the object, with its fields reset to `result`'s unless it is a new one.
    /// `fresh` says whether `result` is the whole of it, as a calculated hit is, or a reset,
    /// which keeps the armor multiplier an object already had.
    fn new_result(&mut self, position: usize, result: SpellResult, fresh: bool) -> usize {
        if self.cache.len() <= position {
            self.cache.resize(position + 1, None);
        }
        let cached = self.cache[position];
        let id = match cached {
            Some(id) if !self.objects[id].in_use => id,
            _ => {
                self.objects.push(PoolObject {
                    in_use: false,
                    result: SpellResult {
                        armor_multiplier: 0.0,
                        ..result
                    },
                });
                let id = self.objects.len() - 1;
                if cached.is_none() {
                    self.cache[position] = Some(id);
                }
                id
            }
        };
        let object = &mut self.objects[id];
        object.in_use = true;
        let armor_multiplier = object.result.armor_multiplier;
        object.result = result;
        if !fresh {
            object.result.armor_multiplier = armor_multiplier;
        }
        id
    }

    /// Go `NewResult` for a hit calculated now.
    pub(crate) fn new_result_of(&mut self, result: &SpellResult) -> usize {
        let position = result
            .target
            .target_position()
            .expect("a cleave hits a target");
        self.new_result(position, *result, true)
    }

    /// Go `DisposeResult`.
    pub(crate) fn dispose(&mut self, id: usize) {
        self.objects[id].in_use = false;
    }

    pub(crate) fn result(&self, id: usize) -> SpellResult {
        self.objects[id].result
    }
}

impl Queue {
    /// The Cleave strike's spell, if the build has one.
    fn cleave_spell(&self) -> Option<SpellId> {
        self.strikes
            .iter()
            .find(|strike| strike.cleave)
            .map(|strike| strike.spell)
    }

    /// The hit of Cleave being dealt, as its listeners hear it.
    pub(crate) fn dealing_result(&self, spell: SpellId) -> Option<SpellResult> {
        let id = *self.dealing.last()?;
        (self.cleave_spell() == Some(spell)).then(|| self.pool.result(id))
    }

    /// Go `CloneResult` of a hit of Cleave for a delayed proc: it takes the target's object, which
    /// is the hit being dealt itself when that has been disposed of before, and resets it.
    pub(crate) fn clone_result(
        &mut self,
        spell: SpellId,
        result: &SpellResult,
    ) -> Option<(SpellResult, usize)> {
        let dealing = *self.dealing.last()?;
        if self.cleave_spell() != Some(spell) {
            return None;
        }
        let position = result.target.target_position()?;
        let reset = SpellResult {
            target: result.target,
            outcome: 0,
            damage: 0.0,
            threat: 0.0,
            armor_multiplier: 0.0,
        };
        let id = self.pool.new_result(position, reset, false);
        // The clone copies the hit it clones, which is the reset object itself when the two are
        // one.
        if id != dealing {
            let armor_multiplier = self.pool.objects[id].result.armor_multiplier;
            self.pool.objects[id].result = SpellResult {
                armor_multiplier,
                ..*result
            };
        }
        Some((self.pool.result(id), id))
    }

    /// Go `DisposeResult` of a clone, once its handler has run.
    pub(crate) fn dispose_clone(&mut self, token: usize) {
        self.pool.dispose(token);
    }
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
/// The caller deals them from [`Queue::cleave_slice`], as Go does from its shared slice.
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

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(position: usize, damage: f64) -> SpellResult {
        SpellResult {
            target: Side::target(position),
            outcome: 2,
            damage,
            threat: damage,
            armor_multiplier: 0.5,
        }
    }

    /// The first object made for a target is the one kept for it, and it is handed out again
    /// once disposed of; a request that finds it in use gets an object of its own.
    #[test]
    fn a_target_keeps_its_first_object_and_hands_it_out_when_free() {
        let mut pool = ResultPool::default();
        let first = pool.new_result_of(&hit(0, 1.0));
        let other = pool.new_result_of(&hit(0, 2.0));
        assert_ne!(first, other);
        pool.dispose(first);
        pool.dispose(other);
        assert_eq!(pool.new_result_of(&hit(0, 3.0)), first);
        // The object that was not kept is never handed out.
        pool.dispose(first);
        assert_eq!(pool.new_result_of(&hit(0, 4.0)), first);
    }

    /// A result nothing disposes of keeps its target's object in use for good.
    #[test]
    fn a_result_never_disposed_of_leaks_its_targets_object() {
        let mut pool = ResultPool::default();
        let leaked = pool.new_result_of(&hit(1, 1.0));
        let next = pool.new_result_of(&hit(1, 2.0));
        pool.dispose(next);
        assert_ne!(pool.new_result_of(&hit(1, 3.0)), leaked);
        assert_ne!(pool.new_result_of(&hit(1, 4.0)), leaked);
    }

    /// A clone of the hit being dealt takes that hit's own object once it is free, which
    /// resets it: the listeners after the clone hear an empty hit, and the clone is empty too.
    #[test]
    fn cloning_the_hit_being_dealt_resets_it() {
        let mut queue = Queue {
            strikes: vec![Strike {
                spell: 7,
                queue_aura: AuraRef {
                    side: Side::Player,
                    index: 0,
                },
                base_damage: 0.0,
                cleave: true,
            }],
            ..Queue::default()
        };
        let id = queue.pool.new_result_of(&hit(2, 5.0));
        queue.pool.dispose(id);
        // Dealt a second time, as a result a later cast left in the shared slice is.
        queue.pool.objects[id].in_use = false;
        queue.dealing.push(id);
        let heard = queue.dealing_result(7).unwrap();
        assert_eq!(heard.damage, 5.0);
        let (clone, token) = queue.clone_result(7, &heard).unwrap();
        assert_eq!(token, id);
        assert_eq!((clone.outcome, clone.damage, clone.threat), (0, 0.0, 0.0));
        let now = queue.dealing_result(7).unwrap();
        assert_eq!((now.outcome, now.damage), (0, 0.0));
        // Another spell's hits are nobody's concern.
        assert!(queue.dealing_result(8).is_none());
        assert!(queue.clone_result(8, &heard).is_none());
    }

    /// While the hit is dealt for the first time its object is in use, so a clone is a new one
    /// holding a copy of the hit.
    #[test]
    fn cloning_a_hit_in_use_copies_it() {
        let mut queue = Queue {
            strikes: vec![Strike {
                spell: 7,
                queue_aura: AuraRef {
                    side: Side::Player,
                    index: 0,
                },
                base_damage: 0.0,
                cleave: true,
            }],
            ..Queue::default()
        };
        let id = queue.pool.new_result_of(&hit(0, 6.0));
        queue.dealing.push(id);
        let heard = queue.dealing_result(7).unwrap();
        let (clone, token) = queue.clone_result(7, &heard).unwrap();
        assert_ne!(token, id);
        assert_eq!(clone.damage, 6.0);
        assert_eq!(queue.dealing_result(7).unwrap().damage, 6.0);
    }
}
