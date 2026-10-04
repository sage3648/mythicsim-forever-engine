//! Kidney Shot, from Go sim/rogue/kidney_shot.go: a finisher whose metrics split by the combo
//! points spent. It rolls the special hit table without a crit; a landed hit stuns the target,
//! unless it is stun immune, then applies the finisher. The stun lasts a second and a second a
//! combo point, scaled by the target's stun duration multiplier, and while it lasts Improved
//! Kidney Shot raises the rogue's damage on the target through the attack table. The stun
//! pauses the swings of a target that tanks the player, from Go incapacitate.go: no swing lands
//! while it lasts, and a stun that ends early gives the swing back its time.

use crate::core::fight::{melee::PhysicalOutcome, Agent, AuraRef, Fight, Side, SpellId};

use super::finisher::Finisher;

#[derive(Clone, Copy, Debug)]
pub(crate) struct KidneyShot {
    /// The stun aura, absent when the target is stun immune.
    pub(crate) stun: Option<AuraRef>,
    pub(crate) base_duration: i64,
    pub(crate) duration_per_combo_point: i64,
    pub(crate) stun_duration_multiplier: f64,
    pub(crate) damage_taken_multiplier: f64,
}

impl KidneyShot {
    /// Kidney Shot's effect; Stealth has already broken.
    pub(crate) fn apply<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        target: Side,
        finisher: &Finisher,
    ) {
        let result = fight.calc_physical_outcome(
            spell,
            target,
            PhysicalOutcome::MeleeSpecialHit { count: true },
        );
        if result.landed() {
            if let Some(stun) = self.stun {
                let points = i64::from(fight.energy_bar().combo_points);
                // Go's stun aura reads its length in OnGain; a refresh keeps the last one.
                if !fight.aura(stun).active {
                    let base = self.base_duration + self.duration_per_combo_point * points;
                    fight.aura_mut(stun).duration =
                        (base as f64 * self.stun_duration_multiplier) as i64;
                }
                fight.activate_aura(stun);
            }
            finisher.apply(fight, spell);
        } else {
            fight.issue_refund(spell);
        }
        fight.deal_damage(spell, result, false);
    }

    /// The stun's `OnGain` pause of the target's swing: the swing time before the pause and
    /// when the pause ends.
    pub(crate) fn pause_target<A: Agent>(&self, fight: &mut Fight<A>) -> (i64, i64) {
        let stun = self.stun.expect("the stun is bound");
        let duration = fight.aura(stun).duration;
        let swing_at = fight.autos.enemy.swing_at;
        let paused_until = fight.now + duration;
        fight.pause_enemy_melee_by(duration);
        (swing_at, paused_until)
    }

    /// The stun's `OnExpire`: a stun that ends before the pause does gives the target's swing
    /// back its time.
    pub(crate) fn resume_target<A: Agent>(&self, fight: &mut Fight<A>, pause: (i64, i64)) {
        let (swing_at, paused_until) = pause;
        if fight.now < paused_until {
            fight.resume_enemy_melee_at(swing_at);
        }
    }

    /// The stun's OnGain and OnExpire with Improved Kidney Shot.
    pub(crate) fn on_stun_change<A: Agent>(&self, fight: &mut Fight<A>, gained: bool) {
        if self.damage_taken_multiplier == 1.0 {
            return;
        }
        if gained {
            fight.config.table.damage_taken_multiplier *= self.damage_taken_multiplier;
        } else {
            fight.config.table.damage_taken_multiplier /= self.damage_taken_multiplier;
        }
    }
}
