//! Death Mark (1316709 and ranks to 1325758), from the rework of push 113483 (Forever's rules
//! only): a finishing move that marks the target for 6 sec. The client's rows state its cost,
//! cooldown and duration, and its tooltip the rest: 12% of the damage the rogue's attacks deal
//! the target is stored for each Combo Point spent (Garrote and Rupture damage at twice the
//! rate), and when the mark ends the stored damage is dealt, up to a cap of
//! `(156 + points * (245 + 0.04 * attack power)) * (crit chance + 1)`. Coup de Grace multiplies
//! the rate and the cap.
//!
//! The tooltip does not say which damage counts. Warcraft Logs do (in-app report of 24 November
//! 2026, report DZCc2gtwhW3FBKyx, 9 of 12 five-point marks within 0.07%): only the rogue's own
//! physical strikes, its white swings and the `stored_class_spells`, at the damage dealt after
//! armor, with Garrote and Rupture at twice the rate. Poisons, Windfury's extra attacks, Shank,
//! racials, item procs and auras store nothing. A miss stores nothing. The release is a hit that
//! cannot miss or crit (the cap holds the crit factor) and takes no armor or multiplier again,
//! which the same logs bear out. A mark still up when the fight ends releases nothing (an
//! assumption, flagged in the README).

use crate::core::fight::{Agent, AuraRef, Fight, Side, SpellId, SpellResult, OUTCOME_HIT};

/// Go `SpellSchoolPhysical`.
const PHYSICAL: u8 = 1;

use crate::classes::rogue::spells::finisher::Finisher;

#[derive(Clone, Debug)]
pub(crate) struct DeathMark {
    pub(crate) aura: AuraRef,
    spell: SpellId,
    stored_per_point: f64,
    cap_base: f64,
    cap_per_point: f64,
    attack_power_per_point: f64,
    coup_de_grace: f64,
    /// By spell, how many times the share its damage stores: 0 for what Death Mark ignores, 1
    /// for a white swing or a stored strike, 2 for Garrote and Rupture.
    rate: Vec<f64>,
}

/// The mark on the target: the Combo Points spent, the damage stored so far and the target.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Mark {
    points: i32,
    stored: f64,
    target: Option<Side>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    aura: &str,
    stored_per_point: f64,
    cap_base: f64,
    cap_per_point: f64,
    attack_power_per_point: f64,
    coup_de_grace: f64,
    doubled_class_spells: &[String],
    stored_class_spells: &[String],
) -> Result<DeathMark, String> {
    let aura = fight.player_aura(aura)?;
    let mut rate = vec![0.0; fight.spells.len()];
    let physical = |fight: &Fight<A>, id: SpellId| {
        fight.spells[id].caster == Side::Player && fight.spells[id].school == PHYSICAL
    };
    // The white swings only: Windfury's extra attack is a copy of the main hand's swing under
    // its own spell, which stays at 0.
    for id in [fight.autos.mh.spell, fight.autos.oh.spell]
        .into_iter()
        .flatten()
    {
        rate[id] = 1.0;
    }
    for (classes, share) in [(stored_class_spells, 1.0), (doubled_class_spells, 2.0)] {
        let names: Vec<&str> = classes.iter().map(String::as_str).collect();
        for id in fight.spells_with_class(&names) {
            if physical(fight, id) {
                rate[id] = share;
            }
        }
    }
    Ok(DeathMark {
        aura,
        spell,
        stored_per_point,
        cap_base,
        cap_per_point,
        attack_power_per_point,
        coup_de_grace,
        rate,
    })
}

impl DeathMark {
    /// The cast's effect; Stealth has already broken. The finisher spends the points and rolls
    /// its talents, then the mark starts empty (a recast restarts it).
    pub(crate) fn apply<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        target: Side,
        finisher: &Finisher,
    ) -> Mark {
        let points = fight.energy_bar().combo_points;
        finisher.apply(fight, spell);
        fight.deactivate_aura(self.aura);
        fight.activate_aura(self.aura);
        Mark {
            points,
            stored: 0.0,
            target: Some(target),
        }
    }

    /// A hit or tick of the rogue's while the mark is up: a share of what a white swing or a
    /// stored strike dealt is stored.
    pub(crate) fn store(&self, mark: &mut Mark, spell: SpellId, result: &SpellResult) {
        if spell == self.spell || mark.target != Some(result.target) || !result.landed() {
            return;
        }
        let share = self.rate.get(spell).copied().unwrap_or(0.0);
        if share == 0.0 {
            return;
        }
        let rate = self.stored_per_point * f64::from(mark.points) * self.coup_de_grace * share;
        mark.stored += result.damage * rate;
    }

    /// The mark ending by its duration: the stored damage, up to the cap, is dealt.
    pub(crate) fn release<A: Agent>(&self, fight: &mut Fight<A>, mark: Mark) {
        let Some(target) = mark.target else {
            return;
        };
        if !fight.fading_by_duration(self.aura) {
            return;
        }
        let points = f64::from(mark.points);
        let power = fight.melee_attack_power();
        let crit = fight.physical_crit_chance(self.spell);
        let cap = (self.cap_base
            + points * (self.cap_per_point + self.attack_power_per_point * power))
            * (1.0 + crit)
            * self.coup_de_grace;
        let damage = mark.stored.min(cap);
        if damage <= 0.0 {
            return;
        }
        let mut result = SpellResult {
            target,
            attacker: Side::Player,
            outcome: OUTCOME_HIT,
            damage,
            threat: 0.0,
            armor_multiplier: 1.0,
        };
        result.threat = fight.threat_of(self.spell, &result);
        fight.deal_damage(self.spell, result, false);
    }
}
