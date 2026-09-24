package priest

import (
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
)

var TalentTreeSizes = [3]int{18, 17, 18}

type Priest struct {
	core.Character
	SelfBuffs
	Talents *proto.PriestTalents

	Latency float64

	ShadowfiendAura *core.Aura
	ShadowfiendPet  *Shadowfiend

	Shadowfiend    *core.Spell
	InnerFocusAura *core.Aura

	VampiricEmbrace *core.Spell

	SearingLightAura  *core.Aura
	ShadowformAura    *core.Aura
	ShadowWeavingAura *core.Aura

	// Every Holy Fire rank the priest knows, so Power in Light can ask whether its dot is running.
	HolyFire []*core.Spell
}

type SelfBuffs struct {
	UseShadowfiend bool
	PreShadowform  bool
	Armor          proto.PriestOptions_Armor
}

func (priest *Priest) GetCharacter() *core.Character {
	return &priest.Character
}
func (priest *Priest) GetPriest() *Priest {
	return priest
}

func (priest *Priest) AddPartyBuffs(_ *proto.PartyBuffs) {
}

// Divine Spirit and Improved Power Word: Fortitude are gone from the Forever trees. Both are raid
// buffs the rest of the raid is built around, so they are assumed to have become baseline.
// TODO: beta will confirm whether they were made baseline or removed outright.
func (priest *Priest) AddRaidBuffs(raidBuffs *proto.RaidBuffs) {
	raidBuffs.PrayerOfShadowProtection = true
	raidBuffs.PrayerOfSpirit = true
	raidBuffs.PrayerOfFortitude = true
}

func (priest *Priest) Initialize() {
	mindblastCDTimer := priest.NewTimer()
	shadowWordDeathCDTimer := priest.NewTimer()

	MindBlastRankMap.Each(func(_ int32, rank *spelldata.Spell) {
		priest.registerMindBlastSpell(rank, mindblastCDTimer)
	})
	ShadowWordPainRankMap.Each(func(_ int32, rank *spelldata.Spell) { priest.registerShadowWordPainSpell(rank) })
	ShadowWordDeathRankMap.Each(func(_ int32, rank *spelldata.Spell) {
		priest.registerShadowWordDeathSpell(rank, shadowWordDeathCDTimer)
	})
	SmiteRankMap.Each(func(_ int32, rank *spelldata.Spell) { priest.registerSmiteSpell(rank) })
	HolyFireRankMap.Each(func(_ int32, rank *spelldata.Spell) { priest.registerHolyFireSpell(rank) })
	priest.registerShadowfiendSpell()

	if priest.Race == proto.Race_RaceNightElf && !priest.RacialsDisabled() {
		starshardsCDTimer := priest.NewTimer()
		StarshardsRankMap.Each(func(_ int32, rank *spelldata.Spell) {
			priest.registerStarshardsSpell(rank, starshardsCDTimer)
		})
	}

	// Devouring Plague is an Undead racial in Classic. The Forever beta client teaches it to priests
	// of every race (SkillLineAbility race mask -1), so it is baseline here.
	devouringPlagueCDTimer := priest.NewTimer()
	DevouringPlagueRankMap.Each(func(_ int32, rank *spelldata.Spell) {
		priest.registerDevouringPlagueSpell(rank, devouringPlagueCDTimer)
	})
}

func (priest *Priest) Reset(_ *core.Simulation) {

}

func (priest *Priest) OnEncounterStart(sim *core.Simulation) {
}

func New(char *core.Character, selfBuffs SelfBuffs, talents string) *Priest {
	priest := &Priest{
		Character: *char,
		SelfBuffs: selfBuffs,
		Talents:   &proto.PriestTalents{},
	}

	core.FillTalentsProto(priest.Talents.ProtoReflect(), talents, TalentTreeSizes)
	priest.EnableManaBar()
	if selfBuffs.Armor == proto.PriestOptions_InnerFire {
		// Inner Fire rank 7: +1580 armor. The charges never run out on a caster that is not hit.
		priest.AddStat(stats.Armor, 1580)
	}
	priest.AddStatDependency(stats.Agility, stats.PhysicalCritPercent, core.CritPerAgiMaxLevel[char.Class])
	priest.ShadowfiendPet = priest.NewShadowfiend()

	return priest
}

