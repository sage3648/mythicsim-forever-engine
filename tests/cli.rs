use std::{fs, process::Command};

#[test]
fn invalid_input_returns_nonzero_and_does_not_write_a_report() {
    let directory =
        std::env::temp_dir().join(format!("forever-rust-cli-test-{}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let input = directory.join("request.json");
    let output = directory.join("result.json");
    fs::write(&input, "{\"raid\":{}}").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_forever-engine"))
        .args(["sim", "--infile"])
        .arg(input)
        .arg("--outfile")
        .arg(&output)
        .output()
        .unwrap();
    let report_exists = output.exists();
    fs::remove_dir_all(directory).unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("request rejected"));
    assert!(!report_exists);
}

#[test]
fn version_identifies_prototype_and_source_revision() {
    let result = Command::new(env!("CARGO_BIN_EXE_forever-engine"))
        .arg("version")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains(forever_engine::SOURCE_REVISION));
}

#[test]
fn benchmark_reports_only_requested_samples_after_warmup() {
    let input =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/static-frost-60.rust.json");
    let result = Command::new(env!("CARGO_BIN_EXE_forever-engine"))
        .args(["bench", "--infile"])
        .arg(input)
        .args(["--warmups", "2", "--samples", "3"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["warmups"], 2);
    assert_eq!(value["samples"], 3);
    assert_eq!(value["elapsed_ns_samples"].as_array().unwrap().len(), 3);
    let reports = value["reports"].as_array().unwrap();
    assert_eq!(reports.len(), 3);
    for report in reports {
        assert_eq!(
            report["work"]["cast_completions"],
            report["counts"]["casts"]
        );
        assert_eq!(report["work"], reports[0]["work"]);
        assert_eq!(report["dps_mean"], reports[0]["dps_mean"]);
    }
}

#[test]
fn prepared_benchmark_repeats_one_result() {
    let input = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/mage/prepared-v2/reference-clearcasting.prepared.json");
    let result = Command::new(env!("CARGO_BIN_EXE_forever-engine"))
        .args(["bench", "--infile"])
        .arg(input)
        .args(["--warmups", "1", "--samples", "2"])
        .output()
        .unwrap();
    assert!(result.status.success());
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["warmups"], 1);
    assert_eq!(value["samples"], 2);
    assert_eq!(value["elapsed_ns_samples"].as_array().unwrap().len(), 2);
    assert_eq!(value["report"]["scenario_id"], "reference-clearcasting");
    assert!(
        value["report"]["result"]["raidMetrics"]["dps"]["avg"]
            .as_f64()
            .unwrap()
            > 0.0
    );
}
