//! Go preset targets (sim/encounters): only whether a target ID brings an AI.

/// Go `GetPresetTargetWithID(id).AI != nil`.
pub(crate) fn preset_target_has_ai(id: i32) -> bool {
    crate::data::tables::tables()
        .preset_targets_with_ai
        .contains(&id)
}
