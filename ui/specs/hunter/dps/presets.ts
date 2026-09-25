import * as PresetUtils from '@app/preset_utils';
import { Debuffs, IndividualBuffs, PartyBuffs, RaidBuffs } from '@generated/proto/buffs';
import { ConsumesSpec, Profession, Race, TristateEffect } from '@generated/proto/common';
import {
	Hunter_Options as HunterOptions,
	HunterOptions_Ammo,
	HunterOptions_PetAttackSpeed,
	HunterOptions_PetType as PetType,
	HunterOptions_QuiverBonus,
} from '@generated/proto/hunter';
import { SavedTalents } from '@generated/proto/ui';

import BeastMasteryAPL from './apls/bm.apl.json';
import MarksmanshipAPL from './apls/mm.apl.json';
import SurvivalAPL from './apls/sv.apl.json';
import SurvivalMeleeAPL from './apls/sv_melee.apl.json';
import LaunchGear from './gear_sets/launch.gear.json';
import P0BisGear from './gear_sets/p0.bis.gear.json';
import P1BisGear from './gear_sets/p1.bis.gear.json';

export const BeastMasteryRotation = PresetUtils.makePresetAPLRotation('Beast Mastery', BeastMasteryAPL);
export const MarksmanshipRotation = PresetUtils.makePresetAPLRotation('Marksmanship', MarksmanshipAPL);
export const SurvivalRotation = PresetUtils.makePresetAPLRotation('Survival', SurvivalAPL);
export const SurvivalMeleeRotation = PresetUtils.makePresetAPLRotation('Survival (Melee)', SurvivalMeleeAPL);
export const DefaultRotation = MarksmanshipRotation;

// Defaults below are what master's ui/hunter (the Forever site before the switch) opens with: its
// page drops the blessings, Fire Resistance Aura and Judgement of Wisdom its presets name.
export const DefaultOptions = HunterOptions.create({
	classOptions: {
		ammo: HunterOptions_Ammo.ThoriumHeadedArrow,
		quiverBonus: HunterOptions_QuiverBonus.Speed15,
		petType: PetType.Cat,
		petAttackSpeed: HunterOptions_PetAttackSpeed.OneTwo,
		petUptime: 1,
	},
});

export const DefaultIndividualBuffs = IndividualBuffs.create({});

export const DefaultPartyBuffs = PartyBuffs.create({
	battleShout: TristateEffect.TristateEffectImproved,
	graceOfAirTotem: true,
	manaSpringTotem: TristateEffect.TristateEffectRegular,
	strengthOfEarthTotem: true,
});

export const DefaultRaidBuffs = RaidBuffs.create({
	arcaneBrilliance: true,
	fireResistanceTotem: true,
	giftOfTheWild: true,
	prayerOfSpirit: true,
});

export const DefaultDebuffs = Debuffs.create({
	curseOfRecklessness: true,
	exposeArmor: true,
	faerieFire: true,
	// A flat 71 ranged attack power: Forever's hunter tree has no Improved Hunter's Mark node.
	huntersMark: true,
	sunderArmor: true,
});

// Master's consumables, as the Forever client's items; Windfury is a totem here, not an imbue.
export const DefaultConsumables = ConsumesSpec.create({
	flaskId: 13512, // Flask of Supreme Power
	battleElixirId: 13452, // Elixir of the Mongoose
	spellPowerElixirId: 13454, // Greater Arcane Elixir
	guardianElixirId: 20007, // Mageblood Elixir
	strengthBuffId: 12451, // Juju Power
	attackPowerBuffId: 12460, // Juju Might
	zanzaId: 8412, // Ground Scorpok Assay
	alcoholId: 21151, // Rumsey Rum Black Label
	dragonbreathChili: true,
	foodId: 20452, // Smoked Desert Dumplings
	potId: 13444, // Major Mana Potion
	conjuredId: 12662, // Demonic Rune
	ohImbueId: 18262, // Elemental Sharpening Stone
});

export const OtherDefaults = {
	reactionTime: 200, // master's default
	distanceFromTarget: 12,
	profession1: Profession.Enchanting,
	profession2: Profession.Engineering,
	race: Race.RaceTroll,
};

export const TalentsP1 = PresetUtils.makePresetTalents('Marksmanship', SavedTalents.create({ talentsString: '5023000501-0050550501503051' }));
export const TalentsBeastMastery = PresetUtils.makePresetTalents('Beast Mastery 35/16/0', SavedTalents.create({ talentsString: '5520001505121251-0050551' }));
export const TalentsMarksmanship = PresetUtils.makePresetTalents('Marksmanship 0/39/12', SavedTalents.create({ talentsString: '-3050552301503151-50024001' }));
export const TalentsSurvival = PresetUtils.makePresetTalents('Survival 0/15/36', SavedTalents.create({ talentsString: '-005055-550230031051220151' }));
export const TalentPresets = [TalentsP1, TalentsBeastMastery, TalentsMarksmanship, TalentsSurvival];

// Our Forever sim's gear presets (master ui/<spec>/gear_sets).
export const GEAR_LAUNCH = PresetUtils.makePresetGear('Launch', LaunchGear);
export const GEAR_P0_BIS = PresetUtils.makePresetGear('Pre-BiS', P0BisGear);
export const GEAR_P1_BIS = PresetUtils.makePresetGear('P1 BiS', P1BisGear);
export const DEFAULT_GEAR = GEAR_P0_BIS;
export const GEAR_PRESETS = [GEAR_LAUNCH, GEAR_P0_BIS, GEAR_P1_BIS];
