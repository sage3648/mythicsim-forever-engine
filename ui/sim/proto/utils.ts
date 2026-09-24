import { Player } from '@generated/proto/api';
import { Class, Faction, IndividualBuffs, PartyBuffs, Race, RaidBuffs, Spec, TristateEffect, UnitReference, UnitReference_Type } from '@generated/proto/common';
import { ResourceType } from '@generated/proto/spell';

import { PlayerClasses } from '../player/classes';
import { PlayerClass } from '../player/player_class';
import { PlayerSpec } from '../player/player_spec';
import { PlayerSpecs } from '../player/specs';
import { getEnumValues } from '../utils/collections';
import { sum } from '../utils/math';

export const NUM_SPECS = getEnumValues(Spec).length;

// Converts '1231321-12313123-0' to [13, 16, 0]. TBC talent strings are one run of per-row digits
// per tree, so the interesting number is the per-tree total and not the digits themselves.
export function getTalentTreePoints(talentsString: string): Array<number> {
	const trees = talentsString.split('-');
	if (trees.length == 2) {
		trees.push('0');
	}
	return trees.map(tree => sum([...tree].map(char => parseInt(char) || 0)));
}

export function getTalentPoints(talentsString: string): number {
	return sum(getTalentTreePoints(talentsString));
}

const CLASS_TEXT: Record<string, string> = {
	druid: 'text-class-druid',
	hunter: 'text-class-hunter',
	mage: 'text-class-mage',
	paladin: 'text-class-paladin',
	priest: 'text-class-priest',
	rogue: 'text-class-rogue',
	shaman: 'text-class-shaman',
	warlock: 'text-class-warlock',
	warrior: 'text-class-warrior',
};

/** The class-colour class. `--color-class-*` in `styles/theme/colors.css` is what makes it resolve, so the argument has to be a `getCssScheme` slug and not a display name. */
export function textClassName(className: string): string {
	return CLASS_TEXT[className];
}
export function textClassNameForClass<ClassType extends Class>(playerClass: PlayerClass<ClassType>): string {
	return textClassName(PlayerClasses.getCssScheme(playerClass));
}
export function textClassNameForSpec<SpecType extends Spec>(playerSpec: PlayerSpec<SpecType>): string {
	return textClassNameForClass(PlayerSpecs.getPlayerClass(playerSpec));
}

export const raceToFaction: Record<Race, Faction> = {
	[Race.RaceUnknown]: Faction.Unknown,

	[Race.RaceDraenei]: Faction.Alliance,
	[Race.RaceDwarf]: Faction.Alliance,
	[Race.RaceGnome]: Faction.Alliance,
	[Race.RaceHuman]: Faction.Alliance,
	[Race.RaceNightElf]: Faction.Alliance,
	// The Skyborne choose a side at character creation; each half is its own race.
	[Race.RaceSkyborneHighOrder]: Faction.Alliance,

	[Race.RaceBloodElf]: Faction.Horde,
	[Race.RaceOrc]: Faction.Horde,
	[Race.RaceTauren]: Faction.Horde,
	[Race.RaceTroll]: Faction.Horde,
	[Race.RaceUndead]: Faction.Horde,
	[Race.RaceSkyborneWindshaper]: Faction.Horde,
};

// Returns a copy of playerOptions, with the class field set.
export function getPlayerSpecFromPlayer<SpecType extends Spec>(player: Player): PlayerSpec<SpecType> {
	const specValues = getEnumValues(Spec);
	for (let i = 0; i < specValues.length; i++) {
		const spec = specValues[i] as SpecType;
		let specString = Spec[spec]; // Returns 'SpecBalanceDruid' for BalanceDruid.
		specString = specString.substring('Spec'.length); // 'BalanceDruid'
		specString = specString.charAt(0).toLowerCase() + specString.slice(1); // 'balanceDruid'

		if (player.spec.oneofKind == specString) {
			return PlayerSpecs.fromProto(spec);
		}
	}

	throw new Error('Unable to parse spec from player proto: ' + JSON.stringify(Player.toJson(player), null, 2));
}

export const ADAMANTITE_SHARPENING_STONE_ID = 29453;
export const ADAMANTITE_WEIGHTSTONE_ID = 34340;

// Returns the corrected imbue id for a slot given the equipped weapon's sharp/blunt eligibility.
// Only rewrites the Adamantite sharpening/weightstone pair; all other imbue ids pass through unchanged.
export function adjustWeaponImbueId(imbueId: number, hasSharp: boolean, hasBlunt: boolean): number {
	if (imbueId !== ADAMANTITE_SHARPENING_STONE_ID && imbueId !== ADAMANTITE_WEIGHTSTONE_ID) return imbueId;
	if (hasSharp) return ADAMANTITE_SHARPENING_STONE_ID;
	if (hasBlunt) return ADAMANTITE_WEIGHTSTONE_ID;
	return 0;
}

export function newUnitReference(raidIndex: number): UnitReference {
	return UnitReference.create({
		type: UnitReference_Type.Player,
		index: raidIndex,
	});
}

export function emptyUnitReference(): UnitReference {
	return UnitReference.create();
}

export const orderedResourceTypes: Array<ResourceType> = [
	ResourceType.ResourceTypeHealth,
	ResourceType.ResourceTypeMana,
	ResourceType.ResourceTypeEnergy,
	ResourceType.ResourceTypeRage,
	ResourceType.ResourceTypeComboPoints,
	ResourceType.ResourceTypeFocus,
	ResourceType.ResourceTypeGenericResource,
];

export const AL_CATEGORY_HARD_MODE = 'Hard Mode';
export const AL_CATEGORY_TITAN_RUNE = 'Titan Rune';

export const defaultRaidBuffMajorDamageCooldowns = (_?: Class): Partial<RaidBuffs> => {
	return RaidBuffs.create({
		bloodlust: true,
	});
};

// The buffs every healer gear planner starts with: the caster stat buffs, the two caster totems
// and the blessings. No cooldowns: nothing is simulated, so Bloodlust and the like only mislead.
// Nothing here depends on the class.
export const defaultHealerRaidBuffs = (): RaidBuffs =>
	RaidBuffs.create({
		arcaneBrilliance: true,
		giftOfTheWild: TristateEffect.TristateEffectImproved,
		powerWordFortitude: TristateEffect.TristateEffectImproved,
		divineSpirit: TristateEffect.TristateEffectImproved,
	});

export const defaultHealerPartyBuffs = (): PartyBuffs =>
	PartyBuffs.create({
		manaSpringTotem: TristateEffect.TristateEffectRegular,
		wrathOfAirTotem: TristateEffect.TristateEffectRegular,
	});

export const defaultHealerIndividualBuffs = (): IndividualBuffs =>
	IndividualBuffs.create({
		blessingOfKings: true,
		blessingOfWisdom: true,
		blessingOfLight: true,
	});
