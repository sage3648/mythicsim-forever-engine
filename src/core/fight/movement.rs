//! Go movement.go: a unit moving toward or away from its target.
//!
//! A move casts the unit's Movement spell, whose aura stacks count the yards from the target,
//! and a pending action ends it at the time the distance takes at the unit's speed. The
//! unit's position is read lazily: each rotation evaluation and each new move bring it up to
//! date, which updates the aura's stacks and starts or stops the unit's swings as it enters or
//! leaves their range.

use crate::core::{queue::Handle, time::NS_PER_SECOND};

use super::{melee::Hand, Action, Agent, AuraRef, Fight, Side, SpellId, PRIORITY_GCD};

/// Go `MovementAction`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Movement {
    src_position: f64,
    move_distance: f64,
    start: i64,
    speed: f64,
    pub(crate) action: Handle,
}

impl<A: Agent> Fight<A> {
    /// The unit's Movement aura and spell, by Go's `OtherActionMove`.
    fn movement_parts(&self, side: Side) -> (AuraRef, SpellId) {
        let index = self.trackers[side.index()]
            .auras
            .iter()
            .position(|aura| {
                aura.action_id
                    .as_ref()
                    .is_some_and(|id| id.other_id == "OtherActionMove")
            })
            .expect("the unit has a Movement aura");
        let spell = (0..self.spells.len())
            .find(|&spell| {
                self.spells[spell].caster == side
                    && self.spells[spell].id.other_id == "OtherActionMove"
            })
            .expect("the unit has a Movement spell");
        (AuraRef { side, index }, spell)
    }

    /// The Movement spell's `ApplyEffects`: the aura, its stacks the yards from the target.
    pub(crate) fn apply_movement(&mut self, side: Side) {
        let (aura, _) = self.movement_parts(side);
        self.activate_aura(aura);
        let yards = (self.unit_config(side).distance as i32).max(1);
        self.set_stacks(aura, yards);
    }

    /// The Movement aura's gain and expiry.
    pub(crate) fn movement_changed(&mut self, side: Side, gained: bool) {
        let unit = self.unit_mut(side);
        unit.moving = gained;
        if !gained {
            unit.movement = None;
        }
    }

    /// Go `GetMovementSpeed` of a pet, the one unit that moves in scope.
    fn movement_speed(&self, side: Side) -> f64 {
        assert!(side == Side::Pet, "only a pet moves");
        self.pet
            .as_ref()
            .expect("the pet is simulated")
            .movement_speed
    }

    /// Go `Unit.MoveTo`.
    pub(crate) fn move_to(&mut self, side: Side, range: f64) {
        if range == self.unit_config(side).distance {
            return;
        }
        self.update_position(side, false);
        let distance = range - self.unit_config(side).distance;
        let speed = self.movement_speed(side);
        // Go time.Duration(math.Abs(d)/speed*1000) * time.Millisecond.
        let time = (distance.abs() / speed * 1000.0) as i64 * crate::core::time::NS_PER_MILLISECOND;
        let signed = if distance < 0.0 { -speed } else { speed };
        self.register_movement(side, signed, self.now + time, distance);
    }

    /// Go `registerMovementAction`.
    fn register_movement(&mut self, side: Side, speed: f64, end: i64, distance: f64) {
        match self.unit(side).movement {
            Some(movement) => {
                self.queue.cancel(movement.action);
            }
            None => {
                let (_, spell) = self.movement_parts(side);
                self.cast(spell, Side::Target);
            }
        }
        let action = self.schedule(end, PRIORITY_GCD, Action::MovementEnd(side));
        let src_position = self.unit_config(side).distance;
        let now = self.now;
        self.unit_mut(side).movement = Some(Movement {
            src_position,
            move_distance: distance,
            start: now,
            speed,
            action,
        });
    }

    /// Go `Unit.UpdatePosition`.
    pub(crate) fn update_position(&mut self, side: Side, last: bool) {
        if !self.unit(side).moving {
            return;
        }
        let Some(movement) = self.unit(side).movement else {
            return;
        };
        let old = self.unit_config(side).distance;
        let new = if last && movement.move_distance != 0.0 {
            movement.src_position + movement.move_distance
        } else {
            movement.src_position
                + (self.now - movement.start) as f64 * movement.speed / NS_PER_SECOND as f64
        };
        if old == new {
            return;
        }
        self.unit_config_mut(side).distance = new;
        let in_range = self.main_hand_in_range(side);
        if self.autos_of(side).mh.enabled != in_range {
            if in_range {
                self.enable_melee_swing(side);
            } else {
                self.cancel_melee_swing(side);
            }
        }
        let yards = (new as i32).max(1);
        let (aura, _) = self.movement_parts(side);
        if yards != self.aura(aura).stacks {
            self.set_stacks(aura, yards);
        }
    }

    /// Go `Unit.FinalizeMovement`.
    pub(crate) fn finalize_movement(&mut self, side: Side) {
        if !self.unit(side).moving {
            return;
        }
        self.update_position(side, true);
        let (aura, _) = self.movement_parts(side);
        self.deactivate_aura(aura);
    }

    fn main_hand_in_range(&self, side: Side) -> bool {
        let weapon = &self.autos_of(side).mh.weapon;
        let distance = self.unit_config(side).distance;
        (weapon.min_range == 0.0 || weapon.min_range < distance)
            && (weapon.max_range == 0.0 || weapon.max_range >= distance)
    }

    /// Go `AutoAttacks.EnableMeleeSwing` after the pull, for a unit that does not dual wield.
    fn enable_melee_swing(&mut self, side: Side) {
        let autos = self.autos_of(side);
        if !autos.melee || self.now < 0 {
            return;
        }
        assert!(!autos.dual_wielding, "a moving unit does not dual wield");
        let now = self.now;
        let in_range = self.main_hand_in_range(side);
        let haste = self.melee_haste_multiplier_of(side);
        let mh = &mut self.autos_of_mut(side).mh;
        mh.swing_at = mh.swing_at.max(now).max(0);
        if in_range && !mh.enabled {
            mh.enabled = true;
            self.add_weapon_attack(side, Hand::Main, haste);
        }
    }

    /// Go `AutoAttacks.CancelMeleeSwing` for a unit that does not dual wield.
    fn cancel_melee_swing(&mut self, side: Side) {
        if !self.autos_of(side).melee || !self.autos_of(side).mh.enabled {
            return;
        }
        self.remove_weapon_attack(side, Hand::Main);
        self.autos_of_mut(side).mh.enabled = false;
    }
}
