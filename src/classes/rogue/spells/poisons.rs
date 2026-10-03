//! Poisons, from Go sim/rogue/poisons.go. Each imbue is a weapon proc on the hand it is on: a
//! landed hit from that hand rolls the poison's chance inside the handler and casts the poison
//! at once. Instant Poison deals a rolled Nature hit; Deadly Poison lands on the magic hit
//! table and stacks a dot that snapshots its tick at the stacks it has.

use crate::core::fight::{Agent, Fight, Outcome, Side, SpellId, SpellResult};

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
    ) {
        if !self.eligible[spell] || !result.landed() {
            return;
        }
        if fight.proc(self.chance, &self.label) {
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
    let damage = min + (max - min) * fight.random("Damage Roll");
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
