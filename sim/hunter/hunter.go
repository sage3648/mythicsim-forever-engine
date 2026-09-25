package hunter

import (
	"time"

	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/stats"
)

const (
	HunterBaseMaxRange         = 35
	ThoridalTheStarsFuryItemID = 34334
	QuiverHasteCategory        = "QuiverHaste"
)

// Marksmanship lost Improved Serpent Sting: node 1091/105003 is parked ten times off the tree's
// canvas, has no live twin, and is missing from the point-spent groups every other tier-4
// Marksmanship node belongs to. See tools/database/tables.go.
var TalentTreeSizes = [3]int{16, 16, 18}

type Hunter struct {
	core.Character

	ClassSpellScaling float64

	Talents *proto.HunterTalents
	Options *proto.HunterOptions

	windFuryEnabled bool

	Pet *HunterPet

	AmmoDPS         float64
	AmmoDamageBonus float64

	AimedShot         *core.Spell
	ArcaneShot        *core.Spell
	AspectOfTheBeast  *core.Spell
	AspectOfTheHawk   *core.Spell
	ExplosiveTrap     *core.Spell
	FreezingTrap      *core.Spell
	ImmolationTrap    *core.Spell
	LaceratingStrikes *core.Spell
	MongooseBite      *core.Spell
	MultiShot         *core.Spell
	RapidFire         *core.Spell
	RaptorStrike      *core.Spell
	RaptorStrikeHit   *core.Spell
	SerpentSting      *core.Spell
	SniperShot        *core.Spell
	StriderKick       *core.Spell
	SummonHawk        *core.Spell
	Volley            *core.Spell
	WingClip          *core.Spell

	AspectOfTheBeastAura *core.Aura
	AspectOfTheHawkAura  *core.Aura
	RapidFireAura        *core.Aura
	TalonOfAlarAura      *core.Aura
	TheBeastWithinAura   *core.Aura
	quiverBonusAura      *core.Aura

	// Mongoose Bite is only castable in the window a dodge opens.
	DefensiveState *core.Aura

	curQueueAura       *core.Aura
	curQueuedAutoSpell *core.Spell
}

func (hunter *Hunter) GetCharacter() *core.Character {
	return &hunter.Character
}

func (hunter *Hunter) GetHunter() *Hunter {
	return hunter
}

func RegisterHunter() {
	core.RegisterAgentFactory(
		proto.Player_Hunter{},
		proto.Spec_SpecHunter,
		func(character *core.Character, options *proto.Player, raid *proto.Raid) core.Agent {
			return NewHunter(character, options, options.GetHunter().Options.ClassOptions, raid)
		},
		func(player *proto.Player, spec interface{}) {
			playerSpec, ok := spec.(*proto.Player_Hunter)
			if !ok {
				panic("Invalid spec value for Hunter!")
			}
			player.Spec = playerSpec
		},
	)
}

