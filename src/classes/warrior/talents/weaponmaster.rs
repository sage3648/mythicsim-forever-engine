//! Weaponmaster with a sword (1290261), from Go sim/warrior/talents_arms.go
//! `registerWeaponmaster`: a landed melee hit of a sword hand, other than a proc's or the
//! extra attack's own, may cast an extra main hand attack at once, which a queued Heroic
//! Strike replaces.
//!
//! Under Forever's rules, from Forever combat logs with a two-hand sword at 5/5: the party
//! Windfury Totem's extra swing never rolls it (its procs gave 0 of 6 extra swings), Sweeping
//! Strikes' hits never roll it (0 of 336, `excluded_class_spells`), and Whirlwind and Cleave roll
//! it once a cast (Whirlwind 7 procs on 100 casts and 283 landed hits), which the agent keeps.

use crate::core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult};

#[derive(Clone, Copy, Debug)]
pub(crate) struct WeaponmasterSword {
    pub(crate) trigger: AuraRef,
    pub(crate) proc_chance: f64,
    pub(crate) extra_attack: SpellId,
    /// Forever's rules: the party Windfury Totem's extra swing does not roll it.
    pub(crate) skips_windfury_totem_swing: bool,
}

/// The trigger's OnSpellHitDealt. `sword` is whether the spell's proc mask holds a bit of a
/// hand that wields a sword. Whether the hit took the roll, whatever it rolled.
pub(crate) fn on_hit<A: Agent>(
    fight: &mut Fight<A>,
    params: WeaponmasterSword,
    spell: SpellId,
    sword: bool,
    result: &SpellResult,
) -> bool {
    let state = &fight.spells[spell];
    if state.flags.proc || !state.melee_proc || !result.landed() {
        return false;
    }
    if !sword || spell == params.extra_attack {
        return false;
    }
    let totem_swing = fight.windfury.as_ref().and_then(|windfury| windfury.extra);
    if params.skips_windfury_totem_swing && totem_swing == Some(spell) {
        return false;
    }
    if params.proc_chance != 1.0 && fight.random_for_aura(params.trigger) > params.proc_chance {
        return true;
    }
    let mut attack = params.extra_attack;
    if fight.config.melee.replace_main_hand_swing {
        attack = A::replace_mh_swing(fight, attack);
        fight.note_extra_attack_strike(params.extra_attack, attack);
    }
    fight.cast(attack, result.target);
    true
}
