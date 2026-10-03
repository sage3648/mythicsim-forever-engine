//! Prepared Frostbolt-only Frost execution, not a complete Frost Mage simulator.
//! The spec owns its supported build and static decision loop. Shared spell
//! mechanics stay in Mage's spells; scheduling and mana primitives stay shared.

use std::collections::BinaryHeap;

use crate::{
    classes::mage::spells::frostbolt::{self, FrostboltRng},
    contracts::Request,
    core::{
        events::{enqueue, Event},
        time::SECOND,
    },
    mechanics::{damage::Outcome, mana},
    report::{Counts, TraceEvent, Work},
};

#[derive(Clone, Copy, Debug)]
pub(crate) enum Kind {
    ManaTick,
    Complete,
    Ready,
    Impact { damage: f64, outcome: Outcome },
}

pub(crate) struct Iteration {
    pub(crate) damage: f64,
    pub(crate) mana_end: f64,
}

pub(crate) fn validate_build(request: &Request) -> Result<(), String> {
    if request.spell.id != 25304
        || request.caster.level != 60
        || !(60..=63).contains(&request.target.level)
    {
        return Err(
            "only level 60 Frostbolt (25304) against level 60 to 63 targets is supported".into(),
        );
    }
    Ok(())
}

pub(crate) fn run_iteration(
    request: &Request,
    seed: u64,
    hit_chance: f64,
    queue: &mut BinaryHeap<Event<Kind>>,
    counts: &mut Counts,
    work: &mut Work,
    mut trace: Option<&mut Vec<TraceEvent>>,
) -> Iteration {
    let mut rng = FrostboltRng::new(seed);
    let mut mana = request.caster.max_mana;
    let mut five_second_rule = 0;
    let mut total_damage = 0.0;
    let mut sequence = 0;
    queue.clear();
    enqueue(queue, &mut sequence, 0, 0, Kind::Ready);
    enqueue(queue, &mut sequence, 2 * SECOND, 1, Kind::ManaTick);
    while let Some(event) = queue.pop() {
        if event.time > request.duration_ns {
            break;
        }
        let (name, damage) = match event.kind {
            Kind::ManaTick => {
                work.mana_ticks += 1;
                mana = mana::regenerate(&request.caster, mana, event.time, five_second_rule);
                enqueue(
                    queue,
                    &mut sequence,
                    event.time + 2 * SECOND,
                    1,
                    Kind::ManaTick,
                );
                ("mana_tick", 0.0)
            }
            Kind::Ready => {
                work.ready_checks += 1;
                if mana >= request.spell.mana_cost {
                    enqueue(
                        queue,
                        &mut sequence,
                        event.time + request.spell.cast_ns,
                        0,
                        Kind::Complete,
                    );
                    // Complete schedules Ready when the hardcast also releases the GCD.
                    if request.spell.gcd_ns > request.spell.cast_ns {
                        enqueue(
                            queue,
                            &mut sequence,
                            event.time + request.spell.gcd_ns,
                            0,
                            Kind::Ready,
                        );
                    }
                    ("cast_start", 0.0)
                } else {
                    // Only mana ticks can make this static rotation ready.
                    // Skip failed polls, but keep the Go reaction-time grid.
                    enqueue(
                        queue,
                        &mut sequence,
                        mana::next_ready_after_tick(event.time, request.reaction_ns),
                        0,
                        Kind::Ready,
                    );
                    ("wait_for_mana", 0.0)
                }
            }
            Kind::Complete => {
                work.cast_completions += 1;
                mana::spend(
                    &mut mana,
                    request.spell.mana_cost,
                    event.time,
                    &mut five_second_rule,
                );
                counts.casts += 1;
                let (damage, outcome) =
                    frostbolt::resolve_cast(request, hit_chance, &mut rng, counts, work);
                enqueue(
                    queue,
                    &mut sequence,
                    event.time + request.spell.travel_ns,
                    -1,
                    Kind::Impact { damage, outcome },
                );
                if request.spell.cast_ns >= request.spell.gcd_ns {
                    enqueue(queue, &mut sequence, event.time, 0, Kind::Ready);
                }
                ("cast_complete", 0.0)
            }
            Kind::Impact { damage, outcome } => {
                work.impacts += 1;
                total_damage += damage;
                (
                    match outcome {
                        Outcome::Hit => "hit",
                        Outcome::Crit => "crit",
                        Outcome::Miss => "miss",
                    },
                    damage,
                )
            }
        };
        if let Some(trace) = trace.as_mut() {
            trace.push(TraceEvent {
                time_ns: event.time,
                event: name.into(),
                mana,
                damage,
            });
        }
    }
    Iteration {
        damage: total_damage,
        mana_end: mana,
    }
}
