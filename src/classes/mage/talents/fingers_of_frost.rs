//! Fingers of Frost (talent 400647, aura 400669) with Shatter (11170), from Go
//! sim/mage/talents_frost.go `registerFingersOfFrost`. A landed chill spell can grant
//! charges that treat the target as frozen and add Shatter's crit to every Mage spell.
//! Frostbolt and Frostfire Bolt roll it at launch, as the beta's logs gain the aura at the cast
//! end (community #776): the next cast takes the bonus and a charge, the bolt's own impact
//! neither. Cone of Cold and Improved Blizzard's ticks land at once and roll on their hits.

use crate::{
    classes::mage::masks::{is_class, DAMAGING},
    core::fight::{Agent, AuraRef, Fight, ModId, ModKind, Side, SpellId, SpellResult},
};

/// The chills the trigger hears: Go `MageSpellConeOfCold | MageSpellImprovedBlizzard`.
const CHILL: &[&str] = &["cone_of_cold", "improved_blizzard"];

#[derive(Clone, Debug)]
pub(crate) struct FingersOfFrost {
    pub(crate) aura: AuraRef,
    pub(crate) trigger: AuraRef,
    proc_chance: f64,
    shatter_mod: ModId,
    /// Go `launcher`: the bolt whose launch granted the charges, which spends none.
    launcher: Option<SpellId>,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    trigger: &str,
    proc_chance: f64,
    shatter_crit: f64,
) -> Result<FingersOfFrost, String> {
    let find = |label: &str| {
        fight.trackers[Side::Player.index()]
            .find(label)
            .map(|index| AuraRef {
                side: Side::Player,
                index,
            })
            .ok_or_else(|| format!("Fingers of Frost aura {label} is not registered"))
    };
    let (aura, trigger) = (find(aura)?, find(trigger)?);
    // Go MageSpellsAll: every spell with a Mage class mask.
    let affected = (0..fight.spells.len())
        .filter(|&spell| {
            fight.spells[spell].class_spell.is_some() && !fight.spells[spell].flags.no_spell_mods
        })
        .collect();
    let shatter_mod = fight.register_mod(ModKind::BonusCritPercent, shatter_crit, 0, affected);
    Ok(FingersOfFrost {
        aura,
        trigger,
        proc_chance,
        shatter_mod,
        launcher: None,
    })
}

impl FingersOfFrost {
    /// Go's `IsTargetFrozen`.
    pub(crate) fn frozen<A: Agent>(&self, fight: &Fight<A>) -> bool {
        fight.aura(self.aura).active
    }

    pub(crate) fn on_gain<A: Agent>(&mut self, fight: &mut Fight<A>) {
        fight.activate_mod(self.shatter_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&mut self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.shatter_mod);
    }

    /// Returns whether a charge should be removed; the caller removes it so the aura's
    /// own callbacks run with this state already stored. The launching bolt spends none.
    pub(crate) fn on_cast_complete<A: Agent>(
        &mut self,
        fight: &mut Fight<A>,
        spell: SpellId,
    ) -> bool {
        if !is_class(fight.spells[spell].class_spell.as_deref(), DAMAGING) {
            return false;
        }
        if self.launcher == Some(spell) {
            self.launcher = None;
            return false;
        }
        true
    }

    /// Go `procFingersOfFrost`: Frostbolt or Frostfire Bolt at launch, on the hit already rolled.
    /// Returns whether the charges are granted, which the caller does.
    pub(crate) fn launch<A: Agent>(
        &mut self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) -> bool {
        if result.landed() && fight.proc(self.proc_chance, "Fingers of Frost") {
            self.launcher = Some(spell);
            return true;
        }
        false
    }

    /// The "Fingers of Frost Trigger" proc: a landed chill spell.
    pub(crate) fn should_proc<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) -> bool {
        let state = &fight.spells[spell];
        if state.flags.proc || !is_class(state.class_spell.as_deref(), CHILL) || !result.landed() {
            return false;
        }
        if self.proc_chance != 1.0 && fight.random_for_aura(self.trigger) > self.proc_chance {
            return false;
        }
        true
    }
}
