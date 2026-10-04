//! Eclipse (408248, aura 408255), from Go sim/druid/talents_balance.go `applyEclipse`, new in
//! Forever: each Wrath banks charges, and while any remain Starfire casts faster; each Starfire
//! spends one.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct Eclipse {
    pub(crate) aura: AuraRef,
    charges_per_wrath: i32,
    cast_time_mod: ModId,
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    cast_time_reduction: i64,
    charges_per_wrath: i32,
) -> Result<Eclipse, String> {
    let aura = fight.player_aura(aura)?;
    let starfire = fight.spells_with_class(&["starfire"]);
    let cast_time_mod =
        fight.register_mod(ModKind::CastTimeFlat, 0.0, cast_time_reduction, starfire);
    Ok(Eclipse {
        aura,
        charges_per_wrath,
        cast_time_mod,
    })
}

impl Eclipse {
    /// The permanent trigger aura's OnCastComplete.
    pub(crate) fn on_cast_complete<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId) {
        match fight.spells[spell].class_spell.as_deref() {
            Some("wrath") => {
                fight.activate_aura(self.aura);
                let stacks = fight.aura(self.aura).stacks + self.charges_per_wrath;
                fight.set_stacks(self.aura, stacks);
            }
            Some("starfire") if fight.aura(self.aura).active => fight.remove_stack(self.aura),
            _ => {}
        }
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cast_time_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.deactivate_mod(self.cast_time_mod);
    }
}
