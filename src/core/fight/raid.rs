//! A fight as one member of a raid: the raid runner steps every member's fight on one clock
//! (see `engine::raid`), so each iteration runs the members to the same timeline.

use std::{cell::RefCell, rc::Rc};

use super::{Agent, AuraRef, Fight, FightReport, Run, Side};

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
    /// Bind what the member's casts give other members and the gifts it takes.
    fn bind_gifts(&mut self, giving: &[(Gift, usize)], taking: &[Gift]) -> Result<(), String>;
    /// The gifts the member's casts sent since the last call.
    fn drain_gifts(&mut self, out: &mut Vec<SentGift>);
    /// Take another member's gift at `time`. Returns whether it landed.
    fn take_gift(&mut self, time: i64, sent: &SentGift) -> bool;
    /// Bind which of the targets swing at the member, by position among the copies, and when.
    fn bind_swings(&mut self, swings: &[Swing]) -> Result<(), String>;
    /// Report every hit that lands on the member's player to a tank's ledger.
    fn bind_tank_ledger(&mut self, ledger: SharedLedger);
    /// Land the member's single-target heals on a tank, as healer number `slot` of its ledger.
    fn bind_tank_healing(&mut self, ledger: SharedLedger, slot: usize);
    /// Take healing other members' heals landed on the member at `time`.
    fn take_heal(&mut self, time: i64, amount: f64);
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
        self.reset_gifts();
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

    fn bind_gifts(&mut self, giving: &[(Gift, usize)], taking: &[Gift]) -> Result<(), String> {
        Fight::bind_gifts(self, giving, taking)
    }

    fn drain_gifts(&mut self, out: &mut Vec<SentGift>) {
        Fight::drain_gifts(self, out);
    }

    fn take_gift(&mut self, time: i64, sent: &SentGift) -> bool {
        Fight::take_gift(self, time, sent)
    }

    fn bind_swings(&mut self, swings: &[Swing]) -> Result<(), String> {
        Fight::bind_swings(self, swings)
    }

    fn bind_tank_ledger(&mut self, ledger: SharedLedger) {
        self.tank_role.get_or_insert_with(Default::default).ledger = Some(ledger);
    }

    fn bind_tank_healing(&mut self, ledger: SharedLedger, slot: usize) {
        self.tank_role.get_or_insert_with(Default::default).healing = Some((ledger, slot));
    }

    fn take_heal(&mut self, time: i64, amount: f64) {
        Fight::take_heal(self, time, amount);
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
    /// Whether a caster's fight registers the raid's copy beside its own. Without it, a
    /// caster's fight is its Quick Sim's with the toggle off.
    pub(crate) caster_copy: bool,
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

/// A target exclusive category that holds a raid debuff a member casts: the change each member
/// makes while it holds the category, by target aura index (`Effect::RaidDebuffCategory`).
#[derive(Clone, Debug)]
pub(crate) struct RaidDebuffCategory {
    name: String,
    members: Vec<(usize, RaidDebuffChange)>,
}

/// Armor added, resistances added and school damage taken multiplied, by Go school index.
#[derive(Clone, Debug, Default)]
struct RaidDebuffChange {
    armor: f64,
    resistance: Vec<(usize, f64)>,
    damage_taken: Vec<(usize, f64)>,
}

/// Go school names, by school index.
const SCHOOLS: [&str; 8] = [
    "none", "physical", "arcane", "fire", "frost", "holy", "nature", "shadow",
];

fn by_school(
    values: &std::collections::BTreeMap<String, f64>,
) -> Result<Vec<(usize, f64)>, String> {
    values
        .iter()
        .map(|(name, value)| {
            SCHOOLS
                .iter()
                .position(|school| school == name)
                .filter(|&index| index >= 2)
                .map(|index| (index, *value))
                .ok_or_else(|| format!("a raid debuff names unknown school {name}"))
        })
        .collect()
}

impl<A: Agent> Fight<A> {
    pub(crate) fn bind_raid_debuffs(
        &mut self,
        effects: &[crate::contracts::prepared_v2::Effect],
    ) -> Result<(), String> {
        for effect in effects {
            let crate::contracts::prepared_v2::Effect::RaidDebuffCategory { category, members } =
                effect
            else {
                continue;
            };
            let mut changes = Vec::new();
            for member in members {
                let index = self.trackers[super::Side::Target.index()]
                    .find(&member.aura)
                    .ok_or_else(|| format!("target aura {} is not registered", member.aura))?;
                changes.push((
                    index,
                    RaidDebuffChange {
                        armor: member.armor,
                        resistance: by_school(&member.resistance)?,
                        damage_taken: by_school(&member.school_damage_taken_multiplier)?,
                    },
                ));
            }
            self.raid_debuffs.push(RaidDebuffCategory {
                name: category.clone(),
                members: changes,
            });
        }
        Ok(())
    }

    /// Go's `OnExpire` of the effect that gave up a raid debuff's category and `OnGain` of the
    /// one that took it. The reset's holder is in the prepared target values already.
    pub(crate) fn raid_debuff_holder_change(
        &mut self,
        tracking: bool,
        category: usize,
        old: Option<usize>,
        new: Option<usize>,
    ) {
        if self.resetting_auras {
            return;
        }
        let state = self.category(tracking, category);
        let Some(described) = self
            .raid_debuffs
            .iter()
            .position(|described| described.name == state.name)
        else {
            return;
        };
        let old = old.map(|member| state.members[member].aura);
        let new = new.map(|member| state.members[member].aura);
        for (aura, gain) in [(old, false), (new, true)] {
            let Some(aura) = aura else {
                continue;
            };
            let Some(change) = self.raid_debuffs[described]
                .members
                .iter()
                .find(|(index, _)| *index == aura.index)
                .map(|(_, change)| change.clone())
            else {
                continue;
            };
            let sign = if gain { 1.0 } else { -1.0 };
            if change.armor != 0.0 {
                self.add_target_armor(aura.side, sign * change.armor);
            }
            for &(school, delta) in &change.resistance {
                self.add_target_resistance(aura.side, school, sign * delta);
            }
            for &(school, factor) in &change.damage_taken {
                let factor = if gain { factor } else { 1.0 / factor };
                self.multiply_target_school_damage_taken(aura.side, school, factor);
            }
        }
    }
}

/// A player aura one member's cast puts on another: the raid's assignments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gift {
    Innervate,
    PowerInfusion,
}

