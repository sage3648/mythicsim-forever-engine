//! The hunter's pet, from Go sim/hunter/pet.go, pet_abilities.go and the pet auras of
//! talents_beast_mastery.go: its custom rotation, its focus abilities, Intimidation, Bestial
//! Wrath and Frenzy. The pet itself, its stats, swings, focus and movement, is the runtime's
//! simulated pet.

use super::agent::HunterAgent;
use crate::{
    contracts::prepared_v2::Effect,
    core::fight::{
        melee::PhysicalOutcome, Agent, AuraRef, DotId, Fight, Outcome, Side, SpellId, SpellResult,
        OUTCOME_CRIT, OUTCOME_LANDED,
    },
    core::time::NS_PER_SECOND,
};

/// Go `DustCloudYieldsToSunderAndExpose` (mythicsim patch 110): players report that in Forever
/// Sunder Armor, Expose Armor and Dust Cloud do not stack, only the strongest applies, so the
/// Tallstrider holds Dust Cloud while Sunder Armor or Expose Armor is on the target and keeps
/// its focus for Bite. No combat log confirms it yet.
pub(crate) const DUST_CLOUD_YIELDS_TO_SUNDER_AND_EXPOSE: bool = true;

/// Go `buffs.SunderArmorCategory`, the single aura category of Sunder Armor and Expose Armor.
pub(crate) const SUNDER_ARMOR_CATEGORY: &str = "MajorArmorReduction";

/// Go `PetConfig.CustomRotation` of the pet's family.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Rotation {
    Cat,
    Scorpid,
    Default,
}

/// Go `HunterPet.ExecuteCustomRotation`'s inputs.
#[derive(Clone, Debug)]
pub(crate) struct PetAi {
    /// The simulated pet that is the hunter's.
    pub(crate) pet: Side,
    /// Go `PetConfig.CustomRotation`: the cat's, the scorpid's, or the default order.
    kind: Rotation,
    special: Option<SpellId>,
    focus_dump: Option<SpellId>,
    extra: Option<SpellId>,
    wait: i64,
    melee_range: f64,
    move_to: f64,
    /// Go `uptimePercent`.
    pub(crate) uptime: f64,
}

/// The pet's spell at a position in its spellbook, or none for -1.
fn pet_spell<A: Agent>(
    fight: &Fight<A>,
    pet: Side,
    position: i64,
) -> Result<Option<SpellId>, String> {
    if position < 0 {
        return Ok(None);
    }
    let pet_spells: Vec<SpellId> = (0..fight.spells.len())
        .filter(|&spell| fight.spells[spell].caster == pet)
        .collect();
    pet_spells
        .get(position as usize)
        .copied()
        .map(Some)
        .ok_or_else(|| format!("the pet has no spell at {position}"))
}

impl PetAi {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn bind<A: Agent>(
        fight: &Fight<A>,
        pet: Side,
        rotation: &str,
        special: i64,
        focus_dump: i64,
        extra: i64,
        wait: i64,
        melee_range: f64,
        move_to: f64,
    ) -> Result<Self, String> {
        let kind = match rotation {
            "cat" => Rotation::Cat,
            "default" => Rotation::Default,
            "scorpid" => Rotation::Scorpid,
            other => return Err(format!("the pet rotation {other} is unsupported")),
        };
        Ok(PetAi {
            pet,
            kind,
            special: pet_spell(fight, pet, special)?,
            focus_dump: pet_spell(fight, pet, focus_dump)?,
            extra: pet_spell(fight, pet, extra)?,
            wait,
            melee_range,
            move_to,
            uptime: 1.0,
        })
    }

