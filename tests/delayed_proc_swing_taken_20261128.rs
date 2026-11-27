//! A delayed proc that heard a swing the player took (production Best Gear, 27 Nov 2026).
//!
//! Nature's Bounty on a Bear Druid procs from the boss's swing, which is no spell of the
//! player's, so it schedules its delayed Rage proc with a sentinel past the spell table. The
//! delayed proc credits the damage it triggers to the spell it heard, and the Rage it gave let
//! the rotation cast at once, so the damage was credited to an index of `usize::MAX` and the
//! engine stopped with "index out of bounds". Nothing is credited to a swing taken.

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared, Rules,
};
use std::{fs, path::Path};

const BEAR: &str = "fixtures/druid/bear-natures-bounty-swing-taken.request.json";

#[test]
fn a_bear_druid_with_natures_bounty_sims_every_iteration() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(BEAR);
    let request = fs::read(path).unwrap();
    let prepared =
        prepare_json_with(&request, "rust", Rules::Forever).unwrap_or_else(|err| panic!("{err:?}"));
    let prepared: PreparedV2 = serde_json::from_value(prepared).unwrap();
    let report = simulate_prepared(&prepared).unwrap();
    let dps = report.result["raidMetrics"]["dps"]["avg"]
        .as_f64()
        .unwrap_or(0.0);
    assert!(dps > 0.0, "the bear deals damage: {dps}");
}