impl Gift {
    /// The aura a member that takes the gift has for it (`prepare::env::RaidLive`).
    pub(crate) fn aura_label(self) -> &'static str {
        match self {
            Gift::Innervate => "Innervates (External)",
            Gift::PowerInfusion => "Power Infusions (External)",
        }
    }
}

/// A gift a member's cast sent: to which member, by its position among the raid's fights, and
/// when its aura ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SentGift {
    pub(crate) gift: Gift,
    pub(crate) to: usize,
    pub(crate) expires: i64,
}

/// The gifts a member gives and takes.
#[derive(Debug, Default)]
pub(crate) struct Gifts {
    /// Each gift the member's casts give another member, that member's position, and when the
    /// last one sent ends.
    giving: Vec<(Gift, usize, i64)>,
    /// The aura each gift the member takes lands on.
    taking: Vec<(Gift, AuraRef)>,
    /// What the member's casts sent since the runner last looked.
    sent: Vec<SentGift>,
}

impl<A: Agent> Fight<A> {
    /// Bind what this member's casts give other members, by their positions, and the aura each
    /// gift it takes lands on.
    pub(crate) fn bind_gifts(
        &mut self,
        giving: &[(Gift, usize)],
        taking: &[Gift],
    ) -> Result<(), String> {
        if giving.is_empty() && taking.is_empty() {
            return Ok(());
        }
        let taking = taking
            .iter()
            .map(|&gift| Ok((gift, self.player_aura(gift.aura_label())?)))
            .collect::<Result<_, String>>()?;
        self.gifts = Some(Box::new(Gifts {
            giving: giving
                .iter()
                .map(|&(gift, to)| (gift, to, i64::MIN))
                .collect(),
            taking,
            sent: Vec::new(),
        }));
        Ok(())
    }

    /// The iteration's reset: nothing the member gave is up.
    pub(crate) fn reset_gifts(&mut self) {
        if let Some(gifts) = self.gifts.as_deref_mut() {
            for entry in &mut gifts.giving {
                entry.2 = i64::MIN;
            }
            gifts.sent.clear();
        }
    }

    /// Whether the member's cast of `gift` goes to another member. When it does, the cast's aura
    /// is that member's from now for `duration`, and not this member's own.
    pub(crate) fn give(&mut self, gift: Gift, duration: i64) -> bool {
        let expires = self.now + duration;
        let Some(gifts) = self.gifts.as_deref_mut() else {
            return false;
        };
        let Some(entry) = gifts.giving.iter_mut().find(|entry| entry.0 == gift) else {
            return false;
        };
        entry.2 = expires;
        gifts.sent.push(SentGift {
            gift,
            to: entry.1,
            expires,
        });
        true
    }

    /// Whether the last `gift` this member gave is still up, when its casts go to another member.
    /// Go's cast condition reads the target's auras, which this fight knows only from its own
    /// casts.
    pub(crate) fn given_up(&self, gift: Gift) -> Option<bool> {
        let gifts = self.gifts.as_deref()?;
        gifts
            .giving
            .iter()
            .find(|entry| entry.0 == gift)
            .map(|entry| entry.2 > self.now)
    }

