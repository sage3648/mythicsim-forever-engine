//! The effects the Tier 1 class set bonuses of the Forever client share (ItemSets 2098 to 2115,
//! client 1.60.1.70425): the 2 piece hit or haste, the 3 piece defense or expertise stat and the
//! 4 piece damage against one creature type. Each class's bonuses are in its own items module,
//! beside its `CLIENT_ITEM_SETS`, with the tooltip line and the ItemSetSpell row they come from.

use super::character::constants::DEFENSE_RATING_PER_DEFENSE_LEVEL;
use super::sim::{AuraId, Sim};
use super::stats::Stat;

/// The request's creature types the Tier 1 bonuses name, by the SpellEffect's creature type mask
/// (`Misc`: 1 Beast, 4 Demon, 8 Elemental, 32 Undead, 64 Humanoid).
pub(crate) const BEAST: &str = "MobTypeBeast";
pub(crate) const DEMON: &str = "MobTypeDemon";
pub(crate) const ELEMENTAL: &str = "MobTypeElemental";
pub(crate) const UNDEAD: &str = "MobTypeUndead";
pub(crate) const HUMANOID: &str = "MobTypeHumanoid";

/// A set bonus that adds melee attack power or spell damage against one creature type, which the
/// export describes as a `mob_type_bonus` effect when every target is of that type
/// (`export_items::gear_proc_effects`), as it does Ceremonial Mallet's and Courier's Clasp's.
#[derive(Clone, Debug)]
pub(crate) struct VersusBonus {
    /// The set bonus's status aura, which the effect is labelled with.
    pub(crate) aura: AuraId,
    /// The request's creature type (`MobTypeUndead`).
    pub(crate) mob_type: &'static str,
    /// Melee attack power against the type (aura 102, `A_MOD_ATTACK_POWER_VERSUS`); ranged
    /// attacks do not read it.
    pub(crate) melee_attack_power: f64,
    /// Damage of magical spells and effects against the type (aura 59).
    pub(crate) spell_damage: f64,
}

impl Sim {
    /// A set bonus of "Improves your chance to hit by 1.0%": the client row's melee (54) and
    /// spell (55) hit auras, as Go's `setHitPercent` states them.
    pub(crate) fn attach_set_hit(&mut self, aura: AuraId) -> AuraId {
        self.attach_stats_buff(aura, super::forever_item_sets::set_hit_percent(1.0))
    }

    /// A set bonus of "Increases your attack speed and casting speed by 1%": the client row's
    /// melee and ranged haste (342) and casting speed (65) auras. Attack speed is melee and
    /// ranged both, so a Hunter's Auto Shot and ranged haste take it too.
    pub(crate) fn attach_set_haste(&mut self, aura: AuraId) -> AuraId {
        self.attach_multiply_attack_speed(aura, 1.01);
        self.attach_multiply_cast_speed(aura, 1.01)
    }

    /// A set bonus of "Increased Defense +n": the client row's skill aura (30) on Defense.
    pub(crate) fn attach_set_defense(&mut self, aura: AuraId, levels: f64) -> AuraId {
        self.attach_stat_buff(
            aura,
            Stat::DefenseRating,
            levels * DEFENSE_RATING_PER_DEFENSE_LEVEL,
        )
    }

    /// A set bonus of "Reduces the chance for your melee attacks to be Dodged or Parried by
    /// 1.2%": the client row's rating aura (189) of 12 on Expertise, as Justice Battleplate's.
    pub(crate) fn attach_set_expertise(&mut self, aura: AuraId) -> AuraId {
        self.attach_stat_buff(aura, Stat::ExpertiseRating, 12.0)
    }

    /// A set bonus of "+n Attack Power against `creature type`": melee attack power while every
    /// target is of the type, and nothing against any other.
    pub(crate) fn attach_versus_attack_power(
        &mut self,
        aura: AuraId,
        mob_type: &'static str,
        attack_power: f64,
    ) -> AuraId {
        self.versus_bonuses.push(VersusBonus {
            aura,
            mob_type,
            melee_attack_power: attack_power,
            spell_damage: 0.0,
        });
        aura
    }

    /// A set bonus of "Increases damage done by your spells and effects by up to n when fighting
    /// `creature type`": spell damage while every target is of the type, and nothing against any
    /// other.
    pub(crate) fn attach_versus_spell_damage(
        &mut self,
        aura: AuraId,
        mob_type: &'static str,
        spell_damage: f64,
    ) -> AuraId {
        self.versus_bonuses.push(VersusBonus {
            aura,
            mob_type,
            melee_attack_power: 0.0,
            spell_damage,
        });
        aura
    }
}

#[cfg(test)]
mod tests;