    /// Go `ExecuteCustomRotation`: past its uptime the pet is disabled, which a disabled pet's
    /// later evaluations only log; otherwise move into melee range, then cast the first
    /// ability that can be cast in the family's order, otherwise wait while the GCD is free.
    pub(crate) fn rotation<A: Agent>(&self, fight: &mut Fight<A>) {
        let remaining = (fight.duration - fight.now) as f64 / fight.duration as f64;
        if remaining < 1.0 - self.uptime {
            fight.disable_pet(self.pet);
            return;
        }
        if fight.unit_config(self.pet).distance > self.melee_range {
            if !fight.unit(self.pet).moving {
                fight.move_to(self.pet, self.move_to);
            }
            return;
        }
        let order: &[Option<SpellId>] = match self.kind {
            Rotation::Cat => &[self.special, self.focus_dump],
            Rotation::Default => &[self.extra, self.special, self.focus_dump],
            // Go's Scorpid rotation: Scorpid Poison while the dot has stacks to gain or less
            // than three seconds left, otherwise the focus dump.
            Rotation::Scorpid => {
                let special = self.special.expect("the Scorpid has Scorpid Poison");
                let dot = fight.spells[special].dot.expect("Scorpid Poison has a dot");
                let aura = fight.aura(fight.dots[dot].aura);
                let remaining = if aura.active {
                    aura.expires - fight.now
                } else {
                    0
                };
                if aura.stacks < aura.max_stacks || remaining < 3 * NS_PER_SECOND {
                    &[self.special]
                } else {
                    &[self.focus_dump]
                }
            }
        };
        for &spell in order.iter().flatten() {
            if fight.can_cast(spell) {
                fight.cast(spell, Side::Target);
                return;
            }
        }
        if fight.gcd_ready_of(self.pet) {
            let ready = fight.now + self.wait;
            fight.wait_until_of(self.pet, ready);
        }
    }
}

/// What a pet damage ability does, from Go pet_abilities.go.
#[derive(Clone, Copy, Debug)]
pub(crate) enum PetAbility {
    /// One hit: a rolled base (Go sim.Roll, or Effect.Roll of a client row, which draws only
    /// with a variance) on the melee special table, or on the magic table.
    Strike {
        min: f64,
        max: f64,
        draws: bool,
        magic: bool,
        /// A client row's average and variance.
        roll: Option<(f64, f64)>,
        /// The targets a cleave hits at most, one for a single hit.
        cleave: usize,
    },
    /// Go `newPetBleed`: a hit roll without damage, then the dot when it landed.
    Bleed {
        ranged: bool,
        tick_base: f64,
        outcome: Outcome,
    },
    /// Go `newSwipe`, which needs `min_targets` active targets, and cleaves when they are up.
    Swipe {
        min_targets: usize,
        cleave: Option<crate::contracts::prepared_v2::PetCleave>,
    },
    /// Go `newScorpidPoison`.
    ScorpidPoison { tick_base: f64, outcome: Outcome },
    /// Go `newDustCloud`, whose aura and armor the agent holds.
    DustCloud,
    /// Go `newFuriousHowl`, whose auras the agent holds.
    FuriousHowl { radius: f64 },
}

impl PetAbility {
    /// The pet spell an effect describes.
    pub(crate) fn spell_id(effect: &Effect) -> Option<i32> {
        match effect {
            Effect::HunterPetStrike { spell_id, .. }
            | Effect::HunterPetBleed { spell_id, .. }
            | Effect::HunterPetSwipe { spell_id, .. }
            | Effect::HunterPetScorpidPoison { spell_id, .. }
            | Effect::HunterPetDustCloud { spell_id, .. }
            | Effect::HunterPetFuriousHowl { spell_id, .. } => Some(*spell_id),
            _ => None,
        }
    }

