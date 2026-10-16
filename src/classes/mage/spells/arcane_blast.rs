//! Arcane Blast (1239700) and its Arcane Blast buff (400573), from Go sim/mage/arcane_blast.go
//! and arcane_charge.go, as client 70425 redesigned them. Each cast stacks the buff up to four
//! times, from its first stack for 15 sec: a stack does not refresh it and nothing spends it.
//! Each stack gives the mage 15% of its Intellect as spell damage, which the `stat_auras`
//! combinations hold, and raises the cost of Arcane Blast itself.

use crate::core::fight::{Agent, AuraRef, Fight, ModId, ModKind, Side, SpellId};

#[derive(Clone, Debug)]
pub(crate) struct ArcaneCharges {
    pub(crate) aura: AuraRef,
    intellect_per_stack: f64,
    cost_per_stack: f64,
    /// The Intellect an iteration's end leaves, which the stacks' last log line reads.
    done_intellect: f64,
    cost_mod: ModId,
    /// The stacks' place in the stat aura combinations: offset and width.
    stat_place: (u32, u32),
}

pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    aura: &str,
    (intellect_per_stack, cost_per_stack, done_intellect): (f64, f64, f64),
    stat_place: (u32, u32),
) -> Result<ArcaneCharges, String> {
    let aura = fight.player_aura(aura)?;
    let blasts = fight.spells_with_class(&["arcane_blast"]);
    let cost_mod = fight.register_mod(ModKind::PowerCostPercentAdd, 0.0, 0, blasts);
    Ok(ArcaneCharges {
        aura,
        intellect_per_stack,
        cost_per_stack,
        done_intellect,
        cost_mod,
        stat_place,
    })
}

/// Go Arcane Blast `ApplyEffects`: the hit lands at once, then the buff starts if it is down and
/// gains a stack.
pub(crate) fn apply<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    charges: &ArcaneCharges,
) {
    let base = fight.roll_damage_effect(spell);
    let result = fight.calc_damage(spell, target, base);
    fight.deal_damage(spell, result, false);
    if !fight.aura(charges.aura).active {
        fight.activate_aura(charges.aura);
    }
    fight.add_stack(charges.aura);
}

impl ArcaneCharges {
    /// Go `UpdateDynamicStatDep`: the stats of the new stack count, and its log line.
    fn set_stat_stacks<A: Agent>(&self, fight: &mut Fight<A>, stacks: i32) {
        let before = fight.player.powers.spell_damage;
        let (offset, width) = self.stat_place;
        let field = ((1u32 << width) - 1) << offset;
        let old_stacks = (fight.stat_mask & field) >> offset;
        fight.set_stat_mask((fight.stat_mask & !field) | ((stacks as u32) << offset));
        if fight.log.is_some() {
            // At an iteration's end Go's stats have lost the raid buffs that fell first.
            let change = if fight.done_iteration {
                (self.intellect_per_stack * f64::from(stacks)
                    - self.intellect_per_stack * f64::from(old_stacks))
                    * self.done_intellect
            } else {
                fight.player.powers.spell_damage - before
            };
            let change = if change == 0.0 {
                "{}".to_string()
            } else {
                format!("{{\"SpellDamage\": {change:.3},}}")
            };
            let amount = self.intellect_per_stack * f64::from(stacks);
            fight.player_log(&format!(
                "Dynamic dep updated (SpellDamage += Intellect * {amount:.2}): {change}"
            ));
        }
    }

    pub(crate) fn on_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.activate_mod(self.cost_mod);
    }

    pub(crate) fn on_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        self.set_stat_stacks(fight, 0);
        fight.deactivate_mod(self.cost_mod);
    }

    pub(crate) fn on_stacks_change<A: Agent>(&self, fight: &mut Fight<A>, stacks: i32) {
        self.set_stat_stacks(fight, stacks);
        fight.update_mod_value(self.cost_mod, self.cost_per_stack * f64::from(stacks));
    }
}
