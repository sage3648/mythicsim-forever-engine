//! The hunter's pet, from Go sim/hunter/pet.go, pet_abilities.go and the pet auras of
//! talents_beast_mastery.go: its custom rotation, its focus abilities, Intimidation, Bestial
//! Wrath and Frenzy. The pet itself, its stats, swings, focus and movement, is the runtime's
//! simulated pet.

use crate::core::fight::{
    melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId, SpellResult, OUTCOME_CRIT,
    OUTCOME_LANDED,
};

/// Go `HunterPet.ExecuteCustomRotation`'s inputs.
#[derive(Clone, Debug)]
pub(crate) struct PetAi {
    /// Go `PetConfig.CustomRotation`: the cat's, or the default order.
    cat: bool,
    special: Option<SpellId>,
    focus_dump: Option<SpellId>,
    extra: Option<SpellId>,
    wait: i64,
    melee_range: f64,
    move_to: f64,
}

/// The pet's spell at a position in its spellbook, or none for -1.
fn pet_spell<A: Agent>(fight: &Fight<A>, position: i64) -> Result<Option<SpellId>, String> {
    if position < 0 {
        return Ok(None);
    }
    let pet_spells: Vec<SpellId> = (0..fight.spells.len())
        .filter(|&spell| fight.spells[spell].caster == Side::Pet)
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
        rotation: &str,
        special: i64,
        focus_dump: i64,
        extra: i64,
        wait: i64,
        melee_range: f64,
        move_to: f64,
    ) -> Result<Self, String> {
        let cat = match rotation {
            "cat" => true,
            "default" => false,
            other => return Err(format!("the pet rotation {other} is unsupported")),
        };
        Ok(PetAi {
            cat,
            special: pet_spell(fight, special)?,
            focus_dump: pet_spell(fight, focus_dump)?,
            extra: pet_spell(fight, extra)?,
            wait,
            melee_range,
            move_to,
        })
    }

    /// Go `ExecuteCustomRotation` at full uptime: move into melee range, then cast the first
    /// ability that can be cast in the family's order, otherwise wait while the GCD is free.
    pub(crate) fn rotation<A: Agent>(&self, fight: &mut Fight<A>) {
        if fight.unit_config(Side::Pet).distance > self.melee_range {
            if !fight.unit(Side::Pet).moving {
                fight.move_to(Side::Pet, self.move_to);
            }
            return;
        }
        let order: &[Option<SpellId>] = if self.cat {
            &[self.special, self.focus_dump]
        } else {
            &[self.extra, self.special, self.focus_dump]
        };
        for &spell in order.iter().flatten() {
            if fight.can_cast(spell) {
                fight.cast(spell, Side::Target);
                return;
            }
        }
        if fight.gcd_ready_of(Side::Pet) {
            let ready = fight.now + self.wait;
            fight.wait_until_of(Side::Pet, ready);
        }
    }
}

/// A pet ability's `ApplyEffects`: a rolled base (Go sim.Roll) on the melee special table.
pub(crate) fn strike<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    range: (f64, f64),
) {
    let (min, max) = range;
    let base = min + (max - min) * fight.random("Damage Roll");
    let result = fight.calc_physical_damage(
        spell,
        target,
        base,
        PhysicalOutcome::MeleeSpecialHitAndCrit { count: true },
    );
    fight.deal_damage(spell, result, false);
}

/// The pet auras the hunter's talents give it.
#[derive(Clone, Debug, Default)]
pub(crate) struct PetAuras {
    /// Intimidation's aura and the pet's physical crit while it holds.
    pub(crate) intimidation: Option<(AuraRef, f64)>,
    /// Bestial Wrath's aura and its damage multiplier.
    pub(crate) bestial_wrath: Option<(AuraRef, f64)>,
    /// Frenzy's trigger, its effect aura, chance and attack speed multiplier.
    pub(crate) frenzy: Option<(AuraRef, AuraRef, f64, f64)>,
}

impl PetAuras {
    pub(crate) fn pet_aura<A: Agent>(fight: &Fight<A>, label: &str) -> Result<AuraRef, String> {
        fight.trackers[Side::Pet.index()]
            .find(label)
            .map(|index| AuraRef {
                side: Side::Pet,
                index,
            })
            .ok_or_else(|| format!("pet aura {label} is not registered"))
    }

    /// Intimidation's gain and expiry: Go `AddStatDynamic` on the pet's physical crit, whose
    /// value with the aura active the exporter reads.
    pub(crate) fn intimidation_changed<A: Agent>(&self, fight: &mut Fight<A>, gained: bool) {
        let (_, crit) = self.intimidation.expect("Intimidation is bound");
        let base = fight.unit_config(Side::Pet).powers.physical_crit_percent;
        fight.unit_mut(Side::Pet).powers.physical_crit_percent = if gained { crit } else { base };
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
        let unit = fight.unit_mut(Side::Pet);
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
        fight.multiply_attack_speed_of(Side::Pet, amount);
    }
}
