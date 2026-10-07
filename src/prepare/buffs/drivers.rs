//! Go sim/core/buffs/drivers.go and paladin.go: the buffs the generated tables hand to a driver.

use crate::contracts::request::Message;

use super::super::env::Environment;
use super::super::sim::UnitId;
use super::super::Refusal;

fn not_yet(name: &str) -> Result<(), Refusal> {
    Err(Refusal::new("buff", format!("{name} is not prepared yet")))
}

pub(crate) fn drive_battle_shout(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Battle Shout")
}
pub(crate) fn drive_mana_tide_totems(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Mana Tide Totem")
}
pub(crate) fn drive_retribution_aura(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Retribution Aura")
}
pub(crate) fn drive_atiesh_mage(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Atiesh")
}
pub(crate) fn drive_atiesh_warlock(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Atiesh")
}
pub(crate) fn drive_atiesh_druid(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Atiesh")
}
pub(crate) fn drive_atiesh_priest(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Atiesh")
}
pub(crate) fn drive_grace_of_air_totem(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Grace of Air Totem")
}
pub(crate) fn drive_windfury_totem(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Windfury Totem")
}
pub(crate) fn drive_flametongue_totem(
    _env: &mut Environment,
    _unit: UnitId,
    _party: &Message,
) -> Result<(), Refusal> {
    not_yet("Flametongue Totem")
}
pub(crate) fn drive_greater_blessing_of_light(
    _env: &mut Environment,
    _unit: UnitId,
    _individual: &Message,
) -> Result<(), Refusal> {
    not_yet("Greater Blessing of Light")
}
pub(crate) fn drive_innervates(
    _env: &mut Environment,
    _unit: UnitId,
    _individual: &Message,
) -> Result<(), Refusal> {
    not_yet("Innervate")
}
pub(crate) fn drive_power_infusions(
    _env: &mut Environment,
    _unit: UnitId,
    _individual: &Message,
) -> Result<(), Refusal> {
    not_yet("Power Infusion")
}
pub(crate) fn drive_judgement_of_light(
    _env: &mut Environment,
    _target: UnitId,
    _debuffs: &Message,
    _raid: &Message,
) -> Result<(), Refusal> {
    not_yet("Judgement of Light")
}
pub(crate) fn drive_judgement_of_wisdom(
    _env: &mut Environment,
    _target: UnitId,
    _debuffs: &Message,
    _raid: &Message,
) -> Result<(), Refusal> {
    not_yet("Judgement of Wisdom")
}
pub(crate) fn drive_sunder_armor(
    _env: &mut Environment,
    _target: UnitId,
    _debuffs: &Message,
    _raid: &Message,
) -> Result<(), Refusal> {
    not_yet("Sunder Armor")
}
