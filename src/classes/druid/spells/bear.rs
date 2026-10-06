//! The bear's spells, from Go sim/druid/forms.go, enrage.go, demoralizing_roar.go, maul.go,
//! lacerate.go, swipe.go and primal_bite.go.
//!
//! Bear Form itself is in `bear_form`.

use crate::core::{
    fight::{
        melee::PhysicalOutcome, AuraRef, DotId, Fight, Outcome, Side, SpellId, TimerId,
        PRIORITY_GCD, PRIORITY_REGEN,
    },
    time::STARTING_CD_TIME,
};

use super::super::agent::DruidAgent;

/// Class periodic action tags.
pub(crate) const ENRAGE_TAG: u32 = 1;
pub(crate) const MAUL_QUEUE_TAG: u32 = 2;
pub(crate) const FRENZIED_REGENERATION_TAG: u32 = 3;

/// Barkskin (22812): its aura cuts physical damage taken through the stat auras, and a cast in
/// the fight restarts the main hand swing.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Barkskin {
    pub(crate) aura: AuraRef,
    pub(crate) stat_bit: u32,
}

impl Barkskin {
    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>) {
        fight.activate_aura(self.aura);
        if fight.now > 0 {
            let now = fight.now;
            fight.stop_melee_until(now);
        }
    }

    pub(crate) fn on_gain(&self, fight: &mut Fight<DruidAgent>) {
        fight.set_stat_aura(self.stat_bit, true);
    }

    pub(crate) fn on_expire(&self, fight: &mut Fight<DruidAgent>) {
        fight.set_stat_aura(self.stat_bit, false);
    }
}

/// Frenzied Regeneration (22842): each second while its aura is up, up to 10 Rage becomes 1%
/// of maximum health a point.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FrenziedRegeneration {
    pub(crate) aura: AuraRef,
    pub(crate) ticks: i32,
    pub(crate) period: i64,
    pub(crate) max_rage_per_tick: f64,
    pub(crate) health_share_per_rage: f64,
    pub(crate) healing_taken_multiplier: f64,
    pub(crate) rage_metrics: usize,
    pub(crate) health_metrics: usize,
}

impl FrenziedRegeneration {
    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>) {
        fight.activate_aura(self.aura);
        fight.start_class_periodic(
            FRENZIED_REGENERATION_TAG,
            self.period,
            self.ticks,
            crate::core::fight::PRIORITY_DOT,
        );
    }

    pub(crate) fn tick(&self, fight: &mut Fight<DruidAgent>) {
        if !fight.aura(self.aura).active {
            return;
        }
        let rage = fight.current_rage().min(self.max_rage_per_tick);
        if rage > 0.0 {
            fight.spend_rage(rage, self.rage_metrics);
            let health = rage
                * self.health_share_per_rage
                * fight.player_max_health()
                * self.healing_taken_multiplier;
            fight.gain_health(health, self.health_metrics);
        }
    }
}

/// Enrage (5229): instant Rage, then Rage each second while its aura is up; the aura cuts
/// armor through the stat auras.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Enrage {
    pub(crate) aura: AuraRef,
    pub(crate) stat_bit: u32,
    pub(crate) instant_rage: f64,
    pub(crate) rage_per_tick: f64,
    pub(crate) ticks: i32,
    pub(crate) period: i64,
    pub(crate) metrics: usize,
}

impl Enrage {
    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>) {
        fight.add_rage(self.instant_rage, self.metrics);
        fight.activate_aura(self.aura);
        fight.start_class_periodic(ENRAGE_TAG, self.period, self.ticks, PRIORITY_REGEN);
    }

    pub(crate) fn tick(&self, fight: &mut Fight<DruidAgent>) {
        if fight.aura(self.aura).active {
            fight.add_rage(self.rage_per_tick, self.metrics);
        }
    }

    pub(crate) fn on_gain(&self, fight: &mut Fight<DruidAgent>) {
        fight.set_stat_aura(self.stat_bit, true);
    }

    pub(crate) fn on_expire(&self, fight: &mut Fight<DruidAgent>) {
        fight.set_stat_aura(self.stat_bit, false);
    }
}

/// Demoralizing Roar (9898): a magic hit roll on every target in unit index order, each landed
/// one activating that target's debuff, whose attack power cut the target's swing reads while
/// it is up.
pub(crate) fn demoralizing_roar(fight: &mut Fight<DruidAgent>, spell: SpellId, aura: AuraRef) {
    let sides: Vec<Side> = fight.target_sides().collect();
    for side in sides {
        let result = fight.calc_outcome(spell, side, Outcome::MagicHit);
        fight.deal_damage(spell, result, false);
        if result.landed() {
            let debuff = fight.aura_on(aura, side);
            fight.activate_aura(debuff);
        }
    }
}

/// Maul (9881) and its queue: the queue cast arms its aura after a realism delay, and the next
/// main hand swing then casts Maul in its place, if Maul can still be cast.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Maul {
    pub(crate) spell: SpellId,
    pub(crate) queue_aura: AuraRef,
    pub(crate) realism: (TimerId, i64),
    pub(crate) flat_damage: f64,
}

impl Maul {
    fn realism_ready(&self, fight: &Fight<DruidAgent>) -> bool {
        fight.timers[self.realism.0] <= fight.now
    }

    /// The queue cast's `ExtraCastCondition`.
    pub(crate) fn can_queue(&self, fight: &Fight<DruidAgent>) -> bool {
        !fight.aura(self.queue_aura).active
            && !fight.agent.maul_queued
            && fight.current_rage() >= fight.current_cost(self.spell)
            && self.realism_ready(fight)
    }

