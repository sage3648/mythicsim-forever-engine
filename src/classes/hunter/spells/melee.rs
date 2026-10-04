//! The hunter's melee abilities, from Go sim/hunter/raptor_strike.go, mongoose_bite.go,
//! lacerating_strikes.go, strider_kick.go and wing_clip.go.
//!
//! Raptor Strike replaces a main hand swing: the rotation casts the queue spell, whose aura
//! lifts the dual wield miss penalty, and the next swing casts Raptor Strike instead when it
//! can, which casts its hit and ends the queue. Mongoose Bite needs the Defensive State window,
//! which it closes; a landed bite with Lacerating Strikes bleeds a share of its damage over
//! the bleed's ticks, a snapshot a new bite's application clears, as Go's does.

use crate::core::fight::{
    melee::PhysicalOutcome, Agent, AuraRef, DotId, Fight, Outcome, Side, SpellId,
};

/// Go's weapon special table for the melee abilities.
const WEAPON_SPECIAL: PhysicalOutcome =
    PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true };

/// The bound Raptor Strike.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RaptorStrike {
    pub(crate) spell: SpellId,
    pub(crate) hit: SpellId,
    pub(crate) queue_aura: AuraRef,
    pub(crate) base_damage: f64,
    melee_range: f64,
}

impl RaptorStrike {
    pub(crate) fn bind<A: Agent>(
        fight: &Fight<A>,
        spell_id: i32,
        queue_aura: &str,
        base_damage: f64,
        melee_range: f64,
    ) -> Result<Self, String> {
        let find = |tag: i32| {
            fight
                .spells
                .iter()
                .position(|spell| {
                    spell.caster == Side::Player
                        && spell.id.spell_id == spell_id
                        && spell.id.tag == tag
                        && spell.id.item_id == 0
                })
                .ok_or_else(|| format!("Raptor Strike {spell_id} tag {tag} is not registered"))
        };
        Ok(RaptorStrike {
            spell: find(0)?,
            hit: find(1)?,
            queue_aura: fight.player_aura(queue_aura)?,
            base_damage,
            melee_range,
        })
    }

    /// The queue spell's `ExtraCastCondition`.
    pub(crate) fn can_queue<A: Agent>(&self, fight: &Fight<A>) -> bool {
        !fight.aura(self.queue_aura).active
            && fight.player.mana >= fight.current_cost(self.spell)
            && fight.player.hardcast.expires <= fight.now
            && fight.config.distance <= self.melee_range
            && fight.spell_ready(self.spell)
    }

    /// Go `TryRaptorStrike`: a queued Raptor Strike that can be cast takes the swing.
    pub(crate) fn replace<A: Agent>(&self, fight: &mut Fight<A>, swing: SpellId) -> SpellId {
        if fight.aura(self.queue_aura).active && fight.can_cast(self.spell) {
            self.spell
        } else {
            swing
        }
    }

    /// Raptor Strike's `ApplyEffects`: its hit, then the queue ends.
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>, target: Side) {
        fight.cast(self.hit, target);
        fight.deactivate_aura(self.queue_aura);
    }

    /// The hit's `ApplyEffects`: the flat damage on a main hand weapon swing.
    pub(crate) fn apply_hit<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        let attack_power = fight.melee_attack_power();
        let damage = self.base_damage + fight.mh_weapon_damage(attack_power);
        let result = fight.calc_physical_damage(spell, target, damage, WEAPON_SPECIAL);
        fight.deal_damage(spell, result, false);
    }

    /// The queue aura's gain and expiry: Go `DisableDWMissPenalty`.
    pub(crate) fn queue_changed<A: Agent>(fight: &mut Fight<A>, gained: bool) {
        fight.player.disable_dw_miss_penalty = gained;
    }
}

