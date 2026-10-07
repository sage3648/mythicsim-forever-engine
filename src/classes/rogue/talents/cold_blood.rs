//! Cold Blood (14177), from Go sim/rogue/talents_assassination.go `registerColdBlood`: the
//! cast activates an aura whose attached modifier raises the crit chance of the masked
//! strikes and finishers, until one of them deals a hit, landed or not. Mutilate's hand
//! strikes take the crit but do not spend it (community #690).

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct ColdBlood {
    pub(crate) aura: AuraRef,
    crit_mod: ModId,
    /// Whether each spell, by spellbook position, carries a class spell that spends the aura.
    spends: Vec<bool>,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    crit_bonus: f64,
    class_spells: &[String],
    spend_class_spells: &[String],
) -> Result<ColdBlood, String> {
    let aura = fight.player_aura(aura)?;
    let names: Vec<&str> = class_spells.iter().map(String::as_str).collect();
    let crit_mod = fight.register_mod(
        ModKind::BonusCritPercent,
        crit_bonus,
        0,
        fight.spells_with_class(&names),
    );
    let spends = fight
        .spells
        .iter()
        .map(|spell| {
            spell
                .class_spell
                .as_deref()
                .is_some_and(|class| spend_class_spells.iter().any(|name| name == class))
        })
        .collect();
    Ok(ColdBlood {
        aura,
        crit_mod,
        spends,
    })
}

impl ColdBlood {
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.crit_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.crit_mod);
    }

    /// The aura's `OnSpellHitDealt`.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        if self.spends[spell] {
            fight.deactivate_aura(self.aura);
        }
    }
}
