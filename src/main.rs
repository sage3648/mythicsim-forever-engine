use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepared_refusals, simulate, simulate_prepared,
    validate_prepared, PreparedError, Request, SOURCE_REVISION,
};
use serde::Deserialize;
use std::{
    env, fs,
    hint::black_box,
    io::Write,
    path::{Path, PathBuf},
    process,
};

/// Reads only the schema version so each contract keeps its own strict parser.
#[derive(Deserialize)]
struct SchemaPeek {
    schema_version: Option<u32>,
}

fn prepared_v2(input: &[u8]) -> Result<Option<PreparedV2>, String> {
    let peek: SchemaPeek =
        serde_json::from_slice(input).map_err(|err| format!("request rejected: {err}"))?;
    if peek.schema_version != Some(2) {
        return Ok(None);
    }
    serde_json::from_slice(input)
        .map(Some)
        .map_err(|err| format!("prepared input rejected: {err}"))
}

/// Writes `contents` to `path` so a reader sees the whole file or none of it.
///
/// The bytes go to a hidden sibling file, which is flushed to disk and then renamed over the
/// destination. A run that is killed or timed out part way, by a signal, a worker's timeout or
/// a full disk, leaves at most that sibling behind, never a truncated report. The sibling's
/// name starts with a dot and ends with `.tmp`, so it is never mistaken for a result.
fn write_atomically(path: &str, contents: &str) -> Result<(), String> {
    let destination = Path::new(path);
    let name = destination
        .file_name()
        .ok_or_else(|| format!("{path} is not a file path"))?;
    let mut sibling = std::ffi::OsString::from(".");
    sibling.push(name);
    sibling.push(format!(".{}.tmp", process::id()));
    let temporary: PathBuf = destination.with_file_name(sibling);
    let written = fs::File::create(&temporary)
        .and_then(|mut file| {
            file.write_all(contents.as_bytes())?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temporary, destination));
    if let Err(err) = written {
        // Best effort: the error being reported is the write's, not the cleanup's.
        let _ = fs::remove_file(&temporary);
        return Err(err.to_string());
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    if args == ["version"] {
        println!(
            "forever-rust-prototype-{} source-{SOURCE_REVISION}",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if args.is_empty() || args == ["--help"] {
        println!("forever-engine sim --infile REQUEST.json [--outfile RESULT.json] [--trace]\nforever-engine bench --infile REQUEST.json|PREPARED_V2.json [--outfile RESULT.json] [--warmups 3] [--samples 7]\nforever-engine check --infile PREPARED_V2.json\nforever-engine version");
        return Ok(());
    }
    if args.len() == 3 && args[0] == "check" && args[1] == "--infile" {
        let input = fs::read(&args[2]).map_err(|err| err.to_string())?;
        let prepared = prepared_v2(&input)?.ok_or("check requires a prepared v2 input")?;
        // An invalid input is an error, never a refusal.
        validate_prepared(&prepared).map_err(|err| PreparedError::Invalid(err).to_string())?;
        let refusals = prepared_refusals(&prepared);
        let reasons: Vec<&str> = refusals
            .iter()
            .map(|refusal| refusal.reason.as_str())
            .collect();
        let report = serde_json::json!({
            "scenario_id": prepared.scenario_id,
            "supported": refusals.is_empty(),
            "reasons": reasons,
            "refusals": refusals,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|err| err.to_string())?
        );
        return Ok(());
    }
    if args[0] != "sim" && args[0] != "bench" {
        return Err("expected sim, bench, check or version".into());
    }
    let mut infile = None;
    let mut outfile = None;
    let mut trace = false;
    let mut warmups = 3u32;
    let mut samples = 7u32;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--infile" | "--outfile" => {
                let flag = &args[index];
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| format!("{flag} requires a path"))?;
                let target = if flag == "--infile" {
                    &mut infile
                } else {
                    &mut outfile
                };
                if target.replace(value.clone()).is_some() {
                    return Err(format!("duplicate {flag}"));
                }
            }
            "--trace" if !trace => trace = true,
            "--warmups" | "--samples" if args[0] == "bench" => {
                let flag = &args[index];
                index += 1;
                let value: u32 = args
                    .get(index)
                    .ok_or_else(|| format!("{flag} requires a count"))?
                    .parse()
                    .map_err(|_| format!("invalid {flag}"))?;
                if flag == "--warmups" {
                    warmups = value;
                } else {
                    samples = value;
                }
            }
            unknown => return Err(format!("unsupported argument {unknown}")),
        }
        index += 1;
    }
    let input = fs::read(infile.ok_or("--infile is required")?).map_err(|err| err.to_string())?;
    if let Some(prepared) = prepared_v2(&input)? {
        if trace {
            return Err(
                "prepared v2 inputs do not support --trace; set debugFirstIteration for logs"
                    .into(),
            );
        }
        let report = simulate_prepared(&prepared).map_err(|err| err.to_string())?;
        let output = if args[0] == "bench" {
            if warmups > 20 || !(1..=100).contains(&samples) {
                return Err("bench requires 0 to 20 warmups and 1 to 100 samples".into());
            }
            for _ in 1..warmups {
                black_box(simulate_prepared(black_box(&prepared)).map_err(|err| err.to_string())?);
            }
            let mut timings = Vec::with_capacity(samples as usize);
            for _ in 0..samples {
                let sample = black_box(simulate_prepared(black_box(&prepared)))
                    .map_err(|err| err.to_string())?;
                if sample.result != report.result {
                    return Err("bench samples produced different results".into());
                }
                timings.push(sample.elapsed_ns);
            }
            serde_json::to_string_pretty(&serde_json::json!({
                "warmups": warmups.max(1), "samples": samples,
                "elapsed_ns_samples": timings, "report": report,
            }))
        } else {
            serde_json::to_string_pretty(&report)
        }
        .map_err(|err| err.to_string())?;
        if let Some(path) = outfile {
            write_atomically(&path, &(output + "\n"))?;
        } else {
            println!("{output}");
        }
        return Ok(());
    }
    let request: Request =
        serde_json::from_slice(&input).map_err(|err| format!("request rejected: {err}"))?;
    let output = if args[0] == "bench" {
        if trace || warmups > 20 || !(1..=100).contains(&samples) {
            return Err("bench requires no trace, 0 to 20 warmups and 1 to 100 samples".into());
        }
        for _ in 0..warmups {
            black_box(simulate(black_box(&request), false)?);
        }
        let mut timings = Vec::with_capacity(samples as usize);
        let mut reports = Vec::with_capacity(samples as usize);
        for _ in 0..samples {
            let report = black_box(simulate(black_box(&request), false)?);
            timings.push(report.elapsed_ns);
            reports.push(report);
        }
        serde_json::to_string_pretty(&serde_json::json!({
            "warmups": warmups, "samples": samples,
            "elapsed_ns_samples": timings, "reports": reports,
        }))
    } else {
        serde_json::to_string_pretty(&simulate(&request, trace)?)
    }
    .map_err(|err| err.to_string())?;
    if let Some(path) = outfile {
        write_atomically(&path, &(output + "\n"))?;
    } else {
        println!("{output}");
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::write_atomically;
    use std::fs;

    fn scratch(label: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("forever-main-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn names(directory: &std::path::Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn a_written_file_is_complete_and_alone() {
        let directory = scratch("whole");
        let path = directory.join("result.json");
        write_atomically(path.to_str().unwrap(), "first\n").unwrap();
        write_atomically(path.to_str().unwrap(), "second\n").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second\n");
        assert_eq!(names(&directory), ["result.json"]);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_failed_write_leaves_the_destination_and_no_temporary_file() {
        let directory = scratch("failed");
        // A directory cannot be replaced by a file, so the final rename fails after the
        // temporary file was written.
        let path = directory.join("result.json");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("kept"), "kept").unwrap();
        assert!(write_atomically(path.to_str().unwrap(), "new\n").is_err());
        assert_eq!(names(&directory), ["result.json"]);
        assert_eq!(fs::read_to_string(path.join("kept")).unwrap(), "kept");
        fs::remove_dir_all(directory).unwrap();
    }
}