    /// The queue cast's `ApplyEffects`.
    pub(crate) fn queue(&self, fight: &mut Fight<DruidAgent>) {
        if self.realism_ready(fight) {
            fight.agent.maul_queued = true;
            let (timer, delay) = self.realism;
            fight.timers[timer] = fight.now + delay;
            fight.start_class_periodic(MAUL_QUEUE_TAG, delay, 1, PRIORITY_GCD);
        }
    }

    /// The realism delay's pending action.
    pub(crate) fn arm(&self, fight: &mut Fight<DruidAgent>) {
        fight.activate_aura(self.queue_aura);
        fight.agent.maul_queued = false;
    }

    /// Go `TryMaul`.
    pub(crate) fn replace_swing(&self, fight: &mut Fight<DruidAgent>, swing: SpellId) -> SpellId {
        if !fight.aura(self.queue_aura).active {
            return swing;
        }
        if !fight.can_cast(self.spell) {
            fight.deactivate_aura(self.queue_aura);
            return swing;
        }
        self.spell
    }

    /// Maul's `ApplyEffects`.
    pub(crate) fn strike(&self, fight: &mut Fight<DruidAgent>, spell: SpellId, target: Side) {
        let attack_power = fight.melee_attack_power();
        let base = self.flat_damage + fight.mh_weapon_damage(attack_power);
        let result = fight.calc_physical_damage(
            spell,
            target,
            base,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
        if !result.landed() {
            fight.issue_refund(spell);
        }
        fight.deactivate_aura(self.queue_aura);
    }
}

/// Lacerate (1235827): a weapon share a stack on the hit, and a stacking bleed whose tick is a
/// flat amount a stack, snapshot once the stacks are in.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Lacerate {
    pub(crate) tick_base: f64,
    pub(crate) weapon_share_per_stack: f64,
    pub(crate) max_stacks: i32,
    pub(crate) tick_can_crit: bool,
}

impl Lacerate {
    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>, spell: SpellId, target: Side) {
        let dot = fight.spells[spell].dot.expect("Lacerate has a dot");
        let dot = fight.dot_on(dot, target);
        let aura = fight.dots[dot].aura;
        let stacks = (fight.aura(aura).stacks + 1).min(self.max_stacks);
        let attack_power = fight.melee_attack_power();
        let base =
            fight.mh_weapon_damage(attack_power) * self.weapon_share_per_stack * f64::from(stacks);
        let result = fight.calc_physical_damage(
            spell,
            target,
            base,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
        if result.landed() {
            if fight.aura(aura).active {
                fight.refresh_aura(aura);
                fight.add_stack(aura);
            } else {
                fight.apply_dot(dot);
                fight.set_stacks(aura, 1);
            }
            // Snapshot again once the stacks are in.
            let stacks = f64::from(fight.aura(aura).stacks);
            fight
                .agent
                .lacerate_snapshot
                .set(dot, self.tick_base * stacks);
        } else {
            fight.issue_refund(spell);
        }
    }

    /// A tick on the stored amount and the current attacker multiplier.
    pub(crate) fn tick(&self, fight: &mut Fight<DruidAgent>, dot: DotId) {
        let state = &fight.dots[dot];
        let (spell, side, multiplier) = (state.spell, state.side, state.periodic_damage_multiplier);
        let base = fight.agent.lacerate_snapshot.get(dot);
        let attacker = fight.attacker_multiplier(spell, true) * multiplier;
        let result = fight.calc_physical_periodic(spell, side, base, attacker, self.tick_can_crit);
        fight.deal_damage(spell, result, true);
    }
}

/// Swipe (9908): a flat hit plus a share of the attack power on each of the first three targets
/// in unit index order, whichever target the cast named, each hit calculated and dealt in turn.
pub(crate) fn swipe(
    fight: &mut Fight<DruidAgent>,
    spell: SpellId,
    flat_damage: f64,
    attack_power_coefficient: f64,
) {
    for position in 0..fight.targets.len().min(3) {
        let attack_power = fight.melee_attack_power();
        // The arm64 build fuses the share's multiply into the add.
        let base = attack_power_coefficient.mul_add(attack_power, flat_damage);
        let result = fight.calc_physical_damage(
            spell,
            Side::target(position),
            base,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
    }
}

/// Primal Bite (1238073): flat damage plus main hand weapon damage on the target. While Berserk
/// is up it strikes up to three targets, from the cast target on in unit index order, each hit
/// calculated and dealt in turn, and lifts the cooldown.
pub(crate) fn primal_bite(
    fight: &mut Fight<DruidAgent>,
    spell: SpellId,
    target: Side,
    flat_damage: f64,
) {
    let berserk = fight.agent.berserk.map(|berserk| berserk.aura);
    let berserk_up =
        |fight: &Fight<DruidAgent>| berserk.is_some_and(|aura| fight.aura(aura).active);
    let hits = if berserk_up(fight) {
        fight.targets.len().min(3)
    } else {
        1
    };
    let mut current = target;
    for hit in 0..hits {
        let attack_power = fight.melee_attack_power();
        let base = flat_damage + fight.mh_weapon_damage(attack_power);
        let result = fight.calc_physical_damage(
            spell,
            current,
            base,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
        if hit == 0 && !result.landed() {
            fight.issue_refund(spell);
        }
        if hits > 1 {
            current = fight.next_target(current);
        }
    }
    if berserk_up(fight) {
        if let Some((timer, _)) = fight.spells[spell].cd {
            fight.timers[timer] = STARTING_CD_TIME;
        }
    }
}
