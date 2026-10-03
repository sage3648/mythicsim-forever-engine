//! Holy Shock and Divine Favor, from Go sim/paladin/holy_shock.go and divine_favor.go.

use crate::core::fight::{AuraRef, Fight, ModId, ModKind, Side, SpellId};

use super::super::agent::PaladinAgent;

/// Holy Shock's damage: the rank's roll, a magic hit and crit.
pub(crate) fn holy_shock(
    fight: &mut Fight<PaladinAgent>,
    spell: SpellId,
    target: Side,
    (min, max): (f64, f64),
) {
    let base = min + (max - min) * fight.random("Damage Roll");
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
}

/// Divine Favor: its aura carries a crit mod on the spells it names and fades when one of
/// them is cast.
#[derive(Clone, Debug)]
pub(crate) struct DivineFavor {
    pub(crate) aura: AuraRef,
    pub(crate) crit_mod: ModId,
    /// The spells whose casts spend the aura.
    pub(crate) spells: Vec<bool>,
}

impl DivineFavor {
    pub(crate) fn bind(
        fight: &mut Fight<PaladinAgent>,
        aura: &str,
        crit: f64,
        names: &[String],
    ) -> Result<Self, String> {
        let aura = fight.player_aura(aura)?;
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let affected = fight.spells_with_class(&names);
        // The trigger matches the class mask alone, mods or not.
        let spells = fight
            .spells
            .iter()
            .map(|spell| {
                spell
                    .class_spell
                    .as_deref()
                    .is_some_and(|class| names.contains(&class))
            })
            .collect();
        let crit_mod = fight.register_mod(ModKind::BonusCritPercent, crit, 0, affected);
        Ok(DivineFavor {
            aura,
            crit_mod,
            spells,
        })
    }

    /// The spell activates the aura.
    pub(crate) fn apply(&self, fight: &mut Fight<PaladinAgent>) {
        fight.activate_aura(self.aura);
    }

    pub(crate) fn on_gain(&self, fight: &mut Fight<PaladinAgent>) {
        fight.activate_mod(self.crit_mod);
    }

    pub(crate) fn on_expire(&self, fight: &mut Fight<PaladinAgent>) {
        fight.deactivate_mod(self.crit_mod);
    }

    /// The aura's trigger: a cast of a named spell, procs included, spends it at once.
    pub(crate) fn on_cast_complete(&self, fight: &mut Fight<PaladinAgent>, spell: SpellId) {
        if self.spells[spell] {
            fight.deactivate_aura(self.aura);
        }
    }
}
