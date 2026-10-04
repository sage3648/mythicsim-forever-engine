//! Dire Bear Form (9634), from Go sim/druid/forms.go. The aura is a stat aura whose bit carries
//! the form's stats, armor and health, which the target's swings read; it sets the bear's threat,
//! spirit regeneration, rage bar and paw, makes Faerie Fire free, and keeps the health fraction
//! across the change of maximum health. Leaving it ends Enrage, Frenzied Regeneration and Maul's
//! queue and restores the rest. The cast spends all Rage, rolls Furor's 10 and enters the form.

use crate::{
    contracts::prepared_v2::{ActionId, Weapon},
    core::fight::{AuraRef, Fight, ModId, ModKind, ResourceKind, SpellId},
};

use super::super::{
    agent::DruidAgent,
    forms::{BEAR, CAT, HUMANOID},
};

#[derive(Clone, Debug)]
pub(crate) struct BearForm {
    pub(crate) aura: AuraRef,
    stat_bit: u32,
    /// The health the form's stat bonus adds to the maximum.
    health_bonus: f64,
    health_metrics: usize,
    rage_metrics: usize,
    pub(crate) initial_threat_multiplier: f64,
    threat_multiplier: f64,
    pub(crate) initial_spirit_regen_multiplier: f64,
    spirit_regen_multiplier: f64,
    furor_proc_chance: f64,
    cost_mod: ModId,
    breaking: Vec<bool>,
    main_hand: Weapon,
    bear_weapon: Weapon,
}

/// The exported parameters.
pub(crate) struct Params<'a> {
    pub(crate) spell: SpellId,
    pub(crate) spell_id: i32,
    pub(crate) aura: &'a str,
    pub(crate) stat_bit: u32,
    pub(crate) health_bonus: f64,
    pub(crate) initial_threat_multiplier: f64,
    pub(crate) threat_multiplier: f64,
    pub(crate) initial_spirit_regen_multiplier: f64,
    pub(crate) spirit_regen_multiplier: f64,
    pub(crate) furor_proc_chance: f64,
    pub(crate) cost_spells: &'a [usize],
    pub(crate) form_breaking_spells: &'a [usize],
    pub(crate) main_hand: &'a Weapon,
    pub(crate) bear_weapon: &'a Weapon,
}

/// Bear Form's attached modifier makes Faerie Fire free. The prepared spells carry it, so
/// binding takes it back out and the aura's modifier puts it in while it is up.
pub(crate) fn bind(fight: &mut Fight<DruidAgent>, params: Params) -> Result<BearForm, String> {
    let aura = fight.player_aura(params.aura)?;
    // Go enters the form in the agent's reset, after the permanent auras.
    fight.set_aura_permanent(aura, false);
    let id = ActionId {
        spell_id: params.spell_id,
        ..ActionId::default()
    };
    // Go registers the aura's health metrics, then the cast's rage metrics, both before the
    // cast's mana cost metrics.
    let health_metrics =
        fight.new_resource_metrics_before_cost(params.spell, id.clone(), ResourceKind::Health);
    let rage_metrics = fight.new_resource_metrics_before_cost(params.spell, id, ResourceKind::Rage);
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
    let cost_mod = fight.register_mod(ModKind::PowerCostPercentAdd, -1.0, 0, costed);
    let mut breaking = vec![false; len];
    for &spell in params.form_breaking_spells {
        if let Some(slot) = breaking.get_mut(spell) {
            *slot = true;
        }
    }
    Ok(BearForm {
        aura,
        stat_bit: params.stat_bit,
        health_bonus: params.health_bonus,
        health_metrics,
        rage_metrics,
        initial_threat_multiplier: params.initial_threat_multiplier,
        threat_multiplier: params.threat_multiplier,
        initial_spirit_regen_multiplier: params.initial_spirit_regen_multiplier,
        spirit_regen_multiplier: params.spirit_regen_multiplier,
        furor_proc_chance: params.furor_proc_chance,
        cost_mod,
        breaking,
        main_hand: params.main_hand.clone(),
        bear_weapon: params.bear_weapon.clone(),
    })
}

