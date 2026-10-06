//! A run that is timed out, cancelled or killed never leaves a partial result.
//!
//! The engine has no timeout or cancellation of its own: a worker bounds a run by ending its
//! process, as `tools/route.py` does when a step outlives `--timeout`. The contract is
//! therefore about what the process leaves behind. It writes its report only after the whole
//! simulation succeeded, and writes it by renaming a finished file over the destination, so
//! an interrupted run leaves the destination untouched: absent, or holding what it held
//! before. A caller that finds a result file can trust that it is complete.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::Duration,
};

const ENGINE: &str = env!("CARGO_BIN_EXE_forever-engine");
const SETTLE: Duration = Duration::from_millis(300);

fn fixture(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(relative)
}

/// A scratch folder holding a prepared input that runs for far longer than a test waits: a
/// cheap accepted fixture asked for the most iterations the contract allows.
fn long_run(label: &str) -> (PathBuf, PathBuf) {
    let directory = std::env::temp_dir().join(format!(
        "forever-interruption-{label}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    let source = fixture("mage/prepared-v2/feral-bear-druid-boomerang-pushback.prepared.json");
    let mut prepared: serde_json::Value =
        serde_json::from_slice(&fs::read(source).unwrap()).unwrap();
    prepared["sim"]["iterations"] = 1_000_000.into();
    let input = directory.join("long.prepared.json");
    fs::write(&input, serde_json::to_vec(&prepared).unwrap()).unwrap();
    (directory, input)
}

fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Starts the long run and, once it is certainly simulating, interrupts it.
fn interrupt(input: &Path, output: &Path, how: impl FnOnce(&mut Child)) {
    let mut child = Command::new(ENGINE)
        .args(["sim", "--infile"])
        .arg(input)
        .arg("--outfile")
        .arg(output)
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(SETTLE);
    assert!(
        child.try_wait().unwrap().is_none(),
        "the run ended before it could be interrupted"
    );
    how(&mut child);
    assert!(!child.wait().unwrap().success());
}

fn kill(child: &mut Child) {
    child.kill().unwrap();
}

#[cfg(unix)]
fn terminate(child: &mut Child) {
    let sent = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .unwrap();
    assert!(sent.success());
}

#[test]
fn a_killed_run_leaves_no_result_file() {
    let (directory, input) = long_run("kill");
    interrupt(&input, &directory.join("result.json"), kill);
    assert_eq!(names(&directory), ["long.prepared.json"]);
    fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn a_terminated_run_leaves_no_result_file() {
    let (directory, input) = long_run("terminate");
    interrupt(&input, &directory.join("result.json"), terminate);
    assert_eq!(names(&directory), ["long.prepared.json"]);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn an_interrupted_run_never_replaces_an_earlier_result() {
    let (directory, input) = long_run("earlier");
    let output = directory.join("result.json");
    fs::write(&output, "earlier result\n").unwrap();
    interrupt(&input, &output, kill);
    assert_eq!(fs::read_to_string(&output).unwrap(), "earlier result\n");
    assert_eq!(names(&directory), ["long.prepared.json", "result.json"]);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_finished_run_leaves_only_its_complete_result() {
    let (directory, _) = long_run("finished");
    let output = directory.join("result.json");
    let done = Command::new(ENGINE)
        .args(["sim", "--infile"])
        .arg(fixture("static-frost-60.rust.json"))
        .arg("--outfile")
        .arg(&output)
        .output()
        .unwrap();
    assert!(done.status.success());
    let report: serde_json::Value = serde_json::from_slice(&fs::read(&output).unwrap()).unwrap();
    assert_eq!(report["scenario_id"], "static-frost-60");
    assert_eq!(names(&directory), ["long.prepared.json", "result.json"]);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_result_that_cannot_be_written_is_an_error_and_leaves_nothing() {
    let (directory, _) = long_run("unwritable");
    let done = Command::new(ENGINE)
        .args(["sim", "--infile"])
        .arg(fixture("static-frost-60.rust.json"))
        .arg("--outfile")
        .arg(directory.join("missing").join("result.json"))
        .output()
        .unwrap();
    assert!(!done.status.success());
    assert!(!done.stderr.is_empty());
    assert_eq!(names(&directory), ["long.prepared.json"]);
    fs::remove_dir_all(directory).unwrap();
}

/// The run that fails part way, rather than being interrupted, leaves nothing either: an
/// input the engine rejects writes no report, as `tests/cli.rs` also checks for v1.
#[test]
fn a_prepared_input_the_engine_rejects_leaves_no_result() {
    let (directory, input) = long_run("rejected");
    let mut prepared: serde_json::Value =
        serde_json::from_slice(&fs::read(&input).unwrap()).unwrap();
    prepared["sim"]["seed"] = 0.into();
    fs::write(&input, serde_json::to_vec(&prepared).unwrap()).unwrap();
    let done = Command::new(ENGINE)
        .args(["sim", "--infile"])
        .arg(&input)
        .arg("--outfile")
        .arg(directory.join("result.json"))
        .output()
        .unwrap();
    assert!(!done.status.success());
    assert!(String::from_utf8_lossy(&done.stderr).contains("rejected"));
    assert_eq!(names(&directory), ["long.prepared.json"]);
    fs::remove_dir_all(directory).unwrap();
}
