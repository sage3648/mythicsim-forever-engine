//! Go common/classic/emerald_dragon_whelp.go and Dragon's Call in items_weapons.go: a weapon
//! proc whose handler summons the Emerald Dragon Whelp, a guardian [`ActivePet`] with its own
//! mana bar, for a fixed time. After each swing or spit the whelp spits Acid Spit half the
//! time, holding its swing until the cast ends, and otherwise waits for its next swing.
//!
//! [`ActivePet`]: super::pet::ActivePet

use crate::contracts::prepared_v2::Effect;

use super::{Agent, AuraRef, Fight, Side, SpellId, SpellResult, OUTCOME_LANDED};

/// The bound Dragon's Call proc and its whelp's rotation.
#[derive(Clone, Debug)]
pub(crate) struct Whelp {
    /// The proc chance of each spell, from the weapon's proc manager.
    pub(crate) chances: Vec<Option<f64>>,
    /// How long each summon lasts.
    pub(crate) duration: i64,
    pub(crate) acid_spit: SpellId,
    pub(crate) spit_chance: f64,
    /// The simulated pet that is the whelp.
    pub(crate) pet: Side,
}

impl<A: Agent> Fight<A> {
    /// Bind Dragon's Call to the simulated summoned guardian and its Acid Spit.
    pub(crate) fn bind_whelp(&mut self, effect: &Effect) -> Result<(), String> {
        let Effect::EmeraldDragonWhelp {
            pet,
            chances,
            delay_ns,
            duration_ns,
            acid_spit_spell_id,
            spit_chance,
            ..
        } = effect
        else {
            return Ok(());
        };
        let Some(side) = self.pet_sides().into_iter().find(|&side| {
            let active = self.active_pet(side);
            active.summoned && active.label == *pet
        }) else {
            return Err(format!("{pet} is not a simulated summoned pet"));
        };
        if *delay_ns != super::SPELL_BATCH_WINDOW {
            return Err(format!(
                "Dragon's Call waits {delay_ns}ns, not a spell batch window"
            ));
        }
        let acid_spit = self
            .spells
            .iter()
            .position(|spell| {
                spell.caster == side
                    && spell.id.spell_id == *acid_spit_spell_id
                    && spell.id.tag == 0
            })
            .ok_or_else(|| format!("Acid Spit {acid_spit_spell_id} is not registered"))?;
        let mut by_spell = vec![None; self.spells.len()];
        for entry in chances {
            if let Some(slot) = by_spell.get_mut(entry.spell) {
                *slot = Some(entry.chance);
            }
        }
        self.whelp = Some(Whelp {
            chances: by_spell,
            duration: *duration_ns,
            acid_spit,
            spit_chance: *spit_chance,
            pet: side,
        });
        Ok(())
    }

    /// Go `AttachProcTriggerCallback` for Dragon's Call: landed hits roll the spell's chance,
    /// then the handler waits a spell batch window.
    pub(crate) fn whelp_callback(&mut self, aura: AuraRef, spell: SpellId, result: &SpellResult) {
        if result.outcome & OUTCOME_LANDED == 0 {
            return;
        }
        let Some(chance) = self.whelp.as_ref().and_then(|whelp| whelp.chances[spell]) else {
            return;
        };
        if !self.proc_for_aura(chance, aura) {
            return;
        }
        self.schedule_delayed_proc(aura, spell, *result);
    }

    /// Dragon's Call's handler: `EmeraldDragonWhelp.summon`.
    pub(crate) fn whelp_summon(&mut self) {
        let whelp = self.whelp.as_ref().expect("the whelp is bound");
        let (side, duration) = (whelp.pet, whelp.duration);
        self.summon_pet(side, duration);
    }

    /// Go `EmeraldDragonWhelp.ExecuteCustomRotation`: a spit it lives to finish, half the time,
    /// holds its swing until the cast ends; otherwise it waits for its next swing.
    pub(crate) fn whelp_rotation(&mut self, side: Side) {
        let whelp = self.whelp.clone().expect("the whelp is bound");
        let spit = whelp.acid_spit;
        let cast_end = self.now
            + self.apply_cast_speed_for_spell(self.spells[spit].default_cast.cast_time, spit);
        let disabled_at = self.active_pet(side).disabled_at;
        if cast_end < disabled_at
            && self.can_cast(spit)
            && self.proc(whelp.spit_chance, "Acid Spit Cast")
        {
            self.stop_melee_until_of(side, cast_end);
            self.cast(spit, Side::Target);
            return;
        }
        let autos = self.autos_of(side);
        let next = autos.mh.swing_at.min(autos.oh.swing_at);
        self.wait_until_of(side, next);
    }
}
