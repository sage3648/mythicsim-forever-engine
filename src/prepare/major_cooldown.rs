//! Go sim/core/major_cooldown.go: the registration helpers. The cooldown manager's
//! registration (`AddMajorCooldown`) and finalization live with the character in character.rs.
//!
//! What the manager does during a fight (picking, sorting and casting a cooldown) is not part of
//! preparation.

use super::aura_helpers::StatBuffAura;
use super::character::MajorCooldown;
use super::sim::{Duration, Sim, UnitId};
use super::spell::{SpellConfig, SpellFlag};
use super::stats::Stats;
use crate::contracts::prepared_v2::ActionId;

/// Go `CooldownPriorityLow` and `CooldownPriorityDefault`.
#[allow(dead_code)]
pub(crate) const COOLDOWN_PRIORITY_LOW: i32 = -1000;
#[allow(dead_code)]
pub(crate) const COOLDOWN_PRIORITY_DEFAULT: i32 = 0;

impl Sim {
    /// Go `RegisterTemporaryStatsOnUseCD`: a major cooldown that gives a temporary boost to
    /// stats, as Icon of the Silver Crescent and Bloodlust Brooch do. The spell's cooldown is
    /// the aura's ICD, and the cooldown type follows the stats the aura buffs.
    pub(crate) fn register_temporary_stats_on_use_cd(
        &mut self,
        unit: UnitId,
        aura_label: &str,
        temp_stats: Stats,
        duration: Duration,
        mut config: SpellConfig,
    ) -> StatBuffAura {
        let action_id: ActionId = config.action_id.clone();
        let aura =
            self.new_temporary_stats_aura(unit, aura_label, &action_id, temp_stats, duration);
        let cd_type = aura.infer_cd_type();
        if config.cast.cd.duration > 0 {
            self.aura_mut(aura.aura).icd = Some(config.cast.cd);
        }
        config.flags |= SpellFlag::NO_ON_CAST_COMPLETE;
        let spell = self.register_spell(unit, config);
        self.spell_mut(spell).related_self_buff = Some(aura.aura);
        self.add_major_cooldown(
            unit,
            MajorCooldown {
                spell,
                priority: 0,
                cooldown_type: cd_type,
                allow_spell_queueing: false,
                timings: Vec::new(),
            },
        );
        aura
    }
}
