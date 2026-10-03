//! Go debug log lines for the first iteration, in Go's exact format, so a plain line
//! diff locates the first event where Rust and Go diverge.

use crate::{contracts::prepared_v2::ActionId, core::time::seconds};

use super::{Agent, Fight, Side};

/// Go `proto.OtherAction` numbers, which Go prints in action IDs.
pub(crate) fn other_action_number(name: &str) -> i32 {
    match name {
        "OtherActionWait" => 1,
        "OtherActionManaRegen" => 2,
        "OtherActionAttack" => 3,
        "OtherActionShoot" => 4,
        "OtherActionEnergyRegen" => 5,
        "OtherActionFocusRegen" => 6,
        "OtherActionPet" => 7,
        "OtherActionRefund" => 8,
        "OtherActionDamageTaken" => 9,
        "OtherActionManaGain" => 10,
        "OtherActionRageGain" => 11,
        "OtherActionHealingModel" => 12,
        "OtherActionPotion" => 17,
        "OtherActionMove" => 20,
        "OtherActionPrepull" => 21,
        "OtherActionEncounterStart" => 22,
        "OtherActionItemSwap" => 23,
        _ => 0,
    }
}

/// Go `ActionID.String`.
pub(crate) fn action_string(id: &ActionId) -> String {
    let mut text = String::from("{");
    if id.spell_id != 0 {
        text.push_str(&format!("SpellID: {}", id.spell_id));
    } else if id.item_id != 0 {
        text.push_str(&format!("ItemID: {}", id.item_id));
    } else if !id.other_id.is_empty() {
        text.push_str(&format!("OtherID: {}", other_action_number(&id.other_id)));
    }
    if id.tag != 0 {
        text.push_str(&format!(", Tag: {}", id.tag));
    }
    text.push('}');
    text
}

impl<A: Agent> Fight<A> {
    /// Go `Simulation.Log` at an explicit time, for lazily expired auras.
    pub(crate) fn log_at(&mut self, time: i64, label: &str, message: &str) {
        if let Some(lines) = self.log.as_mut() {
            lines.push(format!("[{:.2}] [{label}] {message}\n", seconds(time)));
        }
    }

    /// Go `Unit.Label`.
    pub(crate) fn label_of(&self, side: Side) -> String {
        match side {
            Side::Player => self.config.player_label.clone(),
            Side::Target => self.config.target_label.clone(),
            Side::Pet => self
                .pet
                .as_ref()
                .expect("the pet is simulated")
                .label
                .clone(),
        }
    }

    pub(crate) fn unit_log(&mut self, side: Side, message: &str) {
        if self.log.is_some() {
            let label = self.label_of(side);
            self.log_at(self.now, &label, message);
        }
    }

    pub(crate) fn player_log(&mut self, message: &str) {
        self.unit_log(Side::Player, message);
    }
}
