//! Go sim/warrior's shouts: battle_shout.go, demoralizing_shout.go, challenging_shout.go and
//! intimidating_shout.go.

use crate::prepare::buffs;
use crate::prepare::character::constants::CHARACTER_LEVEL;
use crate::prepare::dbcenums;
use crate::prepare::sim::{BuildPhase, Cooldown, Sim, UnitId};
use crate::prepare::spell::{school, DefenseType, ProcMask, SpellConfig, SpellFlag};

use super::helpers::*;
use super::masks;
use super::spell_data::spell_data;
use super::Warrior;

/// Go `ShoutExpirationThreshold`: how close to running out the warrior's own shout has to be
/// before recasting it is worth a global.
pub(super) const SHOUT_EXPIRATION_THRESHOLD: crate::prepare::sim::Duration =
    3 * crate::prepare::sim::SECOND;

impl Warrior {
    /// Go `registerBattleShout`.
    pub(super) fn register_battle_shout(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().battle_shout.highest();
        // A warrior that shouts builds a copy of its own; one that shouts nothing gets the
        // isPlayer=false constructor, whose aura is the party's external copy.
        let casts_own_shout = self.inputs.use_battle_shout;
        let external_shout = crate::contracts::prepared_v2::ActionId {
            spell_id: rank.id,
            tag: -1,
            ..Default::default()
        };

        let auras = new_ally_aura_array(sim, |sim, ally| {
            // The party's Battle Shout registers the external copy before this runs, and that
            // copy keeps the build phase it was registered with.
            let party_shout = !casts_own_shout
                && sim
                    .unit(ally)
                    .auras
                    .iter()
                    .any(|aura| sim.aura(*aura).action_id.as_ref() == Some(&external_shout));
            let aura = buffs::BATTLE_SHOUT
                .class_aura(sim, ally, casts_own_shout, 0)
                .expect("Battle Shout is a buff");
            if !party_shout {
                sim.aura_mut(aura).build_phase = if casts_own_shout {
                    BuildPhase::BUFFS
                } else {
                    BuildPhase::NONE
                };
            }
            aura
        });
        let related = aura_array_to_map(sim, &auras);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                class_spell_mask: masks::BATTLE_SHOUT,
                spell_school: rank.spell_school(),
                flags: SpellFlag::APL | SpellFlag::HELPFUL,
                proc_mask: ProcMask::EMPTY,
                cost: rage_cost(rank),
                cast: cast_config(rank.gcd(), true, Cooldown::default()),
                threat_multiplier: 1.0,
                flat_threat_bonus: rank
                    .find_effect(dbcenums::E_THREAT, 0, 0)
                    .average(CHARACTER_LEVEL),
                has_extra_cast_condition: true,
                related_aura_arrays: related,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerDemoralizingShout`.
    pub(super) fn register_demoralizing_shout(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().demoralizing_shout.highest();
        let auras = new_enemy_aura_array(sim, |sim, target| {
            buffs::DEMORALIZING_SHOUT
                .class_aura(sim, target, true, 0)
                .expect("Demoralizing Shout is a debuff")
        });
        let related = aura_array_to_map(sim, &auras);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: rank.spell_school(),
                defense_type: rank.defense_type_core(),
                class_spell_mask: masks::DEMORALIZING_SHOUT,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::APL,
                cost: rage_cost(rank),
                cast: cast_config(rank.gcd(), true, Cooldown::default()),
                // Not in the client table; Go's Classic value until measured in game.
                threat_multiplier: 0.4,
                flat_threat_bonus: 43.2,
                related_aura_arrays: related,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerChallengingShout`.
    pub(super) fn register_challenging_shout(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().challenging_shout.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::APL,
                class_spell_mask: masks::CHALLENGING_SHOUT,
                cost: rage_cost(rank),
                cast: cast_config(rank.gcd(), true, cd),
                threat_multiplier: 1.0,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `registerIntimidatingShout`.
    pub(super) fn register_intimidating_shout(&self, sim: &mut Sim, unit: UnitId) {
        let rank = spell_data().intimidating_shout.highest();
        let cd = new_cooldown(sim, unit, cooldown_of(rank));
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(rank.id),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::APL,
                class_spell_mask: masks::INTIMIDATING_SHOUT,
                max_range: f64::from(rank.max_range),
                cost: rage_cost(rank),
                cast: cast_config(rank.gcd(), true, cd),
                threat_multiplier: 1.0,
                ..SpellConfig::default()
            },
        );
    }
}