    /// What the member's casts sent since the last call.
    pub(crate) fn drain_gifts(&mut self, out: &mut Vec<SentGift>) {
        if let Some(gifts) = self.gifts.as_deref_mut() {
            out.append(&mut gifts.sent);
        }
    }

    /// Take another member's gift at `time`, which no event of this fight precedes. It lands
    /// unless another aura with its tag is up, as Go's cast condition reads the target's auras,
    /// for the rest of the caster's duration. Returns whether it landed.
    pub(crate) fn take_gift(&mut self, time: i64, sent: &SentGift) -> bool {
        let Some(aura) = self.gifts.as_deref().and_then(|gifts| {
            gifts
                .taking
                .iter()
                .find(|(gift, _)| *gift == sent.gift)
                .map(|&(_, aura)| aura)
        }) else {
            return false;
        };
        if time > self.now {
            self.advance_to(time);
        }
        let remaining = sent.expires - self.now;
        if remaining <= 0 {
            return false;
        }
        if let Some(tag) = self.aura(aura).tag.as_deref() {
            let blocked = self.trackers[Side::Player.index()]
                .auras
                .iter()
                .enumerate()
                .any(|(index, other)| {
                    index != aura.index && other.active && other.tag.as_deref() == Some(tag)
                });
            if blocked {
                return false;
            }
        }
        let own = self.aura(aura).duration;
        self.aura_mut(aura).duration = remaining;
        if self.aura(aura).active {
            self.refresh_aura(aura);
        } else {
            self.activate_aura(aura);
        }
        self.aura_mut(aura).duration = own;
        self.aura(aura).active
    }
}

/// Whether a target copy swings at the member's player, which the raid decides when its targets
/// have different tanks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Swing {
    /// Another member's tank holds the target: its swings never reach this fight.
    Never,
    /// The member's player holds the target for the whole fight.
    Always,
    /// The member's player and another tank swap the target on a timer: the first tank holds it
    /// from the pull for `window` nanoseconds, the second for the next `window`, and so on. The
    /// swing that comes due at a time lands on whoever holds the target then.
    Alternate { window: i64, odd: bool },
}

impl Swing {
    /// Whether the target's swing that comes due at `time` lands on the member.
    fn at(self, time: i64) -> bool {
        match self {
            Swing::Never => false,
            Swing::Always => true,
            Swing::Alternate { window, odd } => (time.div_euclid(window) & 1 == 1) == odd,
        }
    }
}

/// What the raid decided about a member as a tank and as a healer of a tank.
#[derive(Debug, Default)]
pub(crate) struct TankRole {
    /// Each target copy's swings at the member, by position; none for every copy.
    swings: Vec<Swing>,
    /// Where the member, a tank that has healers assigned, reports each hit it takes.
    pub(crate) ledger: Option<SharedLedger>,
    /// The ledger of the tank the member, a healer, is assigned to, and the member's number
    /// among that tank's healers.
    pub(crate) healing: Option<(SharedLedger, usize)>,
    /// Whether the heal being dealt lands on the assigned tank (`Fight::heal_on_tank`).
    on_tank: bool,
    /// What the last heal on the tank computed, before the tank's missing health capped it.
    gross: Option<f64>,
    /// The health metrics of the healing other members landed on this member.
    received: Option<usize>,
}

impl<A: Agent> Fight<A> {
    /// Bind which target copies swing at this member. The copies keep their swing timers, which
    /// run on the raid's clock whoever holds the target, so a copy that does not swing at the
    /// member only skips the swing that comes due: no roll, no damage, no metric.
    pub(crate) fn bind_swings(&mut self, swings: &[Swing]) -> Result<(), String> {
        if swings.len() != self.enemies.len() {
            return Err(format!(
                "{} target swings for {} targets, or a member that no target swings at",
                swings.len(),
                self.enemies.len()
            ));
        }
        self.tank_role.get_or_insert_with(Default::default).swings = swings.to_vec();
        Ok(())
    }

    /// Whether the swing of `target` that comes due now lands on the player.
    #[inline]
    pub(crate) fn enemy_swings_now(&self, target: Side) -> bool {
        let Some(role) = self.tank_role.as_deref() else {
            return true;
        };
        let position = target.target_position().expect("the unit is a target");
        role.swings
            .get(position)
            .is_none_or(|swing| swing.at(self.now))
    }

    /// A hit the player took: its damage after avoidance, armor, blocks and absorbs goes to the
    /// ledger of the healers assigned to it, if it has one.
    #[inline]
    pub(crate) fn tank_hit(&mut self, damage: f64) {
        if damage <= 0.0 {
            return;
        }
        let Some(ledger) = self
            .tank_role
            .as_deref()
            .and_then(|role| role.ledger.as_ref())
        else {
            return;
        };
        ledger.borrow_mut().hit(damage, self.player_max_health());
    }

