//! Life Tap (11689), from Go sim/warlock/lifetap.go: (base + Spirit) times one plus Improved
//! Life Tap, spent as health and gained as mana. No damage modifier touches either side.
//! Demonic Energies gives the summoned demon a share of the restore.

use crate::{
    contracts::prepared_v2::ActionId,
    core::fight::{Agent, Fight, Side},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct LifeTap {
    base_amount: f64,
    mana_multiplier: f64,
    /// The prepared Spirit, which a stat aura can change.
    spirit: f64,
    metrics: usize,
    /// The demon's share and its mana metrics.
    pet: Option<(f64, usize)>,
}

/// Go reads Spirit at each cast, which a stat aura such as an on-use trinket's can change.
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    spell_id: i32,
    base_amount: f64,
    mana_multiplier: f64,
    spirit: f64,
    pet_mana_share: f64,
    demon: Option<Side>,
) -> LifeTap {
    let id = ActionId {
        spell_id,
        ..ActionId::default()
    };
    let metrics = fight.new_mana_metrics(id.clone());
    // Go registers each demon's metrics with the spell; only the summoned one is simulated.
    let pet = demon
        .filter(|_| pet_mana_share > 0.0)
        .map(|demon| (pet_mana_share, fight.new_mana_metrics_of(demon, id)));
    LifeTap {
        base_amount,
        mana_multiplier,
        spirit,
        metrics,
        pet,
    }
}

impl LifeTap {
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        let restore = (self.base_amount + fight.player_spirit(self.spirit)) * self.mana_multiplier;
        fight.remove_health(restore);
        fight.add_mana(restore, self.metrics);
        if let Some((share, metrics)) = self.pet {
            fight.add_mana(restore * share, metrics);
        }
    }
}
