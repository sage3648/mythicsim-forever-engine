//! Class domains. A module exists only when it contains implemented behavior.

pub(crate) mod druid;
pub(crate) mod hunter;
pub(crate) mod mage;
pub(crate) mod paladin;
pub(crate) mod priest;
pub(crate) mod rogue;
pub(crate) mod shaman;
pub(crate) mod warlock;
pub(crate) mod warrior;

/// Run a prepared v2 input that passed the coverage gate with its class's agent.
pub(crate) fn run_prepared(
    prepared: &crate::contracts::prepared_v2::PreparedV2,
) -> Result<crate::core::fight::FightReport, String> {
    match prepared.player.class.as_str() {
        "ClassMage" => mage::prepared::run_prepared(prepared),
        "ClassDruid" => druid::prepared::run_prepared(prepared),
        "ClassWarlock" => warlock::prepared::run_prepared(prepared),
        "ClassShaman" => shaman::prepared::run_prepared(prepared),
        "ClassPaladin" => paladin::prepared::run_prepared(prepared),
        "ClassPriest" => priest::prepared::run_prepared(prepared),
        "ClassRogue" => rogue::prepared::run_prepared(prepared),
        "ClassWarrior" => warrior::prepared::run_prepared(prepared),
        "ClassHunter" => hunter::prepared::run_prepared(prepared),
        other => Err(format!("class {other} has no fight agent")),
    }
}

/// The class agent Rust preparation builds for a player: Go's agent factory.
pub(crate) fn prepare_agent(
    sim: &mut crate::prepare::sim::Sim,
    unit: crate::prepare::sim::UnitId,
    player: &crate::contracts::request::Message,
) -> Result<Box<dyn crate::prepare::agent::PrepAgent>, crate::prepare::Refusal> {
    match player.enum_name("class").as_str() {
        "ClassMage" => mage::prepare::new_mage(sim, unit, player),
        "ClassWarrior" => warrior::prepare::new_warrior(sim, unit, player),
        "ClassPaladin" => paladin::prepare::new_paladin(sim, unit, player),
        other => Err(crate::prepare::Refusal::new(
            "class",
            format!("{other} is not prepared in Rust yet"),
        )),
    }
}

/// The item sets the classes implement: Go's `core.NewItemSet` calls in each class package.
pub(crate) fn item_sets() -> Vec<&'static crate::prepare::item_sets::ItemSet> {
    let mut sets = Vec::new();
    sets.extend(mage::prepare::items::ITEM_SETS);
    sets.extend(warrior::prepare::items::sets::ITEM_SETS);
    sets.extend(paladin::prepare::sets::ITEM_SETS);
    sets
}
