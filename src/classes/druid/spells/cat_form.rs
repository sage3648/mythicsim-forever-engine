//! Cat Form (768), from Go sim/druid/forms.go. The aura sets the cat's threat, spirit
//! regeneration, stats, paw weapon, movement speed and free, faster Faerie Fire; leaving it
//! restores them and remembers the energy left behind. The cast leaves and re-enters the form,
//! setting energy to Furor's carry over: a share of the energy left behind plus a tenth of
//! the cap for each second out of form, at most the cap. The carry over state is never reset,
//! as in Go, so a fight's first shift reads the previous fight's exit.

use crate::{
    contracts::prepared_v2::{ActionId, Weapon},
    core::fight::{AuraRef, Fight, ModId, ModKind, ResourceKind, SpellId},
};

use super::super::{
    agent::DruidAgent,
    forms::{self, BEAR, CAT, HUMANOID},
};

#[derive(Clone, Debug)]
pub(crate) struct CatForm {
    pub(crate) aura: AuraRef,
    stat_bit: u32,
    pub(crate) initial_threat_multiplier: f64,
    threat_multiplier: f64,
    pub(crate) initial_spirit_regen_multiplier: f64,
    spirit_regen_multiplier: f64,
    pub(crate) initial_movement_speed_multiplier: f64,
    movement_speed_bonus: f64,
    furor_max: f64,
    energy_metrics: usize,
    mods: [ModId; 2],
    breaking: Vec<bool>,
    main_hand: Weapon,
    cat_weapon: Weapon,
}

/// The exported parameters.
pub(crate) struct Params<'a> {
    pub(crate) spell: SpellId,
    pub(crate) spell_id: i32,
    pub(crate) aura: &'a str,
    pub(crate) stat_bit: u32,
    pub(crate) initial_threat_multiplier: f64,
    pub(crate) threat_multiplier: f64,
    pub(crate) initial_spirit_regen_multiplier: f64,
    pub(crate) spirit_regen_multiplier: f64,
    pub(crate) initial_movement_speed_multiplier: f64,
    pub(crate) movement_speed_bonus: f64,
    pub(crate) furor_max: f64,
    pub(crate) cost_spells: &'a [usize],
    pub(crate) gcd_spells: &'a [usize],
    pub(crate) gcd_delta: i64,
    pub(crate) form_breaking_spells: &'a [usize],
    pub(crate) main_hand: &'a Weapon,
    pub(crate) cat_weapon: &'a Weapon,
}

/// Cat Form (Passive): Faerie Fire costs nothing and its global cooldown is shorter in the
/// form. The prepared spells carry both, so binding takes them back out and the aura's
/// modifiers put them in while it is up.
pub(crate) fn bind(fight: &mut Fight<DruidAgent>, params: Params) -> Result<CatForm, String> {
    let aura = fight.player_aura(params.aura)?;
    // Go activates the form in the agent's reset, after the permanent auras.
    fight.set_aura_permanent(aura, false);
    let id = ActionId {
        spell_id: params.spell_id,
        ..ActionId::default()
    };
    let energy_metrics =
        fight.new_resource_metrics_before_cost(params.spell, id, ResourceKind::Energy);
    let len = fight.spells.len();
    let costed: Vec<SpellId> = params
        .cost_spells
        .iter()
        .copied()
        .filter(|&spell| spell < len && fight.spells[spell].cost.is_some())
        .collect();
    for &spell in &costed {
        if let Some(cost) = fight.spells[spell].cost.as_mut() {
            cost.additive_percent_modifier -= -1.0;
        }
    }
    let timed: Vec<SpellId> = params
        .gcd_spells
        .iter()
        .copied()
        .filter(|&spell| spell < len)
        .collect();
    for &spell in &timed {
        fight.spells[spell].default_cast.gcd -= params.gcd_delta;
    }
    let cost_mod = fight.register_mod(ModKind::PowerCostPercentAdd, -1.0, 0, costed);
    let gcd_mod = fight.register_mod(ModKind::GlobalCooldownFlat, 0.0, params.gcd_delta, timed);
    let mut breaking = vec![false; len];
    for &spell in params.form_breaking_spells {
        if let Some(slot) = breaking.get_mut(spell) {
            *slot = true;
        }
    }
    Ok(CatForm {
        aura,
        stat_bit: params.stat_bit,
        initial_threat_multiplier: params.initial_threat_multiplier,
        threat_multiplier: params.threat_multiplier,
        initial_spirit_regen_multiplier: params.initial_spirit_regen_multiplier,
        spirit_regen_multiplier: params.spirit_regen_multiplier,
        initial_movement_speed_multiplier: params.initial_movement_speed_multiplier,
        movement_speed_bonus: params.movement_speed_bonus,
        furor_max: params.furor_max,
        energy_metrics,
        mods: [cost_mod, gcd_mod],
        breaking,
        main_hand: params.main_hand.clone(),
        cat_weapon: params.cat_weapon.clone(),
    })
}

