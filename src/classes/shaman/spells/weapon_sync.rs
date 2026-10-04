//! Enhancement's weapon sync, from Go sim/shaman/enhancement/enhancement.go `ApplySyncType`:
//! before each main hand swing it may move the off hand swing, so that both weapons can share
//! Flurry's charges, and returns the swing it was given.

use crate::core::{
    fight::{Agent, Fight},
    time::{go_string, NS_PER_MILLISECOND},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SyncType {
    /// The replacement returns the swing untouched, as Auto does for weapons of unequal speed.
    None,
    /// Go `delayOffhandSwing`, which Auto uses for weapons of equal speed.
    Delay,
    /// Go `syncMainhandOffhandSwings`.
    Sync,
}

impl SyncType {
    pub(crate) fn parse(name: &str) -> Result<SyncType, String> {
        match name {
            "none" => Ok(SyncType::None),
            "auto" | "delay" => Ok(SyncType::Delay),
            "sync" => Ok(SyncType::Sync),
            other => Err(format!("weapon sync {other} is unknown")),
        }
    }
}

/// Go `Duration.Truncate(time.Millisecond)`.
fn truncate_ms(duration: i64) -> i64 {
    duration - duration % NS_PER_MILLISECOND
}

/// The replacement's effect on the off hand swing.
pub(crate) fn apply<A: Agent>(fight: &mut Fight<A>, sync: SyncType, flurry_icd: i64) {
    let now = fight.now;
    let mh_speed = fight.autos.mh.cur_swing_duration;
    let oh_speed = fight.autos.oh.cur_swing_duration;
    let oh_at = fight.autos.oh.swing_at;
    if oh_at - now <= flurry_icd {
        return;
    }
    let (next, verb) = match sync {
        SyncType::None => return,
        SyncType::Delay => {
            let next = now + mh_speed + 100 * NS_PER_MILLISECOND;
            if next <= oh_at {
                return;
            }
            (next, "Delaying OH swing")
        }
        SyncType::Sync => {
            let next = now + mh_speed;
            if next == oh_at {
                return;
            }
            (next, "Syncing OH with MH")
        }
    };
    // Go SetOffhandSwingAt, which does not reschedule the weapon attacks.
    fight.autos.oh.swing_at = next;
    if fight.log.is_some() {
        let line = format!(
            "(Weapon Sync for {}/{}) {verb}, setting next OH swing to {}",
            go_string(truncate_ms(mh_speed)),
            go_string(truncate_ms(oh_speed)),
            go_string(next)
        );
        fight.player_log(&line);
    }
}
