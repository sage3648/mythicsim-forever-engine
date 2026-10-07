//! Go sim/core/spelldata/resolve_aura.go: what the client states about a spell's aura.

use std::rc::Rc;

use crate::contracts::prepared_v2::ActionId;
use crate::data::spells::{Effect, Spell};

use super::sim::{AuraConfig, Duration};
use super::spell::DotConfig;

/// Go `AuraOpt`: an addition to the resolved aura for what the client does not state: the label
/// the sim keys the aura by and the callbacks it acts through.
pub(crate) type AuraOpt = Rc<dyn Fn(&mut AuraConfig)>;

/// Go `AuraConfig`: what the client states about a spell's aura. The caller adds the callbacks
/// and anything the client does not carry to the returned value before registering it.
///
/// A row that states no duration resolves to one of 0, which core refuses to activate: such an
/// aura needs `Permanent()` or a duration of the caller's.
pub(crate) fn aura_config(s: &Spell, opts: &[AuraOpt]) -> AuraConfig {
    let mut aura = AuraConfig {
        label: s.name.clone(),
        action_id: Some(ActionId {
            spell_id: s.id,
            ..ActionId::default()
        }),
        duration: s.duration(),
        max_stacks: max_stacks(s),
        ..AuraConfig::default()
    };
    for opt in opts {
        opt(&mut aura);
    }
    aura
}

/// The sim keeps charges and stacks in one field, and so does the client on all but a handful
/// of spells: `CumulativeAura` counts the stacks an aura builds up, `ProcCharges` counts the
/// times it acts before it drops, and a spell that states both is read as a stacking one.
fn max_stacks(s: &Spell) -> i32 {
    if s.max_stack > 0 {
        return i32::from(s.max_stack);
    }
    i32::from(s.proc_charges)
}

/// Go `Label`: two auras of the same spell on one unit need labels of their own.
pub(crate) fn label(label: impl Into<String>) -> AuraOpt {
    let label = label.into();
    Rc::new(move |aura| aura.label = label.clone())
}

/// Go `Permanent`: an aura that is up for the whole iteration, whatever duration the row
/// states (`MakePermanent` on the config).
#[allow(dead_code)]
pub(crate) fn permanent() -> AuraOpt {
    Rc::new(|aura| {
        aura.duration = super::sim::NEVER_EXPIRES;
        let old = aura.on_reset.take();
        aura.on_reset = Some(Rc::new(move |sim, id| {
            sim.aura_mut(id).duration = super::sim::NEVER_EXPIRES;
            if let Some(old) = &old {
                old(sim, id);
            }
            sim.activate(id);
        }));
    })
}

/// Go `DotConfig`: what the client states about a periodic effect of a spell, as the dot core
/// registers. The ticks are the row's duration over the effect's period. The tick handlers only
/// run in a fight and are not carried.
///
/// The aura's duration is the row's, which core recomputes from the ticks on every application.
pub(crate) fn dot_config(s: &Spell, e: &Effect, opts: &[AuraOpt]) -> DotConfig {
    if e.period_ms <= 0 {
        panic!(
            "spelldata: effect {} of spell {} ({}) states no tick period",
            e.index, s.id, s.name
        );
    }
    // DurationMs, not Duration(): the client's -1 is a permanent aura, which reads as a
    // duration longer than any encounter and would resolve to an absurd number of ticks.
    if s.duration_ms <= 0 {
        panic!(
            "spelldata: spell {} ({}) states no duration to tick over",
            s.id, s.name
        );
    }
    let period: Duration = e.period();
    DotConfig {
        aura: aura_config(s, opts),
        tick_length: period,
        number_of_ticks: (s.duration() / period) as i32,
        bonus_coefficient: e.coeff(),
        ..DotConfig::default()
    }
}
