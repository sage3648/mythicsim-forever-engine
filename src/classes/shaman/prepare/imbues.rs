//! The Shaman's weapon imbues: Go `sim/shaman` `weapon_imbues.go`, with the item swap's: an imbue
//! on the swap set's weapon (`imbueMhSwap`), the callbacks that switch each imbue with the weapon in
//! the hand, and Rockbiter and Flametongue following one weapon alone (mythicsim patch 133).

use std::rc::Rc;

use crate::prepare::aura_helpers::{CallbackMask, HitOutcome, ProcTrigger};
use crate::prepare::buffs::flametongue::disable_flametongue_totem;
use crate::prepare::buffs::generated::WINDFURY_TOTEM;
use crate::prepare::character::constants::CHARACTER_LEVEL;
use crate::prepare::dbcenums;
use crate::prepare::item_swap::{ManagerChance, ManagerMask, SwapCallback, WEAPON_SLOTS};
use crate::prepare::items::slot;
use crate::prepare::shared_items::current_proc_mask_for;
use crate::prepare::sim::{Sim, UnitId, MILLISECOND, NEVER_EXPIRES};
use crate::prepare::spell::{school, DefenseType, ProcMask, SpellConfig, SpellFlag};
use crate::prepare::stats::{Stat, Stats};

use super::spell_data::spell_data;
use super::spells::spell_action;
use super::{flags, masks, Shaman};

const FROSTBRAND_ENCHANT_ID: i32 = 2;
const FLAMETONGUE_ENCHANT_ID: i32 = 5;
const WINDFURY_ENCHANT_ID: i32 = 283;
const ROCKBITER_ENCHANT_ID: i32 = 3021;

impl Shaman {
    /// Go `getWindfuryFixedProcChance`.
    const WINDFURY_FIXED_PROC_CHANCE: f64 = 0.2;

    /// A temp enchant on a hand's weapon: `TempEnchant` of `MainHand()` or `OffHand()`, which
    /// Go reads whether or not a weapon is equipped, and with an item swap
    /// `AddTempEnchant(effectID, slot, false)`.
    fn set_temp_enchant(&self, sim: &mut Sim, unit: UnitId, slot: usize, effect_id: i32) {
        sim.character_mut(unit).equipment[slot].temp_enchant = effect_id;
        sim.item_swap_add_temp_enchant(unit, effect_id, slot, false);
    }

    /// Go `setupItemSwapImbue`: the imbue on the swap set's weapon of a hand.
    fn setup_item_swap_imbue(&self, sim: &mut Sim, unit: UnitId, imbue: &str, imbue_id: i32) {
        if sim.item_swap(unit).is_none() {
            return;
        }
        if self.self_buffs.imbue_mh_swap == imbue {
            sim.item_swap_add_temp_enchant(unit, imbue_id, slot::MAIN_HAND, true);
        }
        if self.self_buffs.imbue_oh_swap == imbue {
            sim.item_swap_add_temp_enchant(unit, imbue_id, slot::OFF_HAND, true);
        }
    }

    /// Go `swapWeaponCarries`: whether a weapon in the hand once the swap set is worn carries the
    /// temporary enchantment (a slot the swap leaves alone keeps the weapon worn at the pull).
    fn swap_weapon_carries(&self, sim: &Sim, unit: UnitId, effect_id: i32) -> bool {
        [slot::MAIN_HAND, slot::OFF_HAND].into_iter().any(|hand| {
            let swapped = sim.unequipped_item(unit, hand).expect("an item swap");
            let item = if swapped.id == 0 {
                &sim.character(unit).equipment[hand]
            } else {
                swapped
            };
            item.temp_enchant == effect_id
        })
    }

