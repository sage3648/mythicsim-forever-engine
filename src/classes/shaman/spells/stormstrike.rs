//! Stormstrike, from Go sim/shaman/stormstrike.go: the target debuff raises this shaman's
//! Lightning Bolt, Chain Lightning, Earth Shock and overload damage through a damage done by
//! caster multiplier, and each of their landed, damaging hits spends a charge.

use crate::{
    classes::shaman::masks::{is_class, STORMSTRIKE_SPELLS},
    core::fight::{melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId, SpellResult},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct Stormstrike {
    pub(crate) debuff: AuraRef,
    damage_multiplier: f64,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    debuff: &str,
    damage_multiplier: f64,
) -> Result<Stormstrike, String> {
    let index = fight.trackers[crate::core::fight::Side::Target.index()]
        .find(debuff)
        .ok_or_else(|| format!("target aura {debuff} is not registered"))?;
    Ok(Stormstrike {
        debuff: AuraRef {
            side: crate::core::fight::Side::Target,
            index,
        },
        damage_multiplier,
    })
}

impl Stormstrike {
    fn boosts<A: Agent>(fight: &Fight<A>, spell: SpellId) -> bool {
        is_class(
            fight.spells[spell].class_spell.as_deref(),
            STORMSTRIKE_SPELLS,
        )
    }

    /// The debuff's `AttachDDBC` handler while it is active on the target the hit lands on.
    pub(crate) fn caster_multiplier<A: Agent>(
        &self,
        fight: &Fight<A>,
        spell: SpellId,
        target: Side,
    ) -> Option<f64> {
        fight
            .aura(fight.aura_on(self.debuff, target))
            .active
            .then(|| {
                if Self::boosts(fight, spell) {
                    self.damage_multiplier
                } else {
                    1.0
                }
            })
    }

    /// The OnSpellHitTaken of the debuff on a target, `aura`.
    pub(crate) fn on_spell_hit_taken<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        aura: AuraRef,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !Self::boosts(fight, spell) || !result.landed() || result.damage == 0.0 {
            return;
        }
        fight.remove_stack(aura);
    }

    /// The cast's ApplyEffects: an outcome without a hit counter; when it lands, every charge
    /// of the debuff and, with a main hand weapon, the main hand strike.
    pub(crate) fn cast<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        target: Side,
        main_hand: Option<SpellId>,
    ) {
        let result = fight.calc_physical_outcome(
            spell,
            target,
            PhysicalOutcome::MeleeSpecialHit { count: false },
        );
        fight.deal_damage(spell, result, false);
        if result.landed() {
            let debuff = fight.aura_on(self.debuff, target);
            fight.activate_aura(debuff);
            let max = fight.aura(debuff).max_stacks;
            fight.set_stacks(debuff, max);
            if let Some(strike) = main_hand {
                fight.cast(strike, target);
            }
        }
    }
}

/// A strike's ApplyEffects: the hand's weapon roll on the special table, blocks included.
pub(crate) fn strike<A: Agent>(
    fight: &mut Fight<A>,
    spell: SpellId,
    target: Side,
    main_hand: bool,
) {
    let attack_power = fight.melee_attack_power();
    let base = if main_hand {
        fight.mh_weapon_damage(attack_power)
    } else {
        fight.oh_weapon_damage(attack_power)
    };
    let result = fight.calc_physical_damage(
        spell,
        target,
        base,
        PhysicalOutcome::MeleeSpecialBlockAndCrit { count: true },
    );
    fight.deal_damage(spell, result, false);
}