func NewHunter(character *core.Character, options *proto.Player, hunterOptions *proto.HunterOptions, _ *proto.Raid) *Hunter {
	hunter := &Hunter{
		Character: *character,
		Talents:   &proto.HunterTalents{},
		Options:   hunterOptions,
	}

	core.FillTalentsProto(hunter.Talents.ProtoReflect(), options.TalentsString, TalentTreeSizes)

	hunter.PseudoStats.CanParry = true

	hunter.EnableManaBar()

	rangedSlot := hunter.GetRangedWeapon()
	hunter.applyAmmoDPS()
	hunter.applyQuiverBonus(rangedSlot)

	rangedWeapon := hunter.WeaponFromRanged()

	if rangedSlot == nil || rangedSlot.ID != ThoridalTheStarsFuryItemID {
		hunter.AmmoDamageBonus = hunter.AmmoDPS * rangedWeapon.SwingSpeed
		rangedWeapon.BaseDamageMin += hunter.AmmoDamageBonus
		rangedWeapon.BaseDamageMax += hunter.AmmoDamageBonus
	}

	hunter.EnableAutoAttacks(hunter, core.AutoAttackOptions{
		Ranged:          rangedWeapon,
		MainHand:        hunter.WeaponFromMainHand(),
		OffHand:         hunter.WeaponFromOffHand(),
		ReplaceMHSwing:  hunter.TryRaptorStrike,
		AutoSwingRanged: true,
		AutoSwingMelee:  true,
	})

	mhConfig := hunter.AutoAttacks.MHConfig()
	applyEffects := mhConfig.ApplyEffects
	mhConfig.ApplyEffects = func(sim *core.Simulation, target *core.Unit, spell *core.Spell) {
		// Emit an "auto delayed" log line whenever the mh auto fired
		// later than it would have in an uncontested rotation. Below 1ms
		// is treated as rounding noise so the common case stays silent.
		delay := hunter.AutoAttacks.MainHandPendingSwingDelay()
		if sim.Log != nil && spell.ActionID.Tag == 1 && delay > time.Millisecond {
			hunter.Log(sim, "%s delayed by %s, was ready at %s", spell.ActionID, delay, sim.CurrentTime-delay)
		}

		applyEffects(sim, target, spell)
	}

	rangedConfig := hunter.AutoAttacks.RangedConfig()
	rangedConfig.MaxRange = HunterBaseMaxRange

	hunter.AddStatDependencies()

	hunter.Pet = hunter.NewHunterPet()

	return hunter
}

var quiverHasteMultipliers = map[proto.HunterOptions_QuiverBonus]float64{
	proto.HunterOptions_Speed10: 1.1,
	proto.HunterOptions_Speed11: 1.11,
	proto.HunterOptions_Speed12: 1.12,
	proto.HunterOptions_Speed13: 1.13,
	proto.HunterOptions_Speed14: 1.14,
	proto.HunterOptions_Speed15: 1.15,
}

var quiverHasteSpellIDs = map[proto.HunterOptions_QuiverBonus]int32{
	proto.HunterOptions_Speed10: 29418,
	proto.HunterOptions_Speed11: 29417,
	proto.HunterOptions_Speed12: 29416,
	proto.HunterOptions_Speed13: 29413,
	proto.HunterOptions_Speed14: 29415,
	proto.HunterOptions_Speed15: 29414,
}

func (hunter *Hunter) applyQuiverBonus(weapon *core.Item) {
	if hunter.Options.QuiverBonus == proto.HunterOptions_QuiverNone {
		return
	}

	isThoridalEquipped := weapon != nil && weapon.ID == ThoridalTheStarsFuryItemID
	buildPhase := core.Ternary(
		isThoridalEquipped,
		core.CharacterBuildPhaseNone,
		core.CharacterBuildPhaseGear)

	multiplier := quiverHasteMultipliers[hunter.Options.QuiverBonus]
	hunter.quiverBonusAura = hunter.RegisterAura(core.Aura{
		Label:      "Haste",
		ActionID:   core.ActionID{SpellID: quiverHasteSpellIDs[hunter.Options.QuiverBonus]},
		Duration:   core.NeverExpires,
		BuildPhase: buildPhase,
		OnGain: func(aura *core.Aura, sim *core.Simulation) {
			hunter.PseudoStats.RangedSpeedMultiplier *= multiplier
		},
		OnExpire: func(aura *core.Aura, sim *core.Simulation) {
			hunter.PseudoStats.RangedSpeedMultiplier /= multiplier
		},
	})

	if !isThoridalEquipped {
		core.MakePermanent(hunter.quiverBonusAura)
	}
}

