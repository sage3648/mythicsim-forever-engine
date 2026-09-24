import { ClassicPhase } from '../core/constants/other';
import * as PresetUtils from '../core/preset_utils';
import {
	Conjured,
	Consumes,
	Debuffs,
	FirePowerBuff,
	Flask,
	Food,
	FrostPowerBuff,
	IndividualBuffs,
	ManaRegenElixir,
	Potions,
	Profession,
	RaidBuffs,
	SpellPowerBuff,
	TristateEffect,
	WeaponImbue,
	ZanzaBuff,
} from '../core/proto/common';
import { Mage_Options as MageOptions, Mage_Options_ArmorType as ArmorType } from '../core/proto/mage';
import { SavedTalents } from '../core/proto/ui';
import ArcaneAPL from './apls/forever_arcane.apl.json';
import FireAPL from './apls/forever_fire.apl.json';
import FrostAPL from './apls/forever_frost.apl.json';
import LaunchGearJSON from './gear_sets/launch.gear.json';
import P0BISGear from './gear_sets/p0.bis.gear.json';
import P1BISGear from './gear_sets/p1.bis.gear.json';

///////////////////////////////////////////////////////////////////////////
//                                 Gear Presets
///////////////////////////////////////////////////////////////////////////

export const GearLaunch = PresetUtils.makePresetGear('Launch', LaunchGearJSON);
export const GearP0BIS = PresetUtils.makePresetGear('Pre-BiS', P0BISGear);
export const GearP1BIS = PresetUtils.makePresetGear('P1 BiS', P1BISGear);

export const GearPresets = {
	[ClassicPhase.Phase1]: [GearLaunch, GearP0BIS, GearP1BIS],
};

export const DefaultGear = GearP0BIS;

///////////////////////////////////////////////////////////////////////////
//                                 APL Presets
///////////////////////////////////////////////////////////////////////////

export const APLFrost = PresetUtils.makePresetAPLRotation('Frost', FrostAPL);
export const APLArcane = PresetUtils.makePresetAPLRotation('Arcane', ArcaneAPL);
export const APLFire = PresetUtils.makePresetAPLRotation('Fire', FireAPL);

export const APLPresets = {
	[ClassicPhase.Phase1]: [APLFrost, APLArcane, APLFire],
};

export const DefaultAPL = APLPresets[ClassicPhase.Phase1][0];

///////////////////////////////////////////////////////////////////////////
//                                 Talent Presets
///////////////////////////////////////////////////////////////////////////

// Default talents. Uses the wowhead calculator format, make the talents on
// https://wowhead.com/classic/talent-calc and copy the numbers in the url.

export const TalentsP1Frost = PresetUtils.makePresetTalents('Frost DPS', SavedTalents.create({ talentsString: '0502050030003--055500033100030024' }));
export const TalentsP1Arcane = PresetUtils.makePresetTalents('Arcane DPS', SavedTalents.create({ talentsString: '050215003100311531-2305003202003-' }));
export const TalentsP1Fire = PresetUtils.makePresetTalents('Fire DPS', SavedTalents.create({ talentsString: '0502252000003-23550000130133051-' }));

export const TalentsFire = PresetUtils.makePresetTalents('Fire 0/35/16', SavedTalents.create({ talentsString: '-03552020130133151-005500033' }));
export const TalentsFrost = PresetUtils.makePresetTalents('Frost 14/0/37', SavedTalents.create({ talentsString: '050005013--0555003301001301251' }));
export const TalentsArcane = PresetUtils.makePresetTalents('Arcane 35/0/16', SavedTalents.create({ talentsString: '055005023100311531--005500033' }));

export const TalentPresets = {
	[ClassicPhase.Phase1]: [TalentsP1Frost, TalentsP1Arcane, TalentsP1Fire, TalentsFire, TalentsFrost, TalentsArcane],
};

export const DefaultTalents = TalentPresets[ClassicPhase.Phase1][0];

///////////////////////////////////////////////////////////////////////////
//                                 Options
///////////////////////////////////////////////////////////////////////////

export const DefaultOptions = MageOptions.create({
	armor: ArmorType.MoltenArmor,
});

export const DefaultConsumes = Consumes.create({
	defaultConjured: Conjured.ConjuredDemonicRune,
	defaultPotion: Potions.MajorManaPotion,
	firePowerBuff: FirePowerBuff.ElixirOfFirepower,
	flask: Flask.FlaskOfSupremePower,
	food: Food.FoodRunnTumTuberSurprise,
	frostPowerBuff: FrostPowerBuff.ElixirOfFrostPower,
	mainHandImbue: WeaponImbue.BrilliantWizardOil,
	manaRegenElixir: ManaRegenElixir.MagebloodPotion,

	spellPowerBuff: SpellPowerBuff.GreaterArcaneElixir,
	zanzaBuff: ZanzaBuff.CerebralCortexCompound,
});

export const DefaultRaidBuffs = RaidBuffs.create({
	arcaneBrilliance: true,
	divineSpirit: true,
	giftOfTheWild: TristateEffect.TristateEffectImproved,
	manaSpringTotem: TristateEffect.TristateEffectRegular,
	moonkinAura: true,
});

export const DefaultIndividualBuffs = IndividualBuffs.create({
	blessingOfWisdom: TristateEffect.TristateEffectImproved,
});

// Improved Scorch and Winter's Chill only help the mage that applied them in Forever, so they
// are no longer raid debuffs anyone else supplies.
export const DefaultDebuffs = Debuffs.create({
	judgementOfWisdom: true,
});

export const OtherDefaults = {
	distanceFromTarget: 20,
	profession1: Profession.Alchemy,
	profession2: Profession.Tailoring,
};
