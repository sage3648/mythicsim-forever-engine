//! Mind Blast (8092 to 10947) and Shadow Word: Death (1309595 to 1309636), from Go
//! sim/priest/mind_blast.go and shadow_word_death.go: an instant hit and crit roll on the
//! rank's own client row, dealt at once.

use crate::core::fight::{Agent, Fight, Side, SpellId};

/// `CalcAndDealDamage` with `OutcomeMagicHitAndCrit`.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
}

/// Shadow Word: Death: Early Demise adds its crit for this roll inside the 20% execute
/// phase. Go adds and subtracts the bonus around the roll, so the field keeps any rounding.
pub(crate) fn apply_shadow_word_death<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    early_demise_crit: f64,
) {
    let bonus = if early_demise_crit > 0.0 && fight.is_execute_phase_20() {
        early_demise_crit
    } else {
        0.0
    };
    fight.spells[spell].bonus_crit_percent += bonus;
    apply(fight, spell, target);
    fight.spells[spell].bonus_crit_percent -= bonus;
}