/// The bound Mongoose Bite and Lacerating Strikes.
#[derive(Clone, Debug)]
pub(crate) struct MongooseBite {
    pub(crate) window: AuraRef,
    base_damage: f64,
    /// Lacerating Strikes: its spell, dot, share of the bite and tick outcome.
    lacerating: Option<(SpellId, DotId, f64, Outcome)>,
    /// The bleed's stored base and attacker multiplier, which its expiry clears.
    pub(crate) bleed: (f64, f64),
}

impl MongooseBite {
    pub(crate) fn bind<A: Agent>(
        fight: &Fight<A>,
        spell: SpellId,
        window: &str,
        base_damage: f64,
        lacerating: Option<(f64, Outcome)>,
    ) -> Result<Self, String> {
        let lacerating = match lacerating {
            Some((share, outcome)) => {
                let id = &fight.spells[spell].id;
                let bleed = fight
                    .spells
                    .iter()
                    .position(|other| {
                        other.caster == Side::Player
                            && other.id.spell_id == id.spell_id
                            && other.id.tag == 1
                    })
                    .ok_or("Lacerating Strikes is not registered")?;
                let dot = fight.spells[bleed]
                    .dot
                    .ok_or("Lacerating Strikes has no dot")?;
                Some((bleed, dot, share, outcome))
            }
            None => None,
        };
        Ok(MongooseBite {
            window: fight.player_aura(window)?,
            base_damage,
            lacerating,
            bleed: (0.0, 0.0),
        })
    }

    /// The bite's `ApplyEffects`: the window closes, then the flat damage on a normalized main
    /// hand swing. Returns the bleed's snapshot when a landed bite procs Lacerating Strikes.
    pub(crate) fn apply<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        target: Side,
    ) -> Option<(SpellId, f64)> {
        fight.deactivate_aura(self.window);
        let attack_power = fight.melee_attack_power();
        let damage = self.base_damage + fight.mh_normalized_weapon_damage(attack_power);
        let result = fight.calc_physical_damage(spell, target, damage, WEAPON_SPECIAL);
        fight.deal_damage(spell, result, false);
        let (bleed, dot, share, _) = self.lacerating?;
        if !result.landed() {
            return None;
        }
        let ticks = fight.dots[dot].base_tick_count;
        Some((bleed, result.damage * share / f64::from(ticks)))
    }

    /// The bleed's cast: Go `Dot.Apply`, without a snapshot of its own.
    pub(crate) fn apply_bleed<A: Agent>(&self, fight: &mut Fight<A>) {
        let (_, dot, _, _) = self.lacerating.expect("Lacerating Strikes is bound");
        fight.apply_dot(dot);
    }

    /// A bleed tick on its stored base and multiplier.
    pub(crate) fn bleed_tick<A: Agent>(&self, fight: &mut Fight<A>, dot: DotId) {
        let (_, _, _, outcome) = self.lacerating.expect("Lacerating Strikes is bound");
        let (spell, side) = (fight.dots[dot].spell, fight.dots[dot].side);
        let (base, multiplier) = self.bleed;
        let result = fight.calc_tick_damage(spell, side, base, multiplier, outcome);
        fight.deal_damage(spell, result, true);
    }
}

/// Strider Kick's `ApplyEffects`: a normalized main hand swing.
pub(crate) fn strider_kick<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let attack_power = fight.melee_attack_power();
    let damage = fight.mh_normalized_weapon_damage(attack_power);
    let result = fight.calc_physical_damage(spell, target, damage, WEAPON_SPECIAL);
    fight.deal_damage(spell, result, false);
}

/// Wing Clip's `ApplyEffects`: its damage effect on the weapon special table.
pub(crate) fn wing_clip<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side, base: f64) {
    let result = fight.calc_physical_damage(spell, target, base, WEAPON_SPECIAL);
    fight.deal_damage(spell, result, false);
}

/// Immolation Trap's `ApplyEffects`: the outcome is dealt, then a landed trap burns.
pub(crate) fn immolation_trap<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHitNoHitCounter);
    fight.deal_damage(spell, result, false);
    if result.landed() {
        let dot = fight.spells[spell].dot.expect("Immolation Trap has a dot");
        fight.apply_dot(dot);
    }
}
