//! Poisons, from Go sim/rogue/poisons.go. Each imbue is a weapon proc on the hand it is on: a
//! landed hit from that hand rolls the poison's chance inside the handler and casts the poison
//! at once. Instant Poison deals a rolled Nature hit; Deadly Poison lands on the magic hit
//! table and stacks a dot that snapshots its tick at the stacks it has. Wound Poison lands on
//! the magic hit table and stacks its healing debuff, which has no effect in scope. Venom adds
//! to every poison's chance while it is up.

use crate::core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId, SpellResult};

/// A poison's weapon proc.
#[derive(Clone, Debug)]
pub(crate) struct PoisonProc {
    pub(crate) label: String,
    pub(crate) spell: SpellId,
    pub(crate) chance: f64,
    /// Whether each spell, by spellbook position, is a hit from the imbued hand that a weapon
    /// proc hears: one without Suppress Weapon Procs whose proc mask names that hand.
    pub(crate) eligible: Vec<bool>,
}

impl PoisonProc {
    /// Go `AttachProcTriggerCallback` for the poison: a landed hit from the imbued hand, then
    /// the handler's chance roll and the cast.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
        additive_bonus: f64,
    ) {
        if !self.eligible[spell] || !result.landed() {
            return;
        }
        // Go poisonProcChance: the poison's own chance, Improved Poisons, then Venom's share.
        if fight.proc(self.chance + additive_bonus, &self.label) {
            fight.cast(self.spell, result.target);
        }
    }
}

/// Instant Poison's effect: a rolled hit on the magic hit and crit table.
pub(crate) fn instant_poison<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    min: f64,
    max: f64,
) {
    let damage = fight.go_roll(min, max);
    let result = fight.calc_damage(spell, target, damage);
    fight.deal_damage(spell, result, false);
}

/// Deadly Poison's effect: a magic hit roll, then the dot refreshes and gains a stack, or
/// starts at one stack, and snapshots its tick at the stacks it now has.
pub(crate) fn deadly_poison<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    tick_damage: f64,
) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHit);
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        return;
    }
    let dot = fight.spells[spell].dot.expect("Deadly Poison has a dot");
    let aura = fight.dots[dot].aura;
    if fight.aura(aura).active {
        fight.refresh_aura(aura);
        if fight.aura(aura).stacks < fight.aura(aura).max_stacks {
            fight.add_stack(aura);
        }
    } else {
        fight.apply_dot(dot);
        fight.set_stacks(aura, 1);
    }
    let stacks = fight.aura(aura).stacks;
    fight.snapshot_dot(dot, tick_damage * f64::from(stacks));
}

/// Wound Poison's effect: a magic hit roll; a landed one starts the debuff at one stack, or
/// refreshes it and adds a stack.
pub(crate) fn wound_poison<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    debuff: AuraRef,
) {
    let result = fight.calc_outcome(spell, target, Outcome::MagicHit);
    fight.deal_damage(spell, result, false);
    if !result.landed() {
        return;
    }
    if !fight.aura(debuff).active {
        fight.activate_aura(debuff);
        fight.set_stacks(debuff, 1);
        return;
    }
    fight.refresh_aura(debuff);
    if fight.aura(debuff).stacks < fight.aura(debuff).max_stacks {
        fight.add_stack(debuff);
    }
}
