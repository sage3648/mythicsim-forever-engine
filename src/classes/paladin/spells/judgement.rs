//! Judgement, from Go sim/paladin/judgement.go, and the seals' judgements from
//! seal_of_command.go and seal_of_righteousness.go.

use crate::core::fight::{
    melee::PhysicalOutcome, Action, Fight, Outcome, Side, SpellId, PRIORITY_GCD,
};

use super::{super::agent::PaladinAgent, seals};

/// Judgement's `ApplyEffects`: the active seal's judgement, then a rotation wake a batch
/// window after the cooldown ends, since nothing on the GCD marks it.
pub(crate) fn apply(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side, wake_delay: i64) {
    let seal = seals::active_seal(fight).expect("Judgement needs an active seal");
    let judgement = fight.agent.seals.seals[seal].judgement;
    fight.cast(judgement, target);
    let at = fight.now + fight.spell_time_to_ready(spell) + wake_delay;
    fight.schedule(at, PRIORITY_GCD, Action::React);
}

/// Judgement of Command: half the rolled damage against a target that is not stunned, on
/// the melee table without dodge, parry or block.
pub(crate) fn command(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side) {
    let mut base = fight.roll_damage_effect(spell);
    base /= 2.0;
    deal(fight, spell, target, base);
}

/// Judgement of Righteousness: the rolled damage, binary, on the same table.
pub(crate) fn righteousness(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side) {
    let base = fight.roll_damage_effect(spell);
    deal(fight, spell, target, base);
}

fn deal(fight: &mut Fight<PaladinAgent>, spell: SpellId, target: Side, base: f64) {
    let result = fight.calc_damage_with(
        spell,
        target,
        base,
        Outcome::Table(PhysicalOutcome::MeleeSpecialNoBlockDodgeParry { count: true }),
    );
    fight.deal_damage(spell, result, false);
}
