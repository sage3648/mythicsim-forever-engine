//! A fight as one member of a raid: the raid runner steps every member's fight on one clock
//! (see `engine::raid`), so each iteration runs the members to the same timeline.

use super::{Agent, Fight, FightReport, Run};

/// A member's fight, whatever its class agent.
pub(crate) trait RaidMember {
    /// What the fight builds once before its first iteration.
    fn prepare_run(&mut self);
    /// The fight length to use in place of the member's own roll, for every iteration after.
    fn set_duration_override(&mut self, duration: Option<i64>);
    /// The iteration's reset and prepull.
    fn begin_iteration(&mut self, iteration: u32);
    /// The length the current iteration runs to.
    fn duration(&self) -> i64;
    /// When the fight's next event is due.
    fn next_time(&mut self) -> i64;
    /// Run the next event. False when the iteration is over.
    fn step(&mut self) -> bool;
    /// The iteration's cleanup. Returns its damage and healing.
    fn end_iteration(&mut self, iteration: u32, run: &mut Run) -> (f64, f64);
    /// The member's report over every iteration.
    fn finish(&mut self, run: Run, elapsed_ns: u64) -> FightReport;
    /// Share the raid's target debuffs; answers, for each, whether the member casts it and
    /// whether the fight holds the raid's copy.
    fn share_auras(&mut self, debuffs: &[SharedDebuff]) -> Vec<(bool, bool)>;
    /// Keep the raid's copy of a debuff down unless a member's state brings it up.
    fn drive_external(&mut self, debuff: &SharedDebuff);
    /// The fight's clock.
    fn now(&self) -> i64;
    /// The shared auras' states that changed since the last call.
    fn drain_shared(&mut self, out: &mut Vec<SharedState>);
    /// Take on other members' shared aura states at `time`.
    fn apply_shared(&mut self, time: i64, states: &[SharedState]);
}

impl<A: Agent> RaidMember for Fight<A> {
    fn prepare_run(&mut self) {
        Fight::prepare_run(self);
    }

    fn set_duration_override(&mut self, duration: Option<i64>) {
        self.duration_override = duration;
    }

    fn begin_iteration(&mut self, iteration: u32) {
        Fight::begin_iteration(self, iteration);
    }

    fn duration(&self) -> i64 {
        self.duration
    }

    fn next_time(&mut self) -> i64 {
        let queued = self
            .queue
            .peek_time()
            .unwrap_or(crate::core::time::NEVER_EXPIRES);
        queued.min(self.autos.min_time).min(self.min_task_time)
    }

    fn step(&mut self) -> bool {
        Fight::step(self)
    }

    fn end_iteration(&mut self, iteration: u32, run: &mut Run) -> (f64, f64) {
        Fight::end_iteration(self, iteration, run);
        (self.totals.last_damage, self.totals.last_healing)
    }

    fn finish(&mut self, run: Run, elapsed_ns: u64) -> FightReport {
        self.report(run.first_duration, run.total_duration, run.logs, elapsed_ns)
    }

    fn share_auras(&mut self, debuffs: &[SharedDebuff]) -> Vec<(bool, bool)> {
        Fight::share_auras(self, debuffs)
    }

    fn drive_external(&mut self, debuff: &SharedDebuff) {
        Fight::drive_external(self, debuff);
    }

    fn now(&self) -> i64 {
        self.now
    }

    fn drain_shared(&mut self, out: &mut Vec<SharedState>) {
        Fight::drain_shared(self, out);
    }

    fn apply_shared(&mut self, time: i64, states: &[SharedState]) {
        Fight::apply_shared(self, time, states);
    }
}

/// The target auras a raid's members share, as one fight sees them: what changed since the
/// runner last looked, and the state another member's fight sends.
#[derive(Debug, Default)]
pub(crate) struct Share {
    /// The raid's key of each target aura index the raid shares.
    keys: Vec<Option<u16>>,
    /// The target aura index of each key, where this fight has the aura.
    indexes: Vec<Option<usize>>,
    /// The shared auras whose state changed, by target position and aura index.
    dirty: Vec<(u8, usize)>,
    /// Set while another member's state is applied, so it is not sent back.
    applying: bool,
}

impl Share {
    pub(crate) fn mark(&mut self, aura: super::AuraRef) {
        if self.applying {
            return;
        }
        let Some(position) = aura.side.target_position() else {
            return;
        };
        if self.keys.get(aura.index).copied().flatten().is_none() {
            return;
        }
        let entry = (position as u8, aura.index);
        if !self.dirty.contains(&entry) {
            self.dirty.push(entry);
        }
    }
}

/// A raid debuff on the target that members' fights share: the raid's copy a toggle registers,
/// and the labels of the ranks a member casts itself.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SharedDebuff {
    pub(crate) name: &'static str,
    /// The request's toggle of the raid's copy.
    pub(crate) field: &'static str,
    pub(crate) external: &'static str,
    pub(crate) own_prefix: &'static str,
    /// The spells whose cast in a member's rotation applies it.
    pub(crate) spells: &'static [i64],
}

