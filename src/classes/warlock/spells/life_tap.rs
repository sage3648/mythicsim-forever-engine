//! Life Tap (11689), from Go sim/warlock/lifetap.go: (base + Spirit) times one plus Improved
//! Life Tap, spent as health and gained as mana. No damage modifier touches either side.
//! Demonic Energies gives the summoned demon a share of the restore.

use crate::{
    contracts::prepared_v2::ActionId,
    core::fight::{Agent, Fight, Side},
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct LifeTap {
    restore: f64,
    metrics: usize,
    /// The demon's share and its mana metrics.
    pet: Option<(f64, usize)>,
}

/// Spirit is fixed during a fight in scope, so the restore is the same every cast.
pub(crate) fn bind<A: Agent>(
    fight: &mut Fight<A>,
    spell_id: i32,
    base_amount: f64,
    mana_multiplier: f64,
    spirit: f64,
    pet_mana_share: f64,
) -> LifeTap {
    let id = ActionId {
        spell_id,
        ..ActionId::default()
    };
    let metrics = fight.new_mana_metrics(id.clone());
    // Go registers each demon's metrics with the spell; only the summoned one is simulated.
    let pet = (pet_mana_share > 0.0 && fight.pet.is_some())
        .then(|| (pet_mana_share, fight.new_mana_metrics_of(Side::Pet, id)));
    LifeTap {
        restore: (base_amount + spirit) * mana_multiplier,
        metrics,
        pet,
    }
}

impl LifeTap {
    pub(crate) fn apply<A: Agent>(&self, fight: &mut Fight<A>) {
        fight.remove_health(self.restore);
        fight.add_mana(self.restore, self.metrics);
        if let Some((share, metrics)) = self.pet {
            fight.add_mana(self.restore * share, metrics);
        }
    }
}
