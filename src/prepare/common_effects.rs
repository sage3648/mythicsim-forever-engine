//! The class-independent effects the exporter describes: tools/oracle-v2 `commonEffects` and
//! the effect lists `prepare` appends after it.

use serde_json::Value;

use super::env::Environment;

pub(crate) fn common_effects(
    _env: &mut Environment,
    _unrepresented: &mut Vec<String>,
) -> Vec<Value> {
    Vec::new()
}
