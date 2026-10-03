//! Prepared v2 execution for Mage builds. The gate decides which inputs Rust supports;
//! supported inputs run in the shared fight runtime with the Mage agent.

mod coverage;

pub(crate) use coverage::{prepared_coverage, IMPLEMENTED_EFFECTS};

/// Run a prepared v2 input that passed the coverage gate with the Mage agent.
pub(crate) fn run_prepared(
    prepared: &crate::contracts::prepared_v2::PreparedV2,
) -> Result<crate::core::fight::FightReport, String> {
    let mut fight = crate::classes::mage::agent::MageAgent::fight(prepared)?;
    Ok(fight.run())
}