impl BearForm {
    /// Go `restoreHealthFraction`: in the fight, health moves to the fraction it held of the
    /// maximum at `before`, now of the new maximum.
    fn restore_health_fraction(fight: &mut Fight<DruidAgent>, before: f64, metrics: usize) {
        if fight.now <= 0 {
            return;
        }
        let health = fight.player.health;
        let fraction = health / before;
        // The arm64 build fuses the multiply into the subtraction.
        let delta = fraction.mul_add(fight.player_max_health(), -health);
        if delta > 1e-6 {
            fight.gain_health(delta, metrics);
        } else if delta < -1e-6 {
            fight.remove_health(-delta);
        }
    }

    /// `OnGain`, then the attached Faerie Fire modifier. The health fraction is taken once the
    /// stat bonus is in, before Heart of the Wild's Stamina.
    pub(crate) fn on_gain(&self, fight: &mut Fight<DruidAgent>) {
        fight.agent.form = BEAR;
        fight.set_rage_bar_in_use(true);
        fight.player.threat_multiplier *= self.threat_multiplier;
        fight.multiply_spirit_regen_multiplier(self.spirit_regen_multiplier);
        let before = fight.player_max_health() + self.health_bonus;
        fight.set_stat_mask(fight.stat_mask | self.stat_bit);
        Self::restore_health_fraction(fight, before, self.health_metrics);
        fight.set_main_hand(self.bear_weapon.clone());
        fight.enable_auto_swing();
        fight.update_mana_regen_rates();
        fight.activate_mod(self.cost_mod);
    }

    /// `OnExpire`, then the attached modifier: the health fraction is taken once the stat bonus
    /// is out; then Enrage, Frenzied Regeneration and Maul's queue end with the form, and the
    /// equipped weapon swings again.
    pub(crate) fn on_expire(&self, fight: &mut Fight<DruidAgent>) {
        fight.agent.form = HUMANOID;
        fight.player.threat_multiplier /= self.threat_multiplier;
        fight.divide_spirit_regen_multiplier(self.spirit_regen_multiplier);
        let before = fight.player_max_health() - self.health_bonus;
        fight.set_stat_mask(fight.stat_mask & !self.stat_bit);
        Self::restore_health_fraction(fight, before, self.health_metrics);
        let enrage = fight.agent.enrage.map(|enrage| enrage.aura);
        let queue = fight.agent.maul.map(|maul| maul.queue_aura);
        for aura in [enrage, fight.agent.frenzied_regeneration, queue]
            .into_iter()
            .flatten()
        {
            fight.deactivate_aura(aura);
        }
        fight.set_main_hand(self.main_hand.clone());
        fight.enable_auto_swing();
        fight.update_mana_regen_rates();
        fight.deactivate_mod(self.cost_mod);
    }

    /// Go `ClearForm` from Bear Form: the rage bar stops being the current power bar.
    pub(crate) fn clear_form(&self, fight: &mut Fight<DruidAgent>) {
        if fight.agent.form & BEAR != 0 {
            fight.deactivate_aura(self.aura);
        }
        fight.agent.form = HUMANOID;
        fight.set_rage_bar_in_use(false);
    }

    /// The cast's `ApplyEffects`: shifting into Bear Form resets Rage, then Furor may give 10.
    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>) {
        let rage = fight.current_rage();
        if rage > 0.0 {
            fight.spend_rage(rage, self.rage_metrics);
        }
        if fight.proc(self.furor_proc_chance, "Furor") {
            fight.add_rage(10.0, self.rage_metrics);
        }
        fight.activate_aura(self.aura);
    }

    /// The automatic use of a form-breaking cooldown waits for a caster form.
    pub(crate) fn activation_allowed(&self, fight: &Fight<DruidAgent>, spell: SpellId) -> bool {
        !self.breaking[spell] || fight.agent.form & (BEAR | CAT) == 0
    }

    /// The form-breaking wrapper around `ApplyEffects`.
    pub(crate) fn after_apply_effects(&self, fight: &mut Fight<DruidAgent>, spell: SpellId) {
        if self.breaking[spell] && fight.agent.form & BEAR != 0 {
            self.clear_form(fight);
        }
    }
}