// Agent is a generic way to access underlying priest on any of the agents.
type PriestAgent interface {
	GetPriest() *Priest
}

// The outcome a priest dot tick rolls. Every priest dot rolls its hit once, when the spell is cast,
// so a tick only rolls the critical strike the client's Periodic Can Crit attribute allows - Shadow
// Word: Pain, Mind Flay, Holy Fire and Starshards carry it, Devouring Plague does not.
// shared.PeriodicTickOutcome cannot serve: its magic branches roll the hit again on every tick, which
// would charge the miss twice.
func priestTickOutcome(canCrit bool, dot *core.Dot) core.OutcomeApplier {
	if !canCrit {
		return dot.OutcomeTick
	}

	return func(sim *core.Simulation, result *core.SpellResult, attackTable *core.AttackTable) {
		metrics := &dot.Spell.SpellMetrics[result.Target.UnitIndex]
		isPartialResist := result.DidResist()

		if dot.Spell.MagicCritCheck(sim, result.Target) {
			result.Outcome = core.OutcomeCrit
			result.Damage *= dot.Spell.CritDamageMultiplier(attackTable)
			metrics.CritTicks++
			if isPartialResist {
				metrics.ResistedCritTicks++
			}
			return
		}

		result.Outcome = core.OutcomeHit
		metrics.Ticks++
		if isPartialResist {
			metrics.ResistedTicks++
		}
	}
}

func NewPriest(character *core.Character, options *proto.Player) *Priest {
	classOptions := options.GetDpsPriest().GetOptions().GetClassOptions()
	selfBuffs := SelfBuffs{
		UseShadowfiend: classOptions.GetUseShadowfiend(),
		PreShadowform:  classOptions.GetPreShadowform(),
		Armor:          classOptions.GetArmor(),
	}

	basePriest := New(character, selfBuffs, options.TalentsString)
	basePriest.Latency = float64(basePriest.ChannelClipDelay.Milliseconds())

	return basePriest
}

func RegisterPriest() {
	core.RegisterAgentFactory(
		proto.Player_DpsPriest{},
		proto.Spec_SpecDpsPriest,
		func(character *core.Character, options *proto.Player, _ *proto.Raid) core.Agent {
			return NewPriest(character, options)
		},
		func(player *proto.Player, spec interface{}) {
			playerSpec, ok := spec.(*proto.Player_DpsPriest)
			if !ok {
				panic("Invalid spec value for Priest!")
			}
			player.Spec = playerSpec
		},
	)
}

const (
	PriestSpellFlagNone        int64 = 0
	PriestSpellDevouringPlague int64 = 1 << iota
	PriestSpellDevouringPlagueDoT
	PriestSpellDevouringPlagueHeal
	PriestSpellHolyNova
	PriestSpellHolyFire
	PriestSpellMindBlast
	PriestSpellMindFlay
	PriestSpellPenance
	PriestSpellPowerInfusion
	PriestSpellStarshards
	PriestSpellShadowform
	PriestSpellShadowWordDeath
	PriestSpellShadowWordPain
	PriestSpellShadowFiend
	PriestSpellVampiricEmbrace
	PriestSpellFade
	PriestSpellSmite

	// TODO: Forever abilities the sim does not model yet; see the stub file named for each.
	PriestSpellChastise
	PriestSpellConfoundingFlash
	PriestSpellContingencyPlan
	PriestSpellDarkSacrifice
	PriestSpellDivineGrace

	PriestSpellLast
	PriestSpellsAll    = PriestSpellLast<<1 - 1
	PriestSpellDoT     = PriestSpellDevouringPlague | PriestSpellHolyFire | PriestSpellMindFlay | PriestSpellShadowWordPain | PriestSpellStarshards
	PriestShadowSpells = PriestSpellDevouringPlague |
		PriestSpellShadowWordDeath |
		PriestSpellShadowform |
		PriestSpellShadowWordPain |
		PriestSpellMindFlay |
		PriestSpellMindBlast |
		PriestSpellShadowFiend |
		PriestSpellVampiricEmbrace
	PriestHolySpells = PriestSpellSmite | PriestSpellHolyFire | PriestSpellHolyNova | PriestSpellPenance
)
