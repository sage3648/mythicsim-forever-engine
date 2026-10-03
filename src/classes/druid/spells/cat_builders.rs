//! The cat's combo point builders, from Go sim/druid/ravage.go, shred.go and claw.go: the
//! rank's flat damage plus main hand weapon damage on the weapon special table, with the
//! spell's weapon multiplier prepared. A landed builder gives a combo point; one that does not
//! land refunds its share of the energy. Ravage needs Prowl, and Ravage and Shred need the
//! druid behind a target it can shred.

use crate::core::fight::{melee::PhysicalOutcome, Fight, Side, SpellId};

use super::super::agent::DruidAgent;

/// Which builder a spell is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Builder {
    Ravage,
    Shred,
    Claw,
}

impl Builder {
    pub(crate) fn parse(kind: &str) -> Option<Builder> {
        match kind {
            "ravage" => Some(Builder::Ravage),
            "shred" => Some(Builder::Shred),
            "claw" => Some(Builder::Claw),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CatBuilders {
    /// Flat damage by spell position, for the builders.
    pub(crate) flat_damage: Vec<Option<f64>>,
    pub(crate) cannot_shred: bool,
}

impl CatBuilders {
    /// `ExtraCastCondition`.
    pub(crate) fn can_cast(&self, fight: &Fight<DruidAgent>, builder: Builder) -> bool {
        let behind = !fight.config.melee.in_front_of_target && !self.cannot_shred;
        match builder {
            Builder::Ravage => {
                let prowl = fight.agent.prowl.as_ref().expect("Ravage needs Prowl");
                fight.aura(prowl.aura).active && behind
            }
            Builder::Shred => behind,
            Builder::Claw => true,
        }
    }

    pub(crate) fn apply(&self, fight: &mut Fight<DruidAgent>, spell: SpellId, target: Side) {
        let flat = self.flat_damage[spell].expect("the builder has flat damage");
        let attack_power = fight.melee_attack_power();
        let base = flat + fight.mh_weapon_damage(attack_power);
        let result = fight.calc_physical_damage(
            spell,
            target,
            base,
            PhysicalOutcome::MeleeWeaponSpecialHitAndCrit { count: true },
        );
        fight.deal_damage(spell, result, false);
        if result.landed() {
            let (_, combo) = fight.spells[spell].energy_metrics.expect("energy cost");
            fight.add_combo_points(1, combo);
        } else {
            fight.issue_refund(spell);
        }
    }
}