/// A shared target aura's state, which every other member's fight takes on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SharedState {
    pub(crate) key: u16,
    pub(crate) target: u8,
    pub(crate) active: bool,
    pub(crate) stacks: i32,
    pub(crate) expires: i64,
}

impl<A: Agent> Fight<A> {
    /// Share the raid's target debuffs, in order of their keys. Answers, for each, whether this
    /// fight's member casts it itself and whether it holds the raid's copy.
    pub(crate) fn share_auras(&mut self, debuffs: &[SharedDebuff]) -> Vec<(bool, bool)> {
        let auras = &self.trackers[super::Side::Target.index()].auras;
        let mut share = Share {
            keys: vec![None; auras.len()],
            ..Share::default()
        };
        let mut found = Vec::with_capacity(debuffs.len());
        for (key, debuff) in debuffs.iter().enumerate() {
            let external = auras.iter().position(|aura| aura.label == debuff.external);
            // Every rank the member casts, of which it keeps the highest for what it is sent.
            let own: Vec<usize> = (0..auras.len())
                .filter(|&index| auras[index].label.starts_with(debuff.own_prefix))
                .collect();
            for &index in own.iter().chain(external.iter()) {
                share.keys[index] = Some(key as u16);
            }
            share.indexes.push(own.last().copied().or(external));
            found.push((!own.is_empty(), external.is_some()));
        }
        self.share = Some(Box::new(share));
        found
    }

    /// The raid's copy of a debuff a member casts: no longer up from the reset, only while a
    /// member keeps it up.
    pub(crate) fn drive_external(&mut self, debuff: &SharedDebuff) {
        let side = super::Side::Target.index();
        let Some(index) = self.trackers[side].find(debuff.external) else {
            return;
        };
        for position in 0..self.targets.len() {
            self.trackers[super::Side::target(position).index()].clear_permanent(index);
        }
    }

    /// Mark a target aura's state as changed when the raid shares it.
    #[inline]
    pub(crate) fn mark_shared(&mut self, aura: super::AuraRef) {
        if let Some(share) = self.share.as_deref_mut() {
            share.mark(aura);
        }
    }

    /// The state of every shared aura that changed since the last call.
    pub(crate) fn drain_shared(&mut self, out: &mut Vec<SharedState>) {
        let Some(share) = self.share.as_deref_mut() else {
            return;
        };
        let dirty = std::mem::take(&mut share.dirty);
        for &(target, index) in &dirty {
            let key = share.keys[index].expect("a marked aura is shared");
            let aura = &self.trackers[super::Side::target(target as usize).index()].auras[index];
            out.push(SharedState {
                key,
                target,
                active: aura.active,
                stacks: aura.stacks,
                expires: aura.expires,
            });
        }
        let share = self.share.as_deref_mut().expect("still shared");
        share.dirty = dirty;
        share.dirty.clear();
    }

    /// Take on other members' shared aura states at `time`, which no event of this fight
    /// precedes: the runner steps the earliest event of any fight first.
    pub(crate) fn apply_shared(&mut self, time: i64, states: &[SharedState]) {
        if self.share.is_none() {
            return;
        }
        if time > self.now {
            self.advance_to(time);
        }
        self.share.as_deref_mut().expect("shared").applying = true;
        for state in states {
            let share = self.share.as_deref().expect("shared");
            let Some(index) = share.indexes.get(state.key as usize).copied().flatten() else {
                continue;
            };
            let aura = super::AuraRef {
                side: super::Side::target(state.target as usize),
                index,
            };
            self.take_state(aura, state);
        }
        self.share.as_deref_mut().expect("shared").applying = false;
    }

    fn take_state(&mut self, aura: super::AuraRef, state: &SharedState) {
        let remaining = if state.expires == crate::core::time::NEVER_EXPIRES {
            crate::core::time::NEVER_EXPIRES
        } else {
            state.expires - self.now
        };
        if !state.active || remaining <= 0 {
            if self.aura(aura).active {
                self.deactivate_aura(aura);
            }
            return;
        }
        // The aura's own duration stays for this member's own casts of it.
        let own = self.aura(aura).duration;
        if !self.aura(aura).active {
            self.aura_mut(aura).duration = remaining;
            self.activate_aura(aura);
            self.aura_mut(aura).duration = own;
            // A stronger member of its exclusive category may block it.
            if !self.aura(aura).active {
                return;
            }
        } else if self.aura(aura).expires != state.expires {
            self.aura_mut(aura).duration = remaining;
            self.refresh_aura(aura);
            self.aura_mut(aura).duration = own;
        }
        if self.aura(aura).max_stacks != 0 && self.aura(aura).stacks != state.stacks {
            self.set_stacks(aura, state.stacks);
        }
    }
}