    /// The ability an effect describes, if Rust implements its shape.
    pub(crate) fn from_effect(effect: &Effect) -> Option<Self> {
        match effect {
            Effect::HunterPetStrike {
                min_damage,
                max_damage,
                draws,
                outcome,
                average,
                variance,
                cleave_targets,
                ..
            } => Some(PetAbility::Strike {
                min: *min_damage,
                max: *max_damage,
                draws: *draws,
                roll: average.zip(*variance),
                cleave: cleave_targets.map_or(Some(1), |n| usize::try_from(n).ok())?,
                magic: match outcome.as_str() {
                    "melee_special" => false,
                    "magic" => true,
                    _ => return None,
                },
            }),
            Effect::HunterPetBleed {
                hit,
                tick_base,
                tick_outcome,
                ..
            } => Some(PetAbility::Bleed {
                ranged: match hit.as_str() {
                    "melee_special" => false,
                    "ranged" => true,
                    _ => return None,
                },
                tick_base: *tick_base,
                outcome: crate::classes::hunter::spells::serpent_sting::tick_outcome(tick_outcome)?,
            }),
            Effect::HunterPetSwipe {
                min_targets,
                cleave,
                ..
            } if *min_targets > 1 => Some(PetAbility::Swipe {
                min_targets: usize::try_from(*min_targets).ok()?,
                cleave: *cleave,
            }),
            Effect::HunterPetScorpidPoison {
                tick_base,
                tick_can_crit,
                tick_magic,
                ..
            } => Some(PetAbility::ScorpidPoison {
                tick_base: *tick_base,
                // The poison's ticks roll the pet's melee crit where its row states Periodic
                // Can Crit.
                outcome: Outcome::tick_hit_rolled(*tick_can_crit, *tick_magic).ok()?,
            }),
            Effect::HunterPetDustCloud { .. } => Some(PetAbility::DustCloud),
            Effect::HunterPetFuriousHowl { radius, .. } => {
                Some(PetAbility::FuriousHowl { radius: *radius })
            }
            _ => None,
        }
    }