// Forever is a Classic-era realm, so these are the Classic ammo values, not TBC's.
func (hunter *Hunter) applyAmmoDPS() {
	switch hunter.Options.Ammo {
	case proto.HunterOptions_RazorArrow, proto.HunterOptions_SolidShot:
		hunter.AmmoDPS = 7.5
	case proto.HunterOptions_JaggedArrow, proto.HunterOptions_AccurateSlugs:
		hunter.AmmoDPS = 13
	case proto.HunterOptions_MithrilGyroShot:
		hunter.AmmoDPS = 15
	case proto.HunterOptions_IceThreadedArrow, proto.HunterOptions_IceThreadedBullet:
		hunter.AmmoDPS = 16.5
	case proto.HunterOptions_ThoriumHeadedArrow, proto.HunterOptions_ThoriumShells:
		hunter.AmmoDPS = 17.5
	case proto.HunterOptions_RockshardPellets:
		hunter.AmmoDPS = 18
	case proto.HunterOptions_Doomshot:
		hunter.AmmoDPS = 20
	case proto.HunterOptions_MiniatureCannonBalls:
		hunter.AmmoDPS = 20.5
	}
}

func (hunter *Hunter) RegisterRangedSpell(config core.SpellConfig) *core.Spell {
	if config.MissileSpeed == 0 {
		config.MissileSpeed = 40
	}

	config.MinRange = core.MinRangedRange
	config.MaxRange = HunterBaseMaxRange
	config.Cast.DefaultCast.GCD = core.GCDDefault
	config.Cast.IgnoreHaste = true

	if config.Cast.DefaultCast.CastTime > 0 {
		if config.Cast.ModifyCast == nil {
			config.Cast.ModifyCast = func(sim *core.Simulation, spell *core.Spell, cast *core.Cast) {
				cast.CastTime = spell.CastTime()
			}
		}

		if config.Cast.CastTime == nil {
			config.Cast.CastTime = func(spell *core.Spell) time.Duration {
				return time.Duration(float64(spell.DefaultCast.CastTime) / hunter.TotalRangedHasteMultiplier())
			}
		}
	}

	if config.DamageMultiplier == 0 && config.DamageMultiplierAdditive == 0 {
		config.DamageMultiplier = 1

		if config.ThreatMultiplier == 0 {
			config.ThreatMultiplier = 1
		}
	}

	return hunter.RegisterSpell(config)
}

func (hunter *Hunter) Initialize() {
	hunter.RegisterSpells()
	hunter.addPvpGloves()
}

func (hunter *Hunter) RegisterSpells() {
	// Forever pairs Aimed Shot's cooldown with Multi-Shot rather than Arcane Shot: "Aimed Shot
	// shares its cooldown with Multi-Shot" and the mirror line on Multi-Shot (Xaryu's and Savix's
	// Hunters, 12 September).
	multiShotTimer := hunter.NewTimer()
	arcaneShotTimer := hunter.NewTimer()
	trapTimer := hunter.NewTimer()

	hunter.registerAspects()
	hunter.registerArcaneShotSpell(arcaneShotTimer)
	hunter.registerAimedShotSpell(multiShotTimer)
	hunter.registerMultiShotSpell(multiShotTimer)
	hunter.registerSniperShotSpell()
	hunter.registerSummonHawkSpell(arcaneShotTimer)
	hunter.registerSerpentStingSpell()
	hunter.registerVolleySpell()

	hunter.registerRaptorStrikeSpell()
	hunter.registerMongooseBiteSpell()
	hunter.registerLaceratingStrikesSpell()
	hunter.registerWingClipSpell()
	hunter.registerStriderKickSpell()

	hunter.registerExplosiveTrapSpell(trapTimer)
	hunter.registerImmolationTrapSpell(trapTimer)
	hunter.registerFreezingTrapSpell(trapTimer)

	hunter.registerRapidFireCD()
}

