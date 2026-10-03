//! Fingers of Frost (talent 400647, aura 400669) with Shatter (11170), from Go
//! sim/mage/talents_frost.go `registerFingersOfFrost`. A landed chill spell can grant
//! charges that treat the target as frozen and add Shatter's crit to every Mage spell.
//! A spell already being cast when the charges arrive keeps no Shatter bonus and spends
//! no charge; Go marks it in flight.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, Side, SpellId, SpellResult};

/// Go `MageSpellChill`: the chill effects Fingers of Frost rolls on.
const CHILL: &[&str] = &[
    "frostbolt",
    "cone_of_cold",
    "frostfire_bolt",
    "improved_blizzard",
];

/// Go `MageSpellsAllDamaging`: casts that spend a charge.
pub(crate) const DAMAGING: &[&str] = &[
    "arcane_blast",
    "arcane_explosion",
    "arcane_missiles_tick",
    "blizzard",
    "fire_blast",
    "fireball",
    "flamestrike",
    "frostbolt",
    "ice_lance",
    "pyroblast",
    "pyroblast_dot",
    "scorch",
    "blast_wave",
    "cone_of_cold",
    "frost_nova",
    "frostfire_bolt",
];

#[derive(Clone, Debug)]
pub(crate) struct FingersOfFrost {
    pub(crate) aura: AuraRef,
    pub(crate) trigger: AuraRef,
    proc_chance: f64,
    shatter_crit: f64,
    shatter_mod: ModId,
    /// Go `inFlight`: the spell being cast when the charges arrived.
    in_flight: Option<SpellId>,
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
        shatter_crit,
        shatter_mod,
        in_flight: None,
    })
}

fn is_class(fight_class: Option<&str>, set: &[&str]) -> bool {
    fight_class.is_some_and(|class| set.contains(&class))
}

impl FingersOfFrost {
    /// Go's `IsTargetFrozen`.
    pub(crate) fn frozen<A: Agent>(&self, fight: &Fight<A>) -> bool {
        fight.aura(self.aura).active
    }

    pub(crate) fn on_gain<A: Agent>(&mut self, fight: &mut Fight<A>) {
        fight.activate_mod(self.shatter_mod);
        self.in_flight = None;
        let hardcast = fight.player.hardcast;
        if hardcast.expires > fight.now {
            if let Some(casting) = hardcast.spell {
                let id = fight.spells[casting].id.clone();
                let found = (0..fight.spells.len()).find(|&spell| {
                    fight.spells[spell].class_spell.is_some() && fight.spells[spell].id == id
                });
                if let Some(spell) = found {
                    fight.spells[spell].bonus_crit_percent -= self.shatter_crit;
                    self.in_flight = Some(spell);
                }
            }
        }
    }

    pub(crate) fn on_expire<A: Agent>(&mut self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.shatter_mod);
        if let Some(spell) = self.in_flight.take() {
            fight.spells[spell].bonus_crit_percent += self.shatter_crit;
        }
    }

    /// Returns whether a charge should be removed; the caller removes it so the aura's
    /// own callbacks run with this state already stored.
    pub(crate) fn on_cast_complete<A: Agent>(
        &mut self,
        fight: &mut Fight<A>,
        spell: SpellId,
    ) -> bool {
        if !is_class(fight.spells[spell].class_spell.as_deref(), DAMAGING) {
            return false;
        }
        if self.in_flight == Some(spell) {
            fight.spells[spell].bonus_crit_percent += self.shatter_crit;
            self.in_flight = None;
            return false;
        }
        true
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
        if self.proc_chance != 1.0 {
            let label = fight.aura(self.trigger).label.clone();
            if fight.random(&label) > self.proc_chance {
                return false;
            }
        }
        true
    }
}