    /// The ability's `ApplyEffects`.
    pub(crate) fn apply(self, fight: &mut Fight<HunterAgent>, spell: SpellId, target: Side) {
        match self {
            PetAbility::Strike {
                min,
                max,
                draws,
                magic,
                roll,
                cleave,
            } => {
                // Go `CalcAndDealCleaveDamageWithVariance` for a cleave: each target's roll is
                // dealt before the next is rolled; one target for a single hit.
                for side in fight.cleave_targets(target, cleave.max(1)) {
                    // Go Effect.Roll for a client row, Simulation.Roll for a literal range.
                    let base = match roll {
                        Some((average, variance)) if magic => fight.effect_roll(average, variance),
                        Some((average, variance)) => fight.physical_effect_roll(average, variance),
                        None if draws && !magic => fight.physical_range_roll(min, max),
                        None if draws => fight.go_roll(min, max),
                        None => min,
                    };
                    let result = if magic {
                        fight.calc_damage(spell, side, base)
                    } else {
                        fight.calc_physical_damage(
                            spell,
                            side,
                            base,
                            PhysicalOutcome::MeleeSpecialHitAndCrit { count: true },
                        )
                    };
                    fight.deal_damage(spell, result, false);
                }
            }
            PetAbility::Bleed { ranged, .. } => {
                let table = if ranged {
                    PhysicalOutcome::RangedHit { count: true }
                } else {
                    PhysicalOutcome::MeleeSpecialHit { count: true }
                };
                let result = fight.calc_outcome(spell, target, Outcome::Table(table));
                fight.deal_damage(spell, result, false);
                if result.landed() {
                    let dot = fight.spells[spell].dot.expect("a pet bleed has a dot");
                    fight.apply_dot(dot);
                }
            }
            // Go `CalcAndDealCleaveDamageWithVariance`: the target and the next ones, each a
            // roll dealt before the next is rolled.
            PetAbility::Swipe { cleave, .. } => {
                let cleave = cleave.expect("Swipe casts only with its targets up");
                let count = usize::try_from(cleave.max_targets)
                    .unwrap_or(0)
                    .min(fight.targets.len());
                let mut side = target;
                for _ in 0..count {
                    let base = fight.physical_effect_roll(cleave.average, cleave.variance);
                    let result = fight.calc_physical_damage(
                        spell,
                        side,
                        base,
                        PhysicalOutcome::MeleeSpecialHitAndCrit { count: true },
                    );
                    fight.deal_damage(spell, result, false);
                    side = fight.next_target(side);
                }
            }
            PetAbility::ScorpidPoison { tick_base, .. } => {
                let table = PhysicalOutcome::MeleeSpecialHit { count: true };
                let result = fight.calc_outcome(spell, target, Outcome::Table(table));
                fight.deal_damage(spell, result, false);
                if !result.landed() {
                    return;
                }
                // Go: a landed poison on a poisoned target refreshes it and adds a stack up to
                // the dot's five, since Apply would drop the stacks (community #694); otherwise
                // it applies the dot with one. Either way the dot snapshots the tick for every
                // stack and ticks on the caster's current stats.
                let dot = fight.spells[spell].dot.expect("Scorpid Poison has a dot");
                let aura = fight.dots[dot].aura;
                if fight.aura(aura).active {
                    fight.refresh_aura(aura);
                    let state = fight.aura(aura);
                    if state.stacks < state.max_stacks {
                        fight.add_stack(aura);
                    }
                } else {
                    fight.apply_dot(dot);
                    fight.set_stacks(aura, 1);
                }
                let stacks = fight.aura(aura).stacks;
                fight.snapshot_dot(dot, tick_base * f64::from(stacks));
            }
            PetAbility::DustCloud => {
                let table = PhysicalOutcome::MeleeSpecialHit { count: true };
                let result = fight.calc_outcome(spell, target, Outcome::Table(table));
                fight.deal_damage(spell, result, false);
                if result.landed() {
                    let (aura, _) = fight.agent.dust_cloud.expect("Dust Cloud is bound");
                    fight.activate_aura(aura);
                }
            }
            PetAbility::FuriousHowl { radius } => {
                // The pet howls for itself, and for its owner while the owner stands within the
                // radius of the target (the owner's distance as Go last read it).
                let howl = fight.agent.furious_howl.expect("Furious Howl is bound");
                fight.activate_aura(howl.pet_aura);
                if fight.unit_config(Side::Player).distance <= radius {
                    fight.activate_aura(howl.owner_aura);
                }
            }
        }
    }
}

/// Go `Dot.CalcAndDealPeriodicSnapshotDamage` for a pet bleed or Scorpid Poison: the snapshot
/// base on the caster's current multiplier, with the tick outcome its row picks.
pub(crate) fn bleed_tick<A: Agent>(fight: &mut Fight<A>, dot: DotId, outcome: Outcome) {
    let state = &fight.dots[dot];
    let (spell, side, base) = (state.spell, state.side, state.snapshot_base);
    let attacker =
        fight.attacker_multiplier(spell, true) * fight.dots[dot].periodic_damage_multiplier;
    let result = fight.calc_tick_damage(spell, side, base, attacker, outcome);
    fight.deal_damage(spell, result, true);
}

/// The Wolf's Furious Howl: the auras it gives and the melee attack power the pet's takes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct FuriousHowl {
    pub(crate) pet: Side,
    pub(crate) pet_aura: AuraRef,
    pub(crate) owner_aura: AuraRef,
    pub(crate) attack_power: f64,
}

impl FuriousHowl {
    /// The pet aura's gain and expiry: Go `AddStatsDynamic` of the row's melee attack power on
    /// the pet. The owner's aura is a stat aura of the owner.
    pub(crate) fn pet_changed<A: Agent>(self, fight: &mut Fight<A>, gained: bool) {
        let amount = if gained {
            self.attack_power
        } else {
            -self.attack_power
        };
        let taken = fight.add_pet_stat_dynamic(self.pet, "AttackPower", amount);
        assert!(taken, "Furious Howl's pet recomputes its stats");
    }
}

