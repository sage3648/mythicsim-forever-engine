//! Arcane Concentration (talent 11213) and Clearcasting (12536), from Go
//! sim/mage/talents_arcane.go `registerArcaneConcentration`. A landed damaging Mage
//! spell can make the next costed Mage cast free, at most once per internal cooldown. The
//! missiles of Arcane Missiles do not count, and Blizzard rolls on its cast, once for each
//! enemy it hits, not on its ticks. Under Forever's rules, since the development notes of 2
//! November 2026, each missile that lands rolls too, and an Arcane Missiles cast that finds
//! Missile Barrage up keeps the Clearcasting. Forever's rules also roll once as an Arcane
//! Missiles channel starts and once on each Flamestrike ground tick, as the combat logs show.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

/// Go `MageSpellsAllDamaging`, shared with Fingers of Frost.
use crate::classes::mage::masks::DAMAGING;

#[derive(Clone, Debug)]
pub(crate) struct ArcaneConcentration {
    pub(crate) clearcasting: AuraRef,
    pub(crate) trigger: AuraRef,
    proc_chance: f64,
    /// Forever's rules: the missiles, the Arcane Missiles channel start and the Flamestrike
    /// ground ticks roll, and Missile Barrage shields the Clearcasting.
    missiles: bool,
    /// The Missile Barrage aura, when the Mage has the talent and `missiles` is on.
    missile_barrage: Option<AuraRef>,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    clearcasting: &str,
    trigger: &str,
    proc_chance: f64,
    missiles: bool,
) -> Result<ArcaneConcentration, String> {
    Ok(ArcaneConcentration {
        clearcasting: fight.player_aura(clearcasting)?,
        trigger: fight.player_aura(trigger)?,
        proc_chance,
        missiles,
        missile_barrage: if missiles {
            fight.player_aura("Missile Barrage").ok()
        } else {
            None
        },
    })
}

impl ArcaneConcentration {
    /// Clearcasting OnGain and OnExpire: Go's integer spell cost percentage.
    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.player.spell_cost_percent_modifier -= 100;
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.player.spell_cost_percent_modifier += 100;
    }

    /// Clearcasting OnCastComplete, which the reference spends it in (Forever's rules spend it in
    /// [`Self::on_apply_effects`] instead).
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        self.spend(fight, spell);
    }

    /// Clearcasting OnApplyEffects, under Forever's rules (wowsims/forever #831): the cast's
    /// effects have not applied yet, so Missile Barrage is still up on the Arcane Missiles cast it
    /// pays for. Spent at the end of the cast, the check found it already gone whenever Missile
    /// Barrage was gained before the Clearcasting, because a cast's callbacks run in the order the
    /// auras were last activated (a quarter of the casts in the Arcane board's fights).
    pub(crate) fn on_apply_effects<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        self.spend(fight, spell);
    }

    /// A Clearcasting gained in this timestep belongs to the next cast; any other costed Mage cast
    /// consumes it. An Arcane Missiles cast with Missile Barrage up does not, under Forever's
    /// rules.
    fn spend<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        let aura = fight.aura(self.clearcasting);
        if aura.remaining(fight.now) == aura.duration {
            return;
        }
        let state = &fight.spells[spell];
        if state.class_spell.is_none() || state.cost.is_none_or(|cost| cost.base == 0) {
            return;
        }
        if state.class_spell.as_deref() == Some("arcane_missiles_cast")
            && self
                .missile_barrage
                .is_some_and(|barrage| fight.aura(barrage).active)
        {
            return;
        }
        fight.deactivate_aura(self.clearcasting);
    }

    /// The "Arcane Concentration" proc trigger, with its internal cooldown.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let state = &fight.spells[spell];
        // The proc trigger's mask is every damaging spell but the Arcane Missiles ticks, which
        // the channel triggers; Forever's rules let each missile roll.
        let damaging = state.class_spell.as_deref().is_some_and(|class| {
            (self.missiles || class != "arcane_missiles_tick") && DAMAGING.contains(&class)
        });
        if state.flags.proc || !damaging || !result.landed() {
            return;
        }
        self.roll(fight);
    }

    /// Forever's rules: an Arcane Missiles channel rolls once as it starts, on top of each
    /// missile's roll. In Forever combat logs Clearcasting came from 8.9% of 347 channel starts,
    /// and 45.3% of the 5-missile channels gained it, against 46.9% expected with the start roll
    /// and 41.0% without. The cast's effects run after Clearcasting is spent
    /// ([`Self::on_apply_effects`]), so a plain channel spends the one it found and a proc from
    /// its start is a fresh one the same cast does not spend.
    pub(crate) fn on_channel_start<A: Agent>(&self, fight: &mut Fight<A>) {
        if self.missiles {
            self.roll(fight);
        }
    }

    /// Forever's rules: a Flamestrike ground tick rolls once, however many enemies it hits, on
    /// the first it lands on. In Forever combat logs Clearcasting came from 13 of 82 ticks.
    pub(crate) fn on_flamestrike_tick<A: Agent>(&self, fight: &mut Fight<A>) {
        if self.missiles {
            self.roll(fight);
        }
    }

    /// The roll under the trigger's internal cooldown, which a proc starts.
    fn roll<A: Agent>(&self, fight: &mut Fight<A>) {
        let icd = fight.aura(self.trigger).icd;
        if let Some((timer, _)) = icd {
            if fight.timers[timer] > fight.now {
                return;
            }
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return;
        }
        if let Some((timer, duration)) = icd {
            fight.timers[timer] = fight.now + duration;
        }
        fight.activate_aura(self.clearcasting);
    }
}