    /// Go `newWindfuryAttackSpell`: 439440 (main hand) or 439441 (off hand), a weapon damage
    /// special hit carrying the rank's extra attack power.
    fn new_windfury_attack_spell(&self, sim: &mut Sim, unit: UnitId, is_mh: bool) {
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(if is_mh { 439440 } else { 439441 }),
                spell_school: school::PHYSICAL,
                defense_type: DefenseType::Melee,
                proc_mask: if is_mh {
                    ProcMask::MELEE_MH_SPECIAL
                } else {
                    ProcMask::MELEE_OH_SPECIAL
                },
                flags: SpellFlag::MELEE_METRICS
                    | SpellFlag::PASSIVE_SPELL
                    | SpellFlag::NO_ON_CAST_COMPLETE,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `RegisterWindfuryImbue`.
    pub(super) fn register_windfury_imbue(&self, sim: &mut Sim, unit: UnitId, proc_mask: ProcMask) {
        let swap = sim.item_swap(unit).is_some();
        if proc_mask == ProcMask::UNKNOWN && !swap {
            return;
        }

        let mut mask = ProcMask::UNKNOWN;
        if self.self_buffs.imbue_mh == "WindfuryWeapon" {
            self.set_temp_enchant(sim, unit, slot::MAIN_HAND, WINDFURY_ENCHANT_ID);
            mask = ProcMask(mask.0 | ProcMask::MELEE_MH.0);
        }
        if self.self_buffs.imbue_oh == "WindfuryWeapon" {
            self.set_temp_enchant(sim, unit, slot::OFF_HAND, WINDFURY_ENCHANT_ID);
            mask = ProcMask(mask.0 | ProcMask::MELEE_OH.0);
        }

        self.setup_item_swap_imbue(sim, unit, "WindfuryWeapon", WINDFURY_ENCHANT_ID);

        let current = current_proc_mask_for(sim, unit, |weapon| {
            weapon.temp_enchant == WINDFURY_ENCHANT_ID
        });
        let dpm = Rc::new(
            sim.new_dynamic_weapon_proc_manager(
                unit,
                0.0,
                Self::WINDFURY_FIXED_PROC_CHANCE,
                current,
            )
            .rebuilt_by_swap(
                sim,
                unit,
                ManagerChance::Fixed(Self::WINDFURY_FIXED_PROC_CHANCE),
                ManagerMask::WeaponTempEnchant(WINDFURY_ENCHANT_ID),
            ),
        );

        // The enchant's equip aura (439431 in every rank's SpellItemEnchantment) rolls 20% on
        // landed melee autos and abilities with a 1.5 sec ProcCategoryRecovery and strikes
        // twice with the proc's weapon.
        self.new_windfury_attack_spell(sim, unit, true);
        self.new_windfury_attack_spell(sim, unit, false);
        let aura = sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Windfury Imbue".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                proc_mask: mask,
                is_weapon_proc: true,
                outcome: HitOutcome::LANDED,
                icd: 1500 * MILLISECOND,
                dpm: Some(dpm),
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );

        if mask.matches(ProcMask::MELEE_MH) {
            // Needs to be higher than Windfury Totem priority.
            sim.new_exclusive_effect(
                aura,
                WINDFURY_TOTEM.category,
                false,
                self.windfury_ap_bonus * 2.0,
                None,
                None,
            );
        }
        // RegisterOnItemSwapWithImbue: the trigger is up while a hand carries the imbue.
        sim.register_item_swap_callback(
            unit,
            &WEAPON_SLOTS,
            SwapCallback::ImbueAura {
                aura,
                enchant_id: WINDFURY_ENCHANT_ID,
            },
        );
    }

