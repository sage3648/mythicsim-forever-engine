//! Demonic Brand (1293695), from Go sim/warlock/talents_demonology.go `applyDemonicBrand`: a
//! landed Searing Pain brands its target (1293696) with charges, and mirrors them on the
//! summoned demon's marker aura. Each landed direct hit of the demon spends a charge, then
//! casts the demon's brand hit, which cannot miss and adds a share of the warlock's spell
//! power and school power.
//!
//! Go tracks the most recently branded target to mirror its charges on the marker; with the
//! one target in scope, a brand that is active was always set on it.

use crate::core::fight::{Agent, AuraRef, Fight, Outcome, Side, SpellId, SpellResult};

#[derive(Clone, Debug)]
pub(crate) struct DemonicBrand {
    target_aura: AuraRef,
    charges: i32,
    trigger_spells: Vec<SpellId>,
    demon: Option<DemonBrand>,
}

/// The summoned demon's half of the brand.
#[derive(Clone, Debug)]
struct DemonBrand {
    marker: AuraRef,
    spell: SpellId,
    min_damage: f64,
    max_damage: f64,
    coefficient: f64,
    /// The Go school index of the warlock's school power stat, read live at each hit.
    school: usize,
}

/// The demon's half of the exported effect.
pub(crate) struct DemonConfig<'a> {
    pub(crate) marker_aura: &'a str,
    /// Position in the demon's spellbook.
    pub(crate) brand_spell: usize,
    pub(crate) min_damage: f64,
    pub(crate) max_damage: f64,
    pub(crate) coefficient: f64,
    /// The Go school index of the school power stat.
    pub(crate) school: usize,
}

pub(crate) fn bind<A: Agent>(
    fight: &Fight<A>,
    target_aura: &str,
    charges: i32,
    trigger_spells: &[usize],
    demon: Option<DemonConfig>,
    demon_side: Option<Side>,
) -> Result<DemonicBrand, String> {
    let target_aura = unit_aura(fight, Side::Target, target_aura)?;
    let demon = match demon {
        None => None,
        Some(config) => {
            let side = demon_side.ok_or("Demonic Brand's demon is not simulated")?;
            let pet_spells: Vec<SpellId> = (0..fight.spells.len())
                .filter(|&spell| fight.spells[spell].caster == side)
                .collect();
            let spell = *pet_spells
                .get(config.brand_spell)
                .ok_or_else(|| format!("the demon has no spell at {}", config.brand_spell))?;
            Some(DemonBrand {
                marker: unit_aura(fight, side, config.marker_aura)?,
                spell,
                min_damage: config.min_damage,
                max_damage: config.max_damage,
                coefficient: config.coefficient,
                school: config.school,
            })
        }
    };
    Ok(DemonicBrand {
        target_aura,
        charges,
        trigger_spells: trigger_spells.to_vec(),
        demon,
    })
}

fn unit_aura<A: Agent>(fight: &Fight<A>, side: Side, label: &str) -> Result<AuraRef, String> {
    fight.trackers[side.index()]
        .find(label)
        .map(|index| AuraRef { side, index })
        .ok_or_else(|| format!("aura {label} is not registered"))
}

impl DemonicBrand {
    /// The trigger's OnSpellHitDealt: Go `AttachProcTriggerCallback` with `OutcomeLanded`
    /// and `TriggerImmediately`. Without a summoned demon it does nothing.
    pub(crate) fn on_spell_hit_dealt<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        if !self.trigger_spells.contains(&spell) || !result.landed() {
            return;
        }
        let Some(demon) = &self.demon else {
            return;
        };
        fight.activate_aura(self.target_aura);
        fight.set_stacks(self.target_aura, self.charges);
        fight.activate_aura(demon.marker);
        fight.set_stacks(demon.marker, self.charges);
    }

    /// The demon's consumer aura's OnSpellHitDealt: a landed direct hit on a branded target
    /// spends a charge before the brand hit.
    pub(crate) fn on_demon_hit<A: Agent>(
        &self,
        fight: &mut Fight<A>,
        spell: SpellId,
        result: &SpellResult,
    ) {
        let Some(demon) = &self.demon else {
            return;
        };
        if !result.landed() || !fight.spells[spell].direct_proc {
            return;
        }
        if !fight.aura(self.target_aura).active {
            return;
        }
        fight.remove_stack(self.target_aura);
        let stacks = fight.aura(self.target_aura).stacks;
        fight.set_stacks(demon.marker, stacks);
        fight.cast(demon.spell, result.target);
    }

    /// The brand hit's `ApplyEffects`: a roll plus the warlock's spell power share, on
    /// `OutcomeAlwaysHit`. Go reads the spell damage and school power stats at each hit, so a
    /// stat aura such as an on-use trinket's moves them.
    pub(crate) fn brand_hit<A: Agent>(&self, fight: &mut Fight<A>, spell: SpellId, target: Side) {
        let demon = self.demon.as_ref().expect("the brand hit has a demon");
        let spell_power = fight.unit(Side::Player).powers.spell_damage
            + fight.player_school_damage()[demon.school];
        let roll = fight.go_roll(demon.min_damage, demon.max_damage);
        // Go's arm64 build fuses the spell power share into the roll.
        let damage = demon.coefficient.mul_add(spell_power, roll);
        let result = fight.calc_damage_with_outcome(spell, target, damage, Outcome::AlwaysHit);
        fight.deal_damage(spell, result, false);
    }
}
