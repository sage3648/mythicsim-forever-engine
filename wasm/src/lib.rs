//! A C ABI for running the engine in a browser, without wasm-bindgen.
//!
//! The page copies a UTF-8 JSON input into memory from [`fe_alloc`], calls an entry point
//! with its pointer and length, then reads the JSON output at [`fe_output_ptr`] and
//! [`fe_output_len`]. The call frees the input buffer.
//!
//! [`fe_sim_request`] does what the worker's `forever-engine sim --request FILE --scenario
//! route --gate --rotation-stats` does under Forever rules: Rust preparation, validation, the
//! coverage gate, the simulation and the rotation statistics. It returns:
//!
//! | Status | Output |
//! | --- | --- |
//! | 0 | The report the native binary writes |
//! | 1 | `{"error": ...}`: the input is invalid or the run failed |
//! | 2 | `{"refused": true, "stage", "refusals"}`: Rust does not cover the request |
//!
//! An instance runs one request at a time, so a page that wants every core runs one instance
//! per Web Worker.

use std::cell::RefCell;

use forever_engine::{
    contracts::prepared_v2::PreparedV2, prepare_json_with, simulate_prepared_gated,
    validate_prepared, EngineIdentity, Gated, PrepareError, PreparedError, Rules,
};
use serde_json::json;

thread_local! {
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// The scenario id the worker's route gives a request.
const SCENARIO: &str = "route";

const STATUS_OK: u32 = 0;
const STATUS_ERROR: u32 = 1;
const STATUS_REFUSED: u32 = 2;

/// Reserves `len` bytes for an input. The next entry point call frees them.
#[no_mangle]
pub extern "C" fn fe_alloc(len: usize) -> *mut u8 {
    let mut buffer = Vec::<u8>::with_capacity(len.max(1));
    let pointer = buffer.as_mut_ptr();
    std::mem::forget(buffer);
    pointer
}

#[no_mangle]
pub extern "C" fn fe_output_ptr() -> *const u8 {
    OUTPUT.with(|output| output.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn fe_output_len() -> usize {
    OUTPUT.with(|output| output.borrow().len())
}

#[no_mangle]
pub extern "C" fn fe_identity() -> u32 {
    match serde_json::to_vec(&EngineIdentity::current()) {
        Ok(bytes) => finish(STATUS_OK, bytes),
        Err(err) => error(err.to_string()),
    }
}

/// # Safety
/// `pointer` and `len` must come from one [`fe_alloc`] call, filled by the caller.
#[no_mangle]
pub unsafe extern "C" fn fe_sim_request(pointer: *mut u8, len: usize) -> u32 {
    let request = Vec::from_raw_parts(pointer, len, len.max(1));
    let prepared = match prepare_json_with(&request, SCENARIO, Rules::Forever) {
        Ok(prepared) => prepared,
        Err(PrepareError::Refused(refusal)) => {
            return refused(
                "prepare",
                json!([{"code": refusal.code, "reason": refusal.reason}]),
            )
        }
        Err(PrepareError::Invalid(reason)) => {
            return refused("prepare", json!([{"code": "request", "reason": reason}]))
        }
        Err(PrepareError::Fault(reason)) => {
            return refused(
                "prepare",
                json!([{"code": "prepare_fault", "reason": reason}]),
            )
        }
    };
    // Through bytes, as the native binary reads its own preparation back.
    let prepared: PreparedV2 =
        match serde_json::to_vec(&prepared).and_then(|bytes| serde_json::from_slice(&bytes)) {
            Ok(prepared) => prepared,
            Err(err) => return error(format!("prepared input rejected: {err}")),
        };
    if let Err(err) = validate_prepared(&prepared) {
        return error(PreparedError::Invalid(err).to_string());
    }
    let mut report = match simulate_prepared_gated(&prepared) {
        Ok(Gated::Simulated(report)) => *report,
        Ok(Gated::Refused(refusals)) => return refused("check", json!(refusals)),
        Err(err) => return error(err.to_string()),
    };
    report.add_rotation_stats();
    match serde_json::to_vec(&report) {
        Ok(bytes) => finish(STATUS_OK, bytes),
        Err(err) => error(err.to_string()),
    }
}

fn refused(stage: &str, refusals: serde_json::Value) -> u32 {
    finish(
        STATUS_REFUSED,
        json!({"refused": true, "stage": stage, "refusals": refusals})
            .to_string()
            .into_bytes(),
    )
}

fn error(message: String) -> u32 {
    finish(
        STATUS_ERROR,
        json!({ "error": message }).to_string().into_bytes(),
    )
}

fn finish(status: u32, bytes: Vec<u8>) -> u32 {
    OUTPUT.with(|output| *output.borrow_mut() = bytes);
    status
}
