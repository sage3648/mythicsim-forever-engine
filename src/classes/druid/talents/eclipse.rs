//! Eclipse (408248, aura 408255), from Go sim/druid/talents_balance.go `applyEclipse`, new in
//! Forever: each Wrath banks charges, and while any remain Starfire casts faster; each Starfire
//! spends one. Under Forever's rules only a Wrath that lands banks them (408248 procs on a
//! harmful spell's damage); the Go reference banks them on every completed Wrath cast.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId, SpellResult};

#[derive(Clone, Debug)]
pub(crate) struct Eclipse {
    pub(crate) aura: AuraRef,
    charges_per_wrath: i32,
    bank_on_landed_hit: bool,
    cast_time_mod: ModId,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    cast_time_reduction: i64,
    charges_per_wrath: i32,
    bank_on_landed_hit: bool,
) -> Result<Eclipse, String> {
    let aura = fight.player_aura(aura)?;
    let starfire = fight.spells_with_class(&["starfire"]);
    let cast_time_mod =
        fight.register_mod(ModKind::CastTimeFlat, 0.0, cast_time_reduction, starfire);
    Ok(Eclipse {
        aura,
        charges_per_wrath,
        bank_on_landed_hit,
        cast_time_mod,
    })
}

impl Eclipse {
    /// The permanent trigger aura's OnCastComplete.
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        match fight.spells[spell].class_spell.as_deref() {
            Some("wrath") if !self.bank_on_landed_hit => self.bank(fight),
            Some("starfire") if fight.aura(self.aura).active => fight.remove_stack(self.aura),
            _ => {}
        }
    }

    /// The permanent trigger aura's OnSpellHitDealt, under Forever's rules.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if self.bank_on_landed_hit
            && result.landed()
            && fight.spells[spell].class_spell.as_deref() == Some("wrath")
        {
            self.bank(fight);
        }
    }

    fn bank<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_aura(self.aura);
        let stacks = fight.aura(self.aura).stacks + self.charges_per_wrath;
        fight.set_stacks(self.aura, stacks);
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cast_time_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cast_time_mod);
    }
}
