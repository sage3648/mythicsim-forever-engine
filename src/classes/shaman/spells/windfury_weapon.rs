//! Windfury Weapon, from Go sim/shaman/weapon_imbues.go `RegisterWindfuryImbue`: a weapon proc
//! with its own cooldown on landed hits strikes twice, with two special hits (439440 for the
//! main hand, 439441 for the off hand) of the hand that procced it. Each hit is that weapon's
//! damage with the rank's extra attack power, rolled on the special attack table. The strikes
//! leave the swing timer alone and grant no attack power aura.

use crate::{
    contracts::prepared_v2::SpellChance,
    core::fight::{melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId, SpellResult},
};

#[derive(Clone, Debug)]
pub(crate) struct WindfuryWeapon {
    pub(crate) trigger: AuraRef,
    main_hand_attack: SpellId,
    off_hand_attack: SpellId,
    attack_power: f64,
    triggers: Vec<bool>,
    main_hand: Vec<bool>,
    chances: Vec<Option<f64>>,
    pub(crate) blocks_windfury_totem: bool,
}

/// Spellbook positions as a mask.
fn mask(count: usize, spells: &[usize]) -> Result<Vec<bool>, String> {
    let mut mask = vec![false; count];
    for &spell in spells {
        *mask
            .get_mut(spell)
            .ok_or_else(|| format!("Windfury Weapon names spell {spell}"))? = true;
    }
    Ok(mask)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    trigger: &str,
    attacks: (usize, usize),
    attack_power: f64,
    triggers: &[usize],
    main_hand: &[usize],
    chances: &[SpellChance],
    blocks_windfury_totem: bool,
) -> Result<WindfuryWeapon, String> {
    let count = fight.spells.len();
    let mut by_spell = vec![None; count];
    for chance in chances {
        *by_spell
            .get_mut(chance.spell)
            .ok_or_else(|| format!("Windfury Weapon names spell {}", chance.spell))? =
            Some(chance.chance);
    }
    for attack in [attacks.0, attacks.1] {
        if attack >= count {
            return Err(format!("Windfury Weapon names spell {attack}"));
        }
    }
    Ok(WindfuryWeapon {
        trigger: fight.player_aura(trigger)?,
        main_hand_attack: attacks.0,
        off_hand_attack: attacks.1,
        attack_power,
        triggers: mask(count, triggers)?,
        main_hand: mask(count, main_hand)?,
        chances: by_spell,
        blocks_windfury_totem,
    })
}

impl WindfuryWeapon {
    /// The trigger aura's gain: a main hand imbue's exclusive effect outbids the party totem's,
    /// whose expiry turns its trigger off and takes the totem down.
    pub(crate) fn on_trigger_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        if !self.blocks_windfury_totem {
            return;
        }
        let Some(windfury) = fight.windfury.as_mut() else {
            return;
        };
        windfury.blocked = true;
        let (trigger, totem) = (windfury.trigger, windfury.totem);
        if fight.aura(trigger).active {
            fight.deactivate_aura(trigger);
            fight.deactivate_aura(totem);
        }
    }

    pub(crate) fn on_trigger_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        if let Some(windfury) = fight
            .windfury
            .as_mut()
            .filter(|_| self.blocks_windfury_totem)
        {
            windfury.blocked = false;
        }
    }

    /// The trigger: landed hits it hears, its cooldown, then the proc manager's roll. The
    /// handler strikes twice with the hand that procced it, at once.
    pub(crate) fn on_trigger_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.triggers[spell] || !result.landed() {
            return;
        }
        let icd = fight.aura(self.trigger).icd;
        if let Some((timer, _)) = icd {
            if fight.timers[timer] > fight.now {
                return;
            }
        }
        let Some(chance) = self.chances[spell] else {
            return;
        };
        if !fight.proc_for_aura(chance, self.trigger) {
            return;
        }
        if let Some((timer, duration)) = icd {
            fight.timers[timer] = fight.now + duration;
        }
        let attack = if self.main_hand[spell] {
            self.main_hand_attack
        } else {
            self.off_hand_attack
        };
        fight.cast(attack, result.target);
        fight.cast(attack, result.target);
    }

    /// `newWindfuryAttackSpell`'s ApplyEffects: the hand's weapon roll with the rank's attack
    /// power added to the spell's own, on the weapon special table.
    pub(crate) fn strike<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        target: Side,
        main_hand: bool,
    ) {
        let attack_power = fight.melee_attack_power() + self.attack_power;
        let base = if main_hand {
            fight.mh_weapon_damage(attack_power)
        } else {
            fight.oh_weapon_damage(attack_power)
        };
        let result = fight.calc_physical_damage(
            spell,
            target,
            base,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
    }
}
