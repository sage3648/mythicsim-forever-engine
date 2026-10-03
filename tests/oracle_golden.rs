use forever_engine::{simulate, Counts, Request, SOURCE_REVISION};
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Deserialize)]
struct Manifest {
    source_revision: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    rust: String,
    expected: Expected,
}

#[derive(Deserialize)]
struct Expected {
    iterations: u32,
    seed: u64,
    dps_mean: f64,
    dps_stdev: f64,
    mana_delta_mean: f64,
    counts: Counts,
}

#[test]
fn matches_real_pinned_engine_goldens_for_all_fixed_gear_scenarios() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let manifest: Manifest =
        serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest.source_revision, SOURCE_REVISION);
    assert_eq!(manifest.cases.len(), 11);
    for case in manifest.cases {
        let request: Request =
            serde_json::from_slice(&fs::read(directory.join(&case.rust)).unwrap()).unwrap();
        let report = simulate(&request, false).unwrap();
        assert_eq!(
            report.iterations, case.expected.iterations,
            "{} iterations",
            case.id
        );
        assert_eq!(report.seed, case.expected.seed, "{} seed", case.id);
        assert_eq!(report.counts, case.expected.counts, "{} outcomes", case.id);
        assert!(
            (report.dps_mean - case.expected.dps_mean).abs() <= 1e-8,
            "{} DPS",
            case.id
        );
        assert!(
            (report.dps_stdev - case.expected.dps_stdev).abs() <= 1e-8,
            "{} DPS spread",
            case.id
        );
        assert!(
            (report.mana_delta_mean - case.expected.mana_delta_mean).abs() <= 1e-8,
            "{} mana",
            case.id
        );
    }
}
