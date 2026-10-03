//! Cold Snap (12472), from Go sim/mage/cold_snap.go: resets the cooldown of every Frost
//! spell that has one. It is cast off the global cooldown by the major cooldown manager.

use crate::core::{
    fight::{Agent, Fight},
    time::STARTING_CD_TIME,
};

pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>) {
    for spell in 0..fight.spells.len() {
        if fight.spells[spell].school & 16 == 0 {
            continue;
        }
        if let Some((timer, _)) = fight.spells[spell].cd {
            fight.timers[timer] = STARTING_CD_TIME;
        }
    }
}
