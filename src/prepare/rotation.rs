//! Go sim/core/apl.go `newAPLRotation`, for what construction changes: the prepull actions
//! and the major cooldowns the rotation casts itself.

use crate::contracts::request::Message;

use super::env::Environment;
use super::Refusal;

/// Go `newAPLRotation` at finalization.
pub(crate) fn build_rotation(
    env: &mut Environment,
    rotation: Option<&Message>,
) -> Result<(), Refusal> {
    let Some(rotation) = rotation else {
        return Ok(());
    };
    if rotation.enum_name("type") != "TypeAPL" {
        return Err(Refusal::new(
            "rotation",
            "only APL rotations are prepared".to_string(),
        ));
    }
    env.prepull_actions += rotation.messages("prepull_actions").len();
    Err(Refusal::new(
        "rotation",
        "rotations are not prepared yet".to_string(),
    ))
}
