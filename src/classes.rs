//! Class domains. A module exists only when it contains implemented behavior.

pub(crate) mod druid;
pub(crate) mod mage;
pub(crate) mod paladin;
pub(crate) mod priest;
pub(crate) mod shaman;

/// Run a prepared v2 input that passed the coverage gate with its class's agent.
pub(crate) fn run_prepared(
    prepared: &crate::contracts::prepared_v2::PreparedV2,
) -> Result<crate::core::fight::FightReport, String> {
    match prepared.player.class.as_str() {
        "ClassMage" => mage::prepared::run_prepared(prepared),
        "ClassDruid" => druid::prepared::run_prepared(prepared),
        "ClassShaman" => shaman::prepared::run_prepared(prepared),
        "ClassPaladin" => paladin::prepared::run_prepared(prepared),
        "ClassPriest" => priest::prepared::run_prepared(prepared),
        other => Err(format!("class {other} has no fight agent")),
    }
}