    /// Mark the heals dealt from here on as landing on the assigned tank, or not. A member that
    /// is not an assigned healer is unaffected.
    #[inline]
    pub(crate) fn heal_on_tank(&mut self, on: bool) {
        if let Some(role) = self.tank_role.as_deref_mut() {
            role.on_tank = on && role.healing.is_some();
        }
    }

    /// A heal of `amount` on the player, as the healer's own fight computed it, when it lands on
    /// the assigned tank: what heals is the tank's missing health at most, and the rest is
    /// overhealing, in place of the share real healers overheal. Answers what heals and what
    /// overheals, or nothing when the heal does not land on a tank.
    #[inline]
    pub(crate) fn tank_heal(&mut self, amount: f64) -> Option<(f64, f64)> {
        let role = self.tank_role.as_deref_mut()?;
        role.gross = None;
        if !role.on_tank {
            return None;
        }
        let (ledger, slot) = role.healing.as_ref()?;
        let landed = ledger.borrow_mut().heal(*slot, self.now, amount);
        role.gross = Some(amount);
        Some(landed)
    }

    /// What a heal on the tank computed, before its overhealing came off.
    pub(crate) fn tank_heal_gross(&self) -> Option<f64> {
        self.tank_role.as_deref().and_then(|role| role.gross)
    }

    /// Healing other members landed on this member's player at `time`, which no event of this
    /// fight precedes. The player's health gains it as it gains any heal, so its death tracking
    /// reads it.
    pub(crate) fn take_heal(&mut self, time: i64, amount: f64) {
        if time > self.now {
            self.advance_to(time);
        }
        let metrics = match self.tank_role.as_deref().and_then(|role| role.received) {
            Some(metrics) => metrics,
            None => {
                let metrics = self.new_health_metrics(crate::contracts::prepared_v2::ActionId {
                    other_id: "OtherActionHealingModel".into(),
                    ..Default::default()
                });
                self.tank_role.get_or_insert_with(Default::default).received = Some(metrics);
                metrics
            }
        };
        self.gain_health(amount, metrics);
    }
}

/// A tank's missing health, which its fight and the healers assigned to it share: the runner
/// creates one per tank with healers and hands it to their fights.
#[derive(Debug, Default)]
pub(crate) struct TankLedger {
    /// Maximum health less current health, from the iteration's hits and the healing that landed.
    missing: f64,
    /// The iteration's damage taken.
    damage: f64,
    /// Each assigned healer's healing that landed and healing that overhealed this iteration.
    healers: Vec<(f64, f64)>,
    /// Healing that landed and the tank's fight has not taken yet, with its time.
    pending: Vec<(i64, f64)>,
}

pub(crate) type SharedLedger = Rc<RefCell<TankLedger>>;

impl TankLedger {
    pub(crate) fn new(healers: usize) -> SharedLedger {
        Rc::new(RefCell::new(TankLedger {
            healers: vec![(0.0, 0.0); healers],
            ..TankLedger::default()
        }))
    }

    /// An iteration starts: the tank is at full health and nothing has landed.
    pub(crate) fn reset(&mut self) {
        self.missing = 0.0;
        self.damage = 0.0;
        self.healers
            .iter_mut()
            .for_each(|entry| *entry = (0.0, 0.0));
        self.pending.clear();
    }

    /// The tank took `damage`; its health cannot be missing more than `max_health`.
    fn hit(&mut self, damage: f64, max_health: f64) {
        self.damage += damage;
        self.missing = (self.missing + damage).min(max_health);
    }

    /// Healer `slot`'s heal of `amount` at `time`: the missing health it fills lands, the rest
    /// overheals. What landed leaves the missing health at once, so the next heal at this moment,
    /// from any healer, sees what is left.
    fn heal(&mut self, slot: usize, time: i64, amount: f64) -> (f64, f64) {
        let landed = amount.min(self.missing).max(0.0);
        let overhealing = amount.max(0.0) - landed;
        self.missing -= landed;
        let entry = &mut self.healers[slot];
        entry.0 += landed;
        entry.1 += overhealing;
        if landed > 0.0 {
            self.pending.push((time, landed));
        }
        (landed, overhealing)
    }

    /// Move the healing the tank's fight has not taken to `out`.
    pub(crate) fn take_pending(&mut self, out: &mut Vec<(i64, f64)>) {
        out.append(&mut self.pending);
    }

    /// The iteration's damage taken and each healer's healing that landed and overhealed.
    pub(crate) fn iteration(&self) -> (f64, &[(f64, f64)]) {
        (self.damage, &self.healers)
    }
}