/// The pet auras the hunter's talents give it.
#[derive(Clone, Debug, Default)]
pub(crate) struct PetAuras {
    /// The simulated pet that is the hunter's.
    pub(crate) pet: Option<Side>,
    /// Intimidation's aura and the physical crit it adds to the pet.
    pub(crate) intimidation: Option<(AuraRef, f64)>,
    /// Bestial Wrath's aura and its damage multiplier.
    pub(crate) bestial_wrath: Option<(AuraRef, f64)>,
    /// Frenzy's trigger, its effect aura, chance and attack speed multiplier.
    pub(crate) frenzy: Option<(AuraRef, AuraRef, f64, f64)>,
}

impl PetAuras {
    pub(crate) fn pet_aura<A: Agent>(
        fight: &Fight<A>,
        pet: Side,
        label: &str,
    ) -> Result<AuraRef, String> {
        fight.trackers[pet.index()]
            .find(label)
            .map(|index| AuraRef { side: pet, index })
            .ok_or_else(|| format!("pet aura {label} is not registered"))
    }

    /// Intimidation's gain and expiry: Go `AddStatDynamic` on the pet's physical crit.
    pub(crate) fn intimidation_changed<A: Agent>(&self, fight: &mut Fight<A>, gained: bool) {
        let (_, bonus) = self.intimidation.expect("Intimidation is bound");
        let amount = if gained { bonus } else { -bonus };
        let pet = self.pet.expect("the hunter has a pet");
        let taken = fight.add_pet_stat_dynamic(pet, "PhysicalCritPercent", amount);
        assert!(taken, "Intimidation's pet recomputes its stats");
    }

    /// Intimidation's `OnSpellHitDealt`: a landed hit ends it.
    pub(crate) fn intimidation_hit<A: Agent>(&self, fight: &mut Fight<A>, result: &SpellResult) {
        if result.outcome & OUTCOME_LANDED != 0 {
            let (aura, _) = self.intimidation.expect("Intimidation is bound");
            fight.deactivate_aura(aura);
        }
    }

    /// Bestial Wrath's gain and expiry on the pet's damage dealt multiplier.
    pub(crate) fn bestial_wrath_changed<A: Agent>(&self, fight: &mut Fight<A>, gained: bool) {
        let (_, multiplier) = self.bestial_wrath.expect("Bestial Wrath is bound");
        let unit = fight.unit_mut(self.pet.expect("the hunter has a pet"));
        if gained {
            unit.damage_dealt_multiplier *= multiplier;
        } else {
            unit.damage_dealt_multiplier /= multiplier;
        }
    }

    /// Go `MakeProcTriggerAura` for Frenzy: a crit by any of the pet's spells rolls the chance,
    /// then the handler waits a spell batch window.
    pub(crate) fn frenzy_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let (trigger, _, chance, _) = self.frenzy.expect("Frenzy is bound");
        if fight.spells[spell].flags.proc || result.outcome & OUTCOME_CRIT == 0 {
            return;
        }
        if chance != 1.0 && fight.random_for_aura(trigger) > chance {
            return;
        }
        fight.schedule_delayed_proc(trigger, spell, *result);
    }

    /// Frenzy's handler: its attack speed aura.
    pub(crate) fn frenzy_proc<A: Agent>(&self, fight: &mut Fight<A>) {
        let (_, aura, _, _) = self.frenzy.expect("Frenzy is bound");
        fight.activate_aura(aura);
    }

    /// Frenzy Effect's gain and expiry: Go `MultiplyAttackSpeed` on the pet.
    pub(crate) fn frenzy_changed<A: Agent>(&self, fight: &mut Fight<A>, gained: bool) {
        let (_, _, _, multiplier) = self.frenzy.expect("Frenzy is bound");
        let amount = if gained { multiplier } else { 1.0 / multiplier };
        fight.multiply_attack_speed_of(self.pet.expect("the hunter has a pet"), amount);
    }
}