func (hunter *Hunter) AddStatDependencies() {
	hunter.AddStatDependency(stats.Strength, stats.AttackPower, 1)
	hunter.AddStatDependency(stats.Agility, stats.AttackPower, 1)
	// A Classic hunter gets two ranged attack power per agility, not one.
	hunter.AddStatDependency(stats.Agility, stats.RangedAttackPower, 2)
	hunter.AddStatDependency(stats.Agility, stats.PhysicalCritPercent, core.CritPerAgiMaxLevel[hunter.Class])
	hunter.AddStatDependency(stats.Agility, stats.DodgeRating, 1.0/25*core.DodgeRatingPerDodgePercent)
}

func (hunter *Hunter) AddRaidBuffs(raidBuffs *proto.RaidBuffs) {
}

func (hunter *Hunter) AddPartyBuffs(partyBuffs *proto.PartyBuffs) {
	if hunter.Talents.TrueshotAura {
		partyBuffs.TrueshotAura = true
	}

	if partyBuffs.WindfuryTotem {
		hunter.windFuryEnabled = true
	}
}

func (hunter *Hunter) Reset(_ *core.Simulation) {
}

func (hunter *Hunter) OnEncounterStart(sim *core.Simulation) {
}

const (
	HunterSpellFlagsNone int64 = 0
	SpellMaskSpellRanged int64 = 1 << iota
	HunterSpellAutoShot
	HunterSpellAimedShot
	HunterSpellArcaneShot
	HunterSpellAspectOfTheBeast
	HunterSpellAspectOfTheHawk
	HunterSpellAspectOfTheViper
	HunterSpellBestialWrath
	HunterSpellMultiShot
	HunterSpellRapidFire
	HunterSpellRaptorStrike
	HunterSpellRaptorStrikeQueue
	HunterSpellReadiness
	HunterSpellScorpidSting
	HunterSpellSerpentSting
	// No spell sets this bit; kept because the Ashtongue Talisman of Swiftness proc filters on it.
	HunterSpellSteadyShot
	HunterSpellVolley
	HunterPetDamage

	HunterSpellExplosiveTrap
	HunterSpellFreezingTrap
	HunterSpellImmolationTrap
	HunterSpellLaceratingStrikes
	HunterSpellMongooseBite
	HunterSpellSniperShot
	HunterSpellStriderKick
	HunterSpellSummonHawk
	HunterSpellWingClip

	// TODO: Forever pet abilities the sim does not model yet; see the stub file named for each.
	HunterSpellDismember
	HunterSpellDustCloud
	HunterSpellEnchantedFlare
	HunterSpellMine
	HunterSpellPinch
	HunterSpellSavageRend
	HunterSpellSwipe
	HunterSpellTendonRip
	HunterSpellWeb

	HunterSpellsAll = HunterSpellAimedShot |
		HunterSpellArcaneShot | HunterSpellBestialWrath |
		HunterSpellMultiShot | HunterSpellRapidFire |
		HunterSpellRaptorStrike | HunterSpellSerpentSting |
		HunterSpellSniperShot | HunterSpellSummonHawk |
		HunterSpellVolley | HunterSpellMongooseBite |
		HunterSpellStriderKick | HunterSpellWingClip |
		HunterSpellExplosiveTrap | HunterSpellFreezingTrap |
		HunterSpellImmolationTrap

	// The shots and stings in Efficiency's (19416) class mask; Sniper Shot is not in it.
	HunterSpellsShotsAndStings = HunterSpellAimedShot | HunterSpellArcaneShot |
		HunterSpellMultiShot | HunterSpellSerpentSting |
		HunterSpellVolley | HunterSpellSummonHawk

	HunterSpellsMelee = HunterSpellRaptorStrike | HunterSpellMongooseBite |
		HunterSpellStriderKick | HunterSpellWingClip

	HunterSpellsTraps = HunterSpellExplosiveTrap | HunterSpellFreezingTrap |
		HunterSpellImmolationTrap
)

// Agent is a generic way to access underlying hunter on any of the agents.
type HunterAgent interface {
	GetHunter() *Hunter
}