    /// Go `newFlametongueImbueSpell`: a Flametongue Totem hit is the imbue's spell with the
    /// totem's base damage, so a shaman's carries the imbue's class mask and the shaman spell
    /// flag.
    fn new_flametongue_imbue_spell(&self, sim: &mut Sim, unit: UnitId) {
        let imbue = spell_data().flametongue_weapon_triggered.by_id(16344);
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(imbue.id),
                spell_school: school::FIRE,
                // The damage logs as Flametongue Attack (10444), Magic in SpellCategories; it
                // crits for 1.5x (2.0x with Elemental Fury).
                defense_type: DefenseType::Magic,
                proc_mask: ProcMask::SPELL_DAMAGE_PROC,
                class_spell_mask: masks::FLAMETONGUE_WEAPON,
                flags: SpellFlag::PASSIVE_SPELL | SpellFlag::PROC | flags::SHAMAN_SPELL,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: 0.10000000149,
                ..SpellConfig::default()
            },
        );
    }

    /// Go `RegisterFlametongueImbue`.
    pub(super) fn register_flametongue_imbue(
        &self,
        sim: &mut Sim,
        unit: UnitId,
        proc_mask: ProcMask,
    ) {
        let swap = sim.item_swap(unit).is_some();
        if proc_mask == ProcMask::UNKNOWN && !swap {
            return;
        }

        for (slot, hand, swapped_hand, label, trigger_proc_mask) in [
            (
                slot::MAIN_HAND,
                self.self_buffs.imbue_mh.as_str(),
                self.self_buffs.imbue_mh_swap.as_str(),
                "ItemSlotMainHand",
                ProcMask::MELEE_MH,
            ),
            (
                slot::OFF_HAND,
                self.self_buffs.imbue_oh.as_str(),
                self.self_buffs.imbue_oh_swap.as_str(),
                "ItemSlotOffHand",
                ProcMask::MELEE_OH,
            ),
        ] {
            // Only the swap's weapon carries it: the trigger starts each fight switched off by
            // its swap callback and comes on at the swap.
            let swap_only = if hand == "FlametongueWeapon" {
                false
            } else if swap
                && swapped_hand == "FlametongueWeapon"
                && sim
                    .unequipped_item(unit, slot)
                    .is_some_and(|item| item.id != 0)
            {
                true
            } else {
                continue;
            };
            if !swap_only {
                self.set_temp_enchant(sim, unit, slot, FLAMETONGUE_ENCHANT_ID);
            }
            self.new_flametongue_imbue_spell(sim, unit);
            let aura = sim.make_proc_trigger_aura(
                unit,
                &ProcTrigger {
                    name: format!("Flametongue Imbue {label}"),
                    proc_mask: trigger_proc_mask,
                    is_weapon_proc: true,
                    outcome: HitOutcome::LANDED,
                    callback: CallbackMask::ON_SPELL_HIT_DEALT,
                    trigger_immediately: true,
                    ..ProcTrigger::default()
                },
            );
            sim.register_item_swap_callback(
                unit,
                &[slot],
                SwapCallback::Flametongue { aura, slot },
            );
            // "When applied to main hand, disables any benefit you personally receive from
            // Flametongue Totem" (patch 70). An off-hand Flametongue Weapon leaves the totem on.
            if slot == crate::prepare::items::slot::MAIN_HAND {
                disable_flametongue_totem(sim, aura);
            }
        }

        self.setup_item_swap_imbue(sim, unit, "FlametongueWeapon", FLAMETONGUE_ENCHANT_ID);
    }

    /// Go `newFrostbrandImbueSpell`.
    fn new_frostbrand_imbue_spell(&self, sim: &mut Sim, unit: UnitId) {
        let imbue = spell_data().frostbrand_weapon_triggered.highest();
        sim.register_spell(
            unit,
            SpellConfig {
                action_id: spell_action(imbue.id),
                spell_school: imbue.spell_school(),
                // Frostbrand Attack (25501 / 38617) is Magic in SpellCategories.
                defense_type: DefenseType::Magic,
                class_spell_mask: masks::FROSTBRAND_WEAPON,
                proc_mask: ProcMask::EMPTY,
                flags: SpellFlag::PASSIVE_SPELL | SpellFlag::PROC | flags::SHAMAN_SPELL,
                damage_multiplier: 1.0,
                threat_multiplier: 1.0,
                bonus_coefficient: imbue.damage_effect().coeff(),
                ..SpellConfig::default()
            },
        );
    }

    /// Go `RegisterFrostbrandImbue`.
    pub(super) fn register_frostbrand_imbue(
        &self,
        sim: &mut Sim,
        unit: UnitId,
        proc_mask: ProcMask,
    ) {
        if proc_mask == ProcMask::UNKNOWN && sim.item_swap(unit).is_none() {
            return;
        }

        if self.self_buffs.imbue_mh == "FrostbrandWeapon" {
            self.set_temp_enchant(sim, unit, slot::MAIN_HAND, FROSTBRAND_ENCHANT_ID);
        }
        if self.self_buffs.imbue_oh == "FrostbrandWeapon" {
            self.set_temp_enchant(sim, unit, slot::OFF_HAND, FROSTBRAND_ENCHANT_ID);
        }

        self.setup_item_swap_imbue(sim, unit, "FrostbrandWeapon", FROSTBRAND_ENCHANT_ID);

        // 8 procs a minute, not Classic's 9 (the client stores no rate).
        let current = current_proc_mask_for(sim, unit, |weapon| {
            weapon.temp_enchant == FROSTBRAND_ENCHANT_ID
        });
        let dpm = Rc::new(
            sim.new_dynamic_weapon_proc_manager(unit, 8.0, 0.0, current)
                .rebuilt_by_swap(
                    sim,
                    unit,
                    ManagerChance::Ppm(8.0),
                    ManagerMask::WeaponTempEnchant(FROSTBRAND_ENCHANT_ID),
                ),
        );

        self.new_frostbrand_imbue_spell(sim, unit);

        let aura = sim.make_proc_trigger_aura(
            unit,
            &ProcTrigger {
                name: "Frostbrand Imbue".to_string(),
                callback: CallbackMask::ON_SPELL_HIT_DEALT,
                is_weapon_proc: true,
                outcome: HitOutcome::LANDED,
                dpm: Some(dpm),
                trigger_immediately: true,
                ..ProcTrigger::default()
            },
        );
        sim.register_item_swap_callback(
            unit,
            &WEAPON_SLOTS,
            SwapCallback::ImbueAura {
                aura,
                enchant_id: FROSTBRAND_ENCHANT_ID,
            },
        );
    }

    /// Go `RegisterRockbiterImbue`: Rockbiter Weapon's passive (16313, rank 7) is a standing
    /// melee attack power aura. Elemental Weapons' first effect adds to it, and Spirit Weapons'
    /// second effect turns its -30% threat into +30% while Rockbiter is up. One passive
    /// whatever the hands, so a second Rockbiter weapon adds nothing.
    pub(super) fn register_rockbiter_imbue(
        &self,
        sim: &mut Sim,
        unit: UnitId,
        proc_mask: ProcMask,
    ) {
        let swap = sim.item_swap(unit).is_some();
        if proc_mask == ProcMask::UNKNOWN && !swap {
            return;
        }

        let mut imbued = false;
        if self.self_buffs.imbue_mh == "RockbiterWeapon" {
            self.set_temp_enchant(sim, unit, slot::MAIN_HAND, ROCKBITER_ENCHANT_ID);
            imbued = true;
        }
        if self.self_buffs.imbue_oh == "RockbiterWeapon" {
            self.set_temp_enchant(sim, unit, slot::OFF_HAND, ROCKBITER_ENCHANT_ID);
            imbued = true;
        }

        self.setup_item_swap_imbue(sim, unit, "RockbiterWeapon", ROCKBITER_ENCHANT_ID);

        // With an item swap, the passive follows the weapons in the hand unless both sets carry
        // it.
        let per_weapon =
            swap && imbued != self.swap_weapon_carries(sim, unit, ROCKBITER_ENCHANT_ID);
        if !imbued && !per_weapon {
            return;
        }

        let data = spell_data();
        let imbue = data.rockbiter_weapon_triggered.highest();
        let attack_power = imbue
            .effect(dbcenums::A_MOD_ATTACK_POWER, 0)
            .average(CHARACTER_LEVEL)
            * (1.0
                + data
                    .elemental_weapons
                    .effect_at(1)
                    .fraction_at(self.talent("elemental_weapons")));
        let aura = sim
            .new_temporary_stats_aura(
                unit,
                "Rockbiter Weapon",
                &spell_action(imbue.id),
                Stats::from_pairs(&[(Stat::AttackPower, attack_power)]),
                NEVER_EXPIRES,
            )
            .aura;
        if !per_weapon {
            sim.make_permanent(aura);
            if self.has_talent("spirit_weapons") {
                sim.unit_mut(unit).pseudo_stats.threat_multiplier *=
                    data.spirit_weapons.effect_at(2).multiplier_at(1);
            }
            return;
        }

        if self.has_talent("spirit_weapons") {
            let threat = data.spirit_weapons.effect_at(2).multiplier_at(1);
            sim.apply_on_gain(
                aura,
                Rc::new(move |sim: &mut Sim, _| {
                    sim.unit_mut(unit).pseudo_stats.threat_multiplier *= threat;
                }),
            );
            sim.apply_on_expire(
                aura,
                Rc::new(move |sim: &mut Sim, _| {
                    sim.unit_mut(unit).pseudo_stats.threat_multiplier /= threat;
                }),
            );
        }
        // The item swap's reset runs this with the pull's weapons, so the aura starts each fight
        // as they say.
        sim.register_item_swap_callback(unit, &WEAPON_SLOTS, SwapCallback::Rockbiter { aura });
    }
}
