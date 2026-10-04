//! Windfury Weapon, from Go sim/shaman/weapon_imbues.go `RegisterWindfuryImbue`: a weapon proc
//! with its own cooldown on landed hits grants charges of attack power and two extra attacks of
//! the hand that procced it. Landed autos spend the charges one spell batch window later.

use crate::{
    contracts::prepared_v2::SpellChance,
    core::fight::{Agent, AuraRef, Fight, SpellId, SpellResult},
};

#[derive(Clone, Debug)]
pub(crate) struct WindfuryWeapon {
    pub(crate) trigger: AuraRef,
    pub(crate) ap_aura: AuraRef,
    /// The attack power aura's bit among the stat auras.
    bit: u32,
    extra_spell: SpellId,
    off_hand_spell: Option<SpellId>,
    triggers: Vec<bool>,
    main_hand: Vec<bool>,
    chances: Vec<Option<f64>>,
    spend: Vec<bool>,
    gain_log: String,
    expire_log: String,
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
    ap_aura: &str,
    extra_spell: usize,
    off_hand_spell: i64,
    triggers: &[usize],
    main_hand: &[usize],
    chances: &[SpellChance],
    spend: &[usize],
    logs: (&str, &str),
    blocks_windfury_totem: bool,
) -> Result<WindfuryWeapon, String> {
    let count = fight.spells.len();
    let bit = fight
        .stat_aura_labels
        .iter()
        .position(|label| label == ap_aura)
        .map(|bit| 1u32 << bit)
        .ok_or_else(|| format!("{ap_aura} is not a stat aura"))?;
    let mut by_spell = vec![None; count];
    for chance in chances {
        *by_spell
            .get_mut(chance.spell)
            .ok_or_else(|| format!("Windfury Weapon names spell {}", chance.spell))? =
            Some(chance.chance);
    }
    if extra_spell >= count {
        return Err(format!("Windfury Weapon names spell {extra_spell}"));
    }
    Ok(WindfuryWeapon {
        trigger: fight.player_aura(trigger)?,
        ap_aura: fight.player_aura(ap_aura)?,
        bit,
        extra_spell,
        off_hand_spell: usize::try_from(off_hand_spell)
            .ok()
            .filter(|&spell| spell < count),
        triggers: mask(count, triggers)?,
        main_hand: mask(count, main_hand)?,
        chances: by_spell,
        spend: mask(count, spend)?,
        gain_log: logs.0.to_string(),
        expire_log: logs.1.to_string(),
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

    /// The trigger: landed hits it hears, its cooldown, then the proc manager's roll.
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
        fight.activate_aura(self.ap_aura);
        let max = fight.aura(self.ap_aura).max_stacks;
        fight.set_stacks(self.ap_aura, max);
        if self.main_hand[spell] {
            fight.extra_mh_attacks_from(2, self.extra_spell);
        } else if let Some(off_hand) = self.off_hand_spell {
            fight.cast(off_hand, result.target);
            fight.cast(off_hand, result.target);
        }
    }

    /// The attack power aura's gain: the stats aura's line, then its stats.
    pub(crate) fn on_ap_gain<A: Agent>(&self, fight: &mut Fight<A>) {
        if fight.log.is_some() {
            let line = self.gain_log.clone();
            fight.player_log(&line);
        }
        fight.set_stat_aura(self.bit, true);
    }

    pub(crate) fn on_ap_expire<A: Agent>(&self, fight: &mut Fight<A>) {
        if fight.log.is_some() {
            let line = self.expire_log.clone();
            fight.player_log(&line);
        }
        fight.set_stat_aura(self.bit, false);
    }

    /// The charges' trigger on the attack power aura: landed autos schedule a spend.
    pub(crate) fn on_spend_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if self.spend[spell] && result.landed() {
            fight.schedule_delayed_proc(self.ap_aura, spell, *result);
        }
    }

    /// The delayed spend: a charge, and the aura once none are left.
    pub(crate) fn spend<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.remove_stack(self.ap_aura);
        if fight.aura(self.ap_aura).stacks == 0 {
            fight.deactivate_aura(self.ap_aura);
        }
    }
}
