//! Go `auraTracker` and `Aura` lifecycle.
//!
//! Callback lists are Go slices maintained by swap removal. Go iterates them with a
//! range loop over the slice header taken before the loop, so a removal or activation
//! during a callback changes which entries the loop still visits. [`CallbackList`]
//! keeps the backing array and logical length separately to reproduce that exactly.

use crate::{
    contracts::prepared_v2::{ActionId, Aura as ExportedAura},
    core::time::NEVER_EXPIRES,
};

use super::{
    log::action_string, metrics::Aggregator, Agent, DotId, Fight, Side, SpellId, SpellResult,
    TimerId,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct AuraRef {
    pub(crate) side: Side,
    pub(crate) index: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum AuraBehavior<K> {
    /// Stat or regeneration effects that the prepared values already include.
    Static,
    /// A listener that never acts in the supported scope.
    Inert,
    /// Go health.go `trackChanceOfDeath`'s listener on hits the player takes.
    ChanceOfDeath,
    /// Go buffs/paladin.go `AttachJudgementOfWisdomMana`.
    JudgementOfWisdom {
        chance: f64,
        mana: f64,
        metrics: usize,
        delay: i64,
    },
    /// Go racials.go `applyTouchOfTheGrave`: a proc trigger that casts the drain.
    TouchOfTheGrave {
        chance: f64,
        delay: i64,
        drain: SpellId,
    },
    /// Go racials.go `applyEureka`'s aura.
    Eureka,
    /// Go `Aura.AttachMultiplyCastSpeed`.
    MultiplyCastSpeed(f64),
    /// Go `MultiplyManaRegenSpeed` on gain and its reciprocal on expire, as racials.go
    /// Energized does with 2 and 0.5.
    MultiplyManaRegenSpeed(f64),
    /// Go `NewTemporaryStatMultiplierAura`: the stats while active. Go recomputes every stat
    /// from the same inputs on each change, so expiry restores the prepared values exactly.
    TemporaryStats {
        /// The aura's bit in `Fight::stat_mask`.
        bit: u32,
        /// Lines in `Fight::aura_logs` logged before the stats change on gain and expiry.
        gain_log: Option<usize>,
        expire_log: Option<usize>,
    },
    /// The Crusader enchant's trigger.
    Crusader,
    /// The party Windfury Totem's totem aura, whose exclusive effect holds the trigger.
    WindfuryTotem,
    /// The party Windfury Totem's trigger.
    WindfuryTrigger,
    /// The party Windfury Totem's charges of attack power, which landed autos spend.
    WindfuryProc {
        bit: u32,
    },
    /// Dragonbreath Chili's trigger.
    DragonbreathChili,
    /// The aura of a dot or channel.
    Dot(DotId),
    Class(K),
}

/// Go callback slice, by list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum List {
    Active = 0,
    ApplyEffects,
    CastComplete,
    SpellHitDealt,
    SpellHitTaken,
    PeriodicDamageDealt,
    PeriodicDamageTaken,
    EncounterStart,
}

const LISTS: usize = 8;

#[derive(Clone, Debug, Default)]
pub(crate) struct CallbackList {
    backing: Vec<usize>,
    len: usize,
}

impl CallbackList {
    fn clear(&mut self) {
        self.len = 0;
    }

    fn push(&mut self, aura: usize) -> usize {
        if self.len < self.backing.len() {
            self.backing[self.len] = aura;
        } else {
            self.backing.push(aura);
        }
        self.len += 1;
        self.len - 1
    }

    /// Go `removeBySwappingToBack`. Returns the aura moved into `index`, if any.
    fn swap_remove(&mut self, index: usize) -> Option<usize> {
        self.backing[index] = self.backing[self.len - 1];
        self.len -= 1;
        (index < self.len).then(|| self.backing[index])
    }

    /// The slice header a Go range loop would capture now.
    pub(crate) fn snapshot_len(&self) -> usize {
        self.len
    }

    /// What a Go range loop reads at `index`, including stale entries past the length.
    pub(crate) fn read(&self, index: usize) -> usize {
        self.backing[index]
    }

    pub(crate) fn live(&self) -> &[usize] {
        &self.backing[..self.len]
    }
}

pub(crate) struct Aura<K> {
    pub(crate) label: String,
    pub(crate) action_id: Option<ActionId>,
    pub(crate) duration: i64,
    pub(crate) max_stacks: i32,
    pub(crate) behavior: AuraBehavior<K>,
    lists: [bool; LISTS],
    permanent: bool,
    /// The later member of its exclusive category that displaces it during the reset.
    displaced_by: Option<String>,
    /// Blocked at the reset by an earlier member of its exclusive category.
    blocked_at_reset: bool,
    pub(crate) icd: Option<(TimerId, i64)>,
    pub(crate) active: bool,
    pub(crate) stacks: i32,
    pub(crate) start: i64,
    pub(crate) expires: i64,
    pub(crate) fade_time: i64,
    positions: [Option<usize>; LISTS],
    pub(crate) uptime: i64,
    pub(crate) procs: i32,
    pub(crate) aggregate: Aggregator,
    pub(crate) procs_sum: i64,
}

impl<K> Aura<K> {
    pub(crate) fn remaining(&self, now: i64) -> i64 {
        if !self.active {
            0
        } else if self.expires == NEVER_EXPIRES {
            NEVER_EXPIRES
        } else {
            self.expires - now
        }
    }
}

pub(crate) struct Tracker<K> {
    pub(crate) auras: Vec<Aura<K>>,
    pub(crate) lists: [CallbackList; LISTS],
    pub(crate) min_expires: i64,
}

impl<K> Default for Tracker<K> {
    fn default() -> Self {
        Self {
            auras: Vec::new(),
            lists: Default::default(),
            min_expires: NEVER_EXPIRES,
        }
    }
}

impl<K> Tracker<K> {
    pub(crate) fn register(
        &mut self,
        exported: &ExportedAura,
        behavior: AuraBehavior<K>,
        icd: Option<(TimerId, i64)>,
    ) {
        let mut lists = [false; LISTS];
        for callback in &exported.callbacks {
            let list = match callback.as_str() {
                "on_apply_effects" => List::ApplyEffects,
                "on_cast_complete" => List::CastComplete,
                "on_spell_hit_dealt" => List::SpellHitDealt,
                "on_spell_hit_taken" => List::SpellHitTaken,
                "on_periodic_damage_dealt" => List::PeriodicDamageDealt,
                "on_periodic_damage_taken" => List::PeriodicDamageTaken,
                "on_encounter_start" => List::EncounterStart,
                _ => continue,
            };
            lists[list as usize] = true;
        }
        self.auras.push(Aura {
            label: exported.label.clone(),
            action_id: exported.action_id.clone(),
            duration: exported.duration_ns,
            max_stacks: exported.max_stacks,
            behavior,
            lists,
            // An aura active right after Go's reset was activated by its OnReset, as was one
            // a later member of its exclusive category displaced.
            permanent: exported.active || exported.displaced_by.is_some(),
            displaced_by: exported.displaced_by.clone(),
            blocked_at_reset: exported.blocked_at_reset,
            icd,
            active: false,
            stacks: 0,
            start: 0,
            expires: 0,
            fade_time: -NEVER_EXPIRES,
            positions: [None; LISTS],
            uptime: 0,
            procs: 0,
            aggregate: Aggregator::default(),
            procs_sum: 0,
        });
    }

    pub(crate) fn find(&self, label: &str) -> Option<usize> {
        self.auras.iter().position(|aura| aura.label == label)
    }

    /// Go `GetAuraByID`: the first aura with the same action, tag included.
    pub(crate) fn find_by_id(&self, id: &ActionId) -> Option<usize> {
        self.auras
            .iter()
            .position(|aura| aura.action_id.as_ref() == Some(id))
    }

    fn add_to(&mut self, list: List, aura: usize) {
        let position = self.lists[list as usize].push(aura);
        self.auras[aura].positions[list as usize] = Some(position);
    }

    fn remove_from(&mut self, list: List, aura: usize) {
        if let Some(position) = self.auras[aura].positions[list as usize].take() {
            if let Some(moved) = self.lists[list as usize].swap_remove(position) {
                self.auras[moved].positions[list as usize] = Some(position);
            }
        }
    }
}

impl<A: Agent> Fight<A> {
    pub(crate) fn aura(&self, aura: AuraRef) -> &Aura<A::Aura> {
        &self.trackers[aura.side.index()].auras[aura.index]
    }

    /// A registered player aura by label.
    pub(crate) fn player_aura(&self, label: &str) -> Result<AuraRef, String> {
        self.trackers[Side::Player.index()]
            .find(label)
            .map(|index| AuraRef {
                side: Side::Player,
                index,
            })
            .ok_or_else(|| format!("player aura {label} is not registered"))
    }

    pub(crate) fn aura_mut(&mut self, aura: AuraRef) -> &mut Aura<A::Aura> {
        &mut self.trackers[aura.side.index()].auras[aura.index]
    }

    fn unit_label(&self, side: Side) -> String {
        match side {
            Side::Player => self.config.player_label.clone(),
            Side::Target => self.config.target_label.clone(),
        }
    }

    /// Go `Aura.Refresh`.
    pub(crate) fn refresh_aura(&mut self, aura: AuraRef) {
        let now = self.now;
        let tracker = &mut self.trackers[aura.side.index()];
        let state = &mut tracker.auras[aura.index];
        if state.duration == NEVER_EXPIRES {
            state.expires = NEVER_EXPIRES;
        } else {
            state.expires = now + state.duration;
            if state.expires < tracker.min_expires {
                tracker.min_expires = state.expires;
                let expires = state.expires;
                self.reschedule_tracker(expires);
            }
        }
    }

    /// Go `Aura.Activate`.
    pub(crate) fn activate_aura(&mut self, aura: AuraRef) {
        self.aura_mut(aura).procs += 1;
        if self.aura(aura).active {
            if let Some(id) = self
                .aura(aura)
                .action_id
                .clone()
                .filter(|_| self.log.is_some())
            {
                let line = format!("Aura refreshed: {}", action_string(&id));
                self.unit_log(aura.side, &line);
            }
            self.refresh_aura(aura);
            return;
        }
        assert!(self.aura(aura).duration != 0, "aura with zero duration");
        {
            let now = self.now;
            let state = self.aura_mut(aura);
            state.active = true;
            state.start = now;
        }
        self.refresh_aura(aura);
        let tracker = &mut self.trackers[aura.side.index()];
        if tracker.auras[aura.index].duration != NEVER_EXPIRES {
            tracker.add_to(List::Active, aura.index);
        }
        for list in [
            List::ApplyEffects,
            List::CastComplete,
            List::SpellHitDealt,
            List::SpellHitTaken,
            List::PeriodicDamageDealt,
            List::PeriodicDamageTaken,
            List::EncounterStart,
        ] {
            if tracker.auras[aura.index].lists[list as usize] {
                tracker.add_to(list, aura.index);
            }
        }
        if let Some(id) = self
            .aura(aura)
            .action_id
            .clone()
            .filter(|_| self.log.is_some())
        {
            let line = format!("Aura gained: {}", action_string(&id));
            self.unit_log(aura.side, &line);
        }
        self.on_gain(aura);
    }

    /// Go `Aura.Deactivate`.
    pub(crate) fn deactivate_aura(&mut self, aura: AuraRef) {
        if !self.aura(aura).active {
            return;
        }
        let now = self.now;
        let has_id = {
            let state = self.aura_mut(aura);
            state.active = false;
            state.action_id.is_some()
        };
        if has_id {
            let state = self.aura_mut(aura);
            let start = state.start.max(0);
            if now > state.expires {
                state.uptime += (state.expires - start).max(0);
            } else {
                state.uptime += (now - start).max(0);
            }
            if self.log.is_some() {
                let expires = self.aura(aura).expires;
                let id = self.aura(aura).action_id.clone().unwrap_or_default();
                let line = format!("Aura faded: {}", action_string(&id));
                let label = self.unit_label(aura.side);
                self.log_at(now.min(expires), &label, &line);
            }
        }
        {
            let state = self.aura_mut(aura);
            state.expires = 0;
            state.fade_time = now;
        }
        let tracker = &mut self.trackers[aura.side.index()];
        for list in [
            List::Active,
            List::ApplyEffects,
            List::CastComplete,
            List::SpellHitDealt,
            List::SpellHitTaken,
            List::PeriodicDamageDealt,
            List::PeriodicDamageTaken,
            List::EncounterStart,
        ] {
            tracker.remove_from(list, aura.index);
        }
        if self.aura(aura).stacks != 0 {
            self.set_stacks(aura, 0);
        }
        self.on_expire(aura);
    }

    /// Go `Aura.SetStacks`.
    pub(crate) fn set_stacks(&mut self, aura: AuraRef, stacks: i32) {
        let state = self.aura(aura);
        assert!(state.active || stacks == 0, "stacks on an inactive aura");
        assert!(state.max_stacks != 0, "stacks on an aura without MaxStacks");
        let old = state.stacks;
        let new = stacks.min(state.max_stacks).max(0);
        if old == new {
            return;
        }
        if let Some(id) = state.action_id.clone().filter(|_| self.log.is_some()) {
            let line = format!("{} stacks: {old} --> {new}", action_string(&id));
            self.unit_log(aura.side, &line);
        }
        self.aura_mut(aura).stacks = new;
        if let AuraBehavior::Class(kind) = self.aura(aura).behavior {
            A::on_stacks_change(self, aura, kind, old, new);
        }
        if self.aura(aura).stacks == 0 {
            self.deactivate_aura(aura);
        }
    }

    pub(crate) fn add_stack(&mut self, aura: AuraRef) {
        let stacks = self.aura(aura).stacks + 1;
        self.set_stacks(aura, stacks);
    }

    pub(crate) fn remove_stack(&mut self, aura: AuraRef) {
        let stacks = (self.aura(aura).stacks - 1).max(0);
        self.set_stacks(aura, stacks);
    }

    fn on_gain(&mut self, aura: AuraRef) {
        match self.aura(aura).behavior {
            AuraBehavior::Dot(dot) => self.dot_on_gain(dot),
            AuraBehavior::Eureka => self.eureka_gain(),
            AuraBehavior::MultiplyCastSpeed(multiplier) => self.multiply_cast_speed(multiplier),
            AuraBehavior::MultiplyManaRegenSpeed(multiplier) => {
                self.multiply_mana_regen_speed(multiplier)
            }
            AuraBehavior::WindfuryProc { bit } => {
                self.stat_mask |= bit;
                self.player.powers = self.stat_combos[self.stat_mask as usize];
            }
            AuraBehavior::WindfuryTotem => {
                let trigger = self
                    .windfury
                    .as_ref()
                    .expect("Windfury Totem is bound")
                    .trigger;
                if !self.aura(trigger).active {
                    self.activate_aura(trigger);
                }
            }
            AuraBehavior::TemporaryStats { bit, gain_log, .. } => {
                if let (Some(line), true) = (gain_log, self.log.is_some()) {
                    let line = self.aura_logs[line].clone();
                    self.player_log(&line);
                }
                self.stat_mask |= bit;
                self.player.powers = self.stat_combos[self.stat_mask as usize];
            }
            AuraBehavior::Class(kind) => A::on_gain(self, aura, kind),
            _ => {}
        }
    }

    fn on_expire(&mut self, aura: AuraRef) {
        match self.aura(aura).behavior {
            AuraBehavior::Dot(dot) => self.dot_on_expire(dot),
            AuraBehavior::Eureka => self.eureka_expire(),
            AuraBehavior::MultiplyCastSpeed(multiplier) => {
                self.multiply_cast_speed(1.0 / multiplier)
            }
            AuraBehavior::WindfuryProc { bit } => {
                self.stat_mask &= !bit;
                self.player.powers = self.stat_combos[self.stat_mask as usize];
            }
            AuraBehavior::WindfuryTotem => {
                let trigger = self
                    .windfury
                    .as_ref()
                    .expect("Windfury Totem is bound")
                    .trigger;
                self.deactivate_aura(trigger);
            }
            AuraBehavior::TemporaryStats {
                bit, expire_log, ..
            } => {
                if let (Some(line), true) = (expire_log, self.log.is_some()) {
                    let line = self.aura_logs[line].clone();
                    self.player_log(&line);
                }
                self.stat_mask &= !bit;
                self.player.powers = self.stat_combos[self.stat_mask as usize];
            }
            AuraBehavior::MultiplyManaRegenSpeed(multiplier) => {
                self.multiply_mana_regen_speed(1.0 / multiplier)
            }
            AuraBehavior::Class(kind) => A::on_expire(self, aura, kind),
            _ => {}
        }
    }

    /// Go `auraTracker.reset`: reset effects ran before this; then each aura resets in
    /// registration order and permanent auras activate.
    pub(crate) fn reset_auras(&mut self, side: Side) {
        let tracker = &mut self.trackers[side.index()];
        for list in &mut tracker.lists {
            list.clear();
        }
        tracker.min_expires = NEVER_EXPIRES;
        for index in 0..self.trackers[side.index()].auras.len() {
            let state = &mut self.trackers[side.index()].auras[index];
            assert!(
                !state.active && state.stacks == 0,
                "aura active during reset"
            );
            state.uptime = 0;
            state.procs = 0;
            state.fade_time = -NEVER_EXPIRES;
            if state.permanent {
                state.duration = NEVER_EXPIRES;
                // Go ExclusiveEffect.Activate: a stronger later member of the category
                // deactivates the earlier one before it activates.
                let label = state.label.clone();
                for other in 0..index {
                    let displaced = &self.trackers[side.index()].auras[other];
                    if displaced.active && displaced.displaced_by.as_deref() == Some(&label) {
                        self.deactivate_aura(AuraRef { side, index: other });
                    }
                }
                self.activate_aura(AuraRef { side, index });
            } else if state.blocked_at_reset {
                // Go Aura.Activate counts the proc before the exclusive effect blocks it.
                state.procs += 1;
            }
        }
    }

    /// Go `auraTracker.tryAdvance` and `advance`.
    pub(crate) fn try_advance_tracker(&mut self, side: Side) -> i64 {
        if self.now < self.trackers[side.index()].min_expires {
            return self.trackers[side.index()].min_expires;
        }
        loop {
            let tracker = &self.trackers[side.index()];
            let mut min = NEVER_EXPIRES;
            let mut expired = None;
            for &index in tracker.lists[List::Active as usize].live() {
                let expires = tracker.auras[index].expires;
                if expires <= self.now {
                    expired = Some(index);
                    break;
                }
                min = min.min(expires);
            }
            // Deactivation edits the list, so scan again from the start, as Go does.
            match expired {
                Some(index) => self.deactivate_aura(AuraRef { side, index }),
                None => {
                    self.trackers[side.index()].min_expires = min;
                    return min;
                }
            }
        }
    }

    /// Go `auraTracker.doneIteration`: deactivate every aura in registration order, then
    /// fold this iteration's uptime and procs into the aggregates.
    pub(crate) fn aura_done_iteration(&mut self, side: Side) {
        'restart: loop {
            for index in 0..self.trackers[side.index()].auras.len() {
                if self.trackers[side.index()].auras[index].active {
                    self.deactivate_aura(AuraRef { side, index });
                    continue 'restart;
                }
            }
            break;
        }
        for state in &mut self.trackers[side.index()].auras {
            state.start = 0;
            state.expires = 0;
            state
                .aggregate
                .add(crate::core::time::seconds(state.uptime));
            state.procs_sum += i64::from(state.procs);
        }
    }

    /// Go `auraTracker.OnCastComplete`. No active check, as in Go.
    pub(crate) fn on_cast_complete(&mut self, spell: SpellId) {
        let list = List::CastComplete as usize;
        let length = self.trackers[Side::Player.index()].lists[list].snapshot_len();
        for position in 0..length {
            let index = self.trackers[Side::Player.index()].lists[list].read(position);
            let aura = AuraRef {
                side: Side::Player,
                index,
            };
            match self.aura(aura).behavior {
                AuraBehavior::Class(kind) => A::on_cast_complete(self, aura, kind, spell),
                AuraBehavior::Eureka => self.eureka_cast_complete(spell),
                _ => {}
            }
        }
    }

    /// Go `auraTracker.OnPeriodicDamageDealt` on the caster, which skips no inactive aura. No
    /// target aura in scope acts on periodic damage taken.
    pub(crate) fn on_periodic_damage(&mut self, spell: SpellId, result: &SpellResult) {
        let list = List::PeriodicDamageDealt as usize;
        let length = self.trackers[Side::Player.index()].lists[list].snapshot_len();
        for position in 0..length {
            let index = self.trackers[Side::Player.index()].lists[list].read(position);
            let aura = AuraRef {
                side: Side::Player,
                index,
            };
            if let AuraBehavior::Class(kind) = self.aura(aura).behavior {
                A::on_periodic_damage_dealt(self, aura, kind, spell, result);
            }
        }
    }

    /// Go `auraTracker.OnSpellHitDealt` on the caster and `OnSpellHitTaken` on the target.
    pub(crate) fn on_spell_hit(&mut self, spell: SpellId, result: &SpellResult) {
        for (side, list) in [
            (Side::Player, List::SpellHitDealt),
            (result.target, List::SpellHitTaken),
        ] {
            // Listeners of the caster's hits, as opposed to the hits its target takes.
            let dealt = list == List::SpellHitDealt;
            let list = list as usize;
            let length = self.trackers[side.index()].lists[list].snapshot_len();
            for position in 0..length {
                let index = self.trackers[side.index()].lists[list].read(position);
                let aura = AuraRef { side, index };
                if !self.aura(aura).active {
                    continue;
                }
                match self.aura(aura).behavior.clone() {
                    AuraBehavior::Class(kind) if dealt => {
                        A::on_spell_hit_dealt(self, aura, kind, spell, result)
                    }
                    AuraBehavior::JudgementOfWisdom { chance, delay, .. } => {
                        self.judgement_of_wisdom_callback(aura, spell, result, chance, delay)
                    }
                    AuraBehavior::TouchOfTheGrave { chance, delay, .. } if dealt => {
                        self.touch_of_the_grave_callback(aura, spell, result, chance, delay)
                    }
                    AuraBehavior::WindfuryTrigger if dealt => {
                        self.windfury_trigger(aura, spell, result)
                    }
                    AuraBehavior::WindfuryProc { .. } if dealt => {
                        let windfury = self.windfury.as_ref().expect("Windfury Totem is bound");
                        // The charges' own trigger: a landed auto spends one, at once.
                        if windfury.spend_spells[spell]
                            && result.outcome & super::OUTCOME_LANDED != 0
                        {
                            self.remove_stack(aura);
                        }
                    }
                    AuraBehavior::Crusader if dealt => self.crusader_callback(aura, spell, result),
                    AuraBehavior::DragonbreathChili if dealt => {
                        self.chili_callback(aura, spell, result)
                    }
                    AuraBehavior::ChanceOfDeath if !dealt && side == Side::Player => {
                        self.chance_of_death_hit_taken(result)
                    }
                    _ => {}
                }
            }
        }
    }

    /// Go `AttachProcTriggerCallback` with `ProcMaskDirect` and no outcome filter, delayed by
    /// the spell batch window.
    fn judgement_of_wisdom_callback(
        &mut self,
        aura: AuraRef,
        spell: SpellId,
        result: &SpellResult,
        chance: f64,
        delay: i64,
    ) {
        let spell_state = &self.spells[spell];
        if spell_state.flags.proc || !spell_state.direct_proc {
            return;
        }
        if chance != 1.0 && self.random_for_aura(aura) > chance {
            return;
        }
        let result = *result;
        self.schedule(
            self.now + delay,
            super::PRIORITY_DOT,
            super::Action::DelayedProc {
                aura,
                spell,
                result,
            },
        );
    }

    /// Go `AttachProcTriggerCallback`'s delayed handler: run it a spell batch window from now.
    pub(crate) fn schedule_delayed_proc(
        &mut self,
        aura: AuraRef,
        spell: SpellId,
        result: SpellResult,
    ) {
        self.schedule(
            self.now + super::SPELL_BATCH_WINDOW,
            super::PRIORITY_DOT,
            super::Action::DelayedProc {
                aura,
                spell,
                result,
            },
        );
    }

    /// Go `AttachProcTriggerCallback` for the Windfury Totem trigger: landed hits, the
    /// cooldown, the chance roll; then charges, one fewer when an auto granted them, and an
    /// extra main hand attack at once.
    fn windfury_trigger(&mut self, aura: AuraRef, spell: SpellId, result: &SpellResult) {
        let windfury = self.windfury.clone().expect("Windfury Totem is bound");
        if !windfury.trigger_spells[spell] || result.outcome & super::OUTCOME_LANDED == 0 {
            return;
        }
        let icd = self.aura(aura).icd;
        if let Some((timer, _)) = icd {
            if self.timers[timer] > self.now {
                return;
            }
        }
        if windfury.trigger_chance != 1.0 && self.random_for_aura(aura) > windfury.trigger_chance {
            return;
        }
        if let Some((timer, duration)) = icd {
            self.timers[timer] = self.now + duration;
        }
        self.activate_aura(windfury.proc_aura);
        let mut charges = self.aura(windfury.proc_aura).max_stacks;
        if self.spells[spell].white_hit {
            charges -= 1;
        }
        self.set_stacks(windfury.proc_aura, charges);
        self.cast(windfury.extra, result.target);
    }

    /// Go `AttachProcTriggerCallback` for Crusader: a weapon proc on landed hits that rolls the
    /// hand's chance, then waits a spell batch window.
    fn crusader_callback(&mut self, aura: AuraRef, spell: SpellId, result: &SpellResult) {
        if result.outcome & super::OUTCOME_LANDED == 0 {
            return;
        }
        let Some(chance) = self
            .crusader
            .as_ref()
            .and_then(|crusader| crusader.chances[spell])
        else {
            return;
        };
        if !self.proc(
            chance,
            &self.trackers[aura.side.index()].auras[aura.index]
                .label
                .clone(),
        ) {
            return;
        }
        self.schedule_delayed_proc(aura, spell, *result);
    }

    /// Go `AttachProcTriggerCallback` for Dragonbreath Chili: landed melee hits, its cooldown,
    /// then a chance roll, and the cast a spell batch window later.
    fn chili_callback(&mut self, aura: AuraRef, spell: SpellId, result: &SpellResult) {
        let Some(chili) = self.chili.as_ref() else {
            return;
        };
        if !chili.spells[spell] || result.outcome & super::OUTCOME_LANDED == 0 {
            return;
        }
        let (chance, delay) = (chili.proc_chance, chili.delay);
        let icd = self.aura(aura).icd;
        if let Some((timer, _)) = icd {
            if self.timers[timer] > self.now {
                return;
            }
        }
        if chance != 1.0 && self.random_for_aura(aura) > chance {
            return;
        }
        if let Some((timer, duration)) = icd {
            self.timers[timer] = self.now + duration;
        }
        let result = *result;
        self.schedule(
            self.now + delay,
            super::PRIORITY_DOT,
            super::Action::DelayedProc {
                aura,
                spell,
                result,
            },
        );
    }

    /// The delayed half of a proc trigger.
    pub(crate) fn delayed_proc(&mut self, aura: AuraRef, spell: SpellId, result: SpellResult) {
        match self.aura(aura).behavior.clone() {
            AuraBehavior::JudgementOfWisdom { mana, metrics, .. } => {
                // Go: melee and ranged hits always pay; spells must land.
                if !self.spells[spell].melee_or_ranged_proc
                    && result.outcome & super::OUTCOME_LANDED == 0
                {
                    return;
                }
                self.add_mana(mana, metrics);
            }
            AuraBehavior::TouchOfTheGrave { drain, .. } => {
                self.cast(drain, result.target);
            }
            AuraBehavior::Crusader => {
                let crusader = self.crusader.clone().expect("Crusader is bound");
                // Go Ternary(spell.IsOH(), ohAura, mhAura).
                let hand = if self.spells[spell].off_hand_proc {
                    crusader.oh_aura
                } else {
                    crusader.mh_aura
                };
                self.activate_aura(hand);
                let heal = crusader.heal_min
                    + (crusader.heal_max - crusader.heal_min) * self.random("Damage Roll");
                self.gain_health(heal, crusader.heal_metrics);
            }
            AuraBehavior::DragonbreathChili => {
                let chili = self.chili.clone().expect("Dragonbreath Chili is bound");
                self.cast(chili.spell, result.target);
            }
            AuraBehavior::Class(kind) => A::on_delayed_proc(self, aura, kind, spell, result),
            _ => {}
        }
    }
}
