use forever_engine::{
    check_prepared, contracts::prepared_v2::PreparedV2, simulate, simulate_prepared, PreparedError,
    Request, SOURCE_REVISION,
};
use serde::Deserialize;
use std::{env, fs, hint::black_box, process};

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
        let reasons = match check_prepared(&prepared) {
            Ok(()) => Vec::new(),
            Err(PreparedError::Unsupported(reasons)) => reasons,
            Err(err) => return Err(err.to_string()),
        };
        let report = serde_json::json!({
            "scenario_id": prepared.scenario_id,
            "supported": reasons.is_empty(),
            "reasons": reasons,
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
            fs::write(path, output + "\n").map_err(|err| err.to_string())?;
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
        fs::write(path, output + "\n").map_err(|err| err.to_string())?;
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