impl CatForm {
    /// The passive movement speed effect, which Go activates before the gain.
    pub(crate) fn on_exclusive_gain(&self, fight: &mut Fight<DruidAgent>) {
        forms::multiply_movement_speed(fight, 1.0 + self.movement_speed_bonus);
    }

    /// `OnGain`, then the attached Faerie Fire modifiers.
    pub(crate) fn on_gain(&self, fight: &mut Fight<DruidAgent>) {
        fight.agent.form = CAT;
        fight.player.threat_multiplier *= self.threat_multiplier;
        fight.multiply_spirit_regen_multiplier(self.spirit_regen_multiplier);
        fight.set_stat_mask(fight.stat_mask | self.stat_bit);
        fight.set_main_hand(self.cat_weapon.clone());
        fight.enable_auto_swing();
        fight.update_mana_regen_rates();
        for &id in &self.mods {
            fight.activate_mod(id);
        }
    }

    /// The movement speed effect ends first, then `OnExpire`, the attached modifiers and
    /// Prowl's expiry hook.
    pub(crate) fn on_expire(&self, fight: &mut Fight<DruidAgent>) {
        forms::multiply_movement_speed(fight, 1.0 / (1.0 + self.movement_speed_bonus));
        fight.agent.form = HUMANOID;
        fight.player.threat_multiplier /= self.threat_multiplier;
        fight.divide_spirit_regen_multiplier(self.spirit_regen_multiplier);
        fight.set_stat_mask(fight.stat_mask & !self.stat_bit);
        fight.agent.last_cat_form_energy = fight.energy_bar().current;
        fight.agent.last_cat_form_exit = fight.now;
        fight.set_main_hand(self.main_hand.clone());
        fight.enable_auto_swing();
        fight.update_mana_regen_rates();
        for &id in &self.mods {
            fight.deactivate_mod(id);
        }
        if let Some(prowl) = fight.agent.prowl.clone() {
            if fight.aura(prowl.aura).active {
                fight.deactivate_aura(prowl.aura);
            }
        }
    }

    /// Go `ClearForm`.
    pub(crate) fn clear_form(&self, fight: &mut Fight<DruidAgent>) {
        if fight.agent.form & CAT != 0 {
            fight.deactivate_aura(self.aura);
        }
        fight.agent.form = HUMANOID;
    }

    /// The cast's `ApplyEffects`.
    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>) {
        if fight.aura(self.aura).active {
            fight.deactivate_aura(self.aura);
        }
        if fight.now > 0 {
            let target = self.furor_shift_energy(fight);
            let delta = target - fight.energy_bar().current;
            if delta > 0.0 {
                fight.add_energy(delta, self.energy_metrics);
            } else if delta < 0.0 {
                fight.spend_energy(-delta, self.energy_metrics);
            }
        }
        fight.activate_aura(self.aura);
    }

    /// Go `furorShiftEnergy`.
    fn furor_shift_energy(&self, fight: &Fight<DruidAgent>) -> f64 {
        if self.furor_max == 0.0 {
            return 0.0;
        }
        let m2 = self.furor_max;
        let carry_over = fight.agent.last_cat_form_energy * m2 / 100.0;
        let mut out_of_form = 0.0;
        if fight.agent.last_cat_form_exit > 0 {
            out_of_form =
                m2 / 10.0 * crate::core::time::seconds(fight.now - fight.agent.last_cat_form_exit);
        }
        m2.min(carry_over + out_of_form)
    }

    /// The automatic use of a form-breaking cooldown waits for a caster form.
    pub(crate) fn activation_allowed(&self, fight: &Fight<DruidAgent>, spell: SpellId) -> bool {
        !self.breaking[spell] || fight.agent.form & (BEAR | CAT) == 0
    }

    /// The form-breaking wrapper around `ApplyEffects`.
    pub(crate) fn after_apply_effects(&self, fight: &mut Fight<DruidAgent>, spell: SpellId) {
        if self.breaking[spell] && fight.agent.form & (BEAR | CAT) != 0 {
            self.clear_form(fight);
        }
    }
}
