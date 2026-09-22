package database

import (
	"encoding/json"
	"fmt"
	"log"
	"regexp"
	"strconv"
	"strings"

	"github.com/tailscale/hujson"
	"github.com/wowsims/classic/sim/core/proto"
)

// Which of Wowhead's game databases the scrapers read: "classic" for Classic Era,
// "forever" once wowhead.com/forever carries the beta client's data. It selects the host
// path of every tooltip and gear planner request and the gear planner table names.
var WowheadGame = "classic"

// WowheadUrl builds a nether.wowhead.com URL under the selected game's path.
func WowheadUrl(path string) string {
	return "https://nether.wowhead.com/" + WowheadGame + path
}

// Example db input file: https://nether.wowhead.com/classic/data/gear-planner?dv=100

// ParseWowheadDBFor reads a dump whose page key names a different game than the default.
// Forever's own gear planner publishes under "classicplus"; the Classic Era one is "classic".
func ParseWowheadDBFor(game string, dbContents string) WowheadDatabase {
	was := WowheadGame
	WowheadGame = game
	defer func() { WowheadGame = was }()
	return ParseWowheadDB(dbContents)
}

func ParseWowheadDB(dbContents string) WowheadDatabase {
	var wowheadDB WowheadDatabase

	// Each part looks like 'WH.setPageData("wow.gearPlanner.some.name", {......});'
	parts := strings.Split(dbContents, "WH.setPageData(")

	for _, dbPart := range parts {
		// fmt.Printf("Part len: %d\n", len(dbPart))
		if len(dbPart) < 10 {
			continue
		}
		dbPart = strings.TrimSpace(dbPart)
		dbPart = strings.TrimRight(dbPart, ");")

		if dbPart[0] != '"' {
			continue
		}
		secondQuoteIdx := strings.Index(dbPart[1:], "\"")
		if secondQuoteIdx == -1 {
			continue
		}
		dbName := dbPart[1 : secondQuoteIdx+1]
		// fmt.Printf("DB name: %s\n", dbName)

		commaIdx := strings.Index(dbPart, ",")
		dbContents := dbPart[commaIdx+1:]
		if dbName == "wow.gearPlanner."+WowheadGame+".item" {
			standardized, err := hujson.Standardize([]byte(dbContents)) // Removes invalid JSON, such as trailing commas
			if err != nil {
				log.Fatalf("Failed to standardize json %s\n\n%s\n\n%s", err, dbContents[0:30], dbContents[len(dbContents)-30:])
			}

			err = json.Unmarshal(standardized, &wowheadDB.Items)
			if err != nil {
				log.Fatalf("failed to parse wowhead item db to json %s\n\n%s", err, dbContents[0:30])
			}
		}

		if dbName == "wow.gearPlanner."+WowheadGame+".randomEnchant" {
			standardized, err := hujson.Standardize([]byte(dbContents)) // Removes invalid JSON, such as trailing commas
			if err != nil {
				log.Fatalf("Failed to standardize json %s\n\n%s\n\n%s", err, dbContents[0:30], dbContents[len(dbContents)-30:])
			}

			err = json.Unmarshal(standardized, &wowheadDB.RandomSuffixes)
			if err != nil {
				log.Fatalf("failed to parse wowhead random suffix db to json %s\n\n%s", err, dbContents[0:30])
			}
		}
	}

	fmt.Printf("\n--\nWowhead DB items loaded: %d\n--\n", len(wowheadDB.Items))
	fmt.Printf("\n--\nWowhead DB random suffixes loaded: %d\n--\n", len(wowheadDB.RandomSuffixes))

	return wowheadDB
}

type WowheadDatabase struct {
	Items          map[string]WowheadItem
	RandomSuffixes map[string]WowheadRandomSuffix
}

type WowheadRandomSuffix struct {
	ID    int32        `json:"id"`
	Name  string       `json:"name"`
	Stats WowheadStats `json:"stats"`
}

type WowheadStats struct {
	Armor             int32 `json:"armor"`
	ArmorBonus        int32 `json:"armorbonus"`
	Strength          int32 `json:"str"`
	Agility           int32 `json:"agi"`
	Stamina           int32 `json:"sta"`
	Intellect         int32 `json:"int"`
	Spirit            int32 `json:"spi"`
	SpellPower        int32 `json:"spldmg"`
	ArcanePower       int32 `json:"arcsplpwr"`
	FirePower         int32 `json:"firsplpwr"`
	FrostPower        int32 `json:"frosplpwr"`
	HolyPower         int32 `json:"holsplpwr"`
	NaturePower       int32 `json:"natsplpwr"`
	ShadowPower       int32 `json:"shasplpwr"`
	MeleeCrit         int32 `json:"mlecritstrkpct"`
	MP5               int32 `json:"manargn"`
	AttackPower       int32 `json:"mleatkpwr"`
	RangedAttackPower int32 `json:"rgdatkpwr"`
	Defense           int32 `json:"def"`
	Block             int32 `json:"blockpct"`
	Dodge             int32 `json:"dodgepct"`
	ArcaneResistance  int32 `json:"arcres"`
	FireResistance    int32 `json:"firres"`
	FrostResistance   int32 `json:"frores"`
	NatureResistance  int32 `json:"natres"`
	ShadowResistance  int32 `json:"shares"`
	Healing           int32 `json:"splheal"`

	// Forever's own gear planner renamed several keys and collapsed the split ones. Measured
	// against every item both dumps carry, by tools/data_watch/wh_rating_units.py:
	//
	//   spldmg -> splpwr, mleatkpwr -> atkpwr, def -> defrtng   identical values, pure renames
	//   mlecritstrkpct -> critstrkrtng   x14      dodgepct -> dodgertng   x12
	//   mlehitpct      -> hitrtng        x10      blockpct -> blockrtng   x5
	//   parrypct       -> parryrtng      x15
	//
	// Those factors are retail rating conversions, and this sim's own are all 1 - it stores
	// percentages - so the ratings are divided back down in statsOf rather than carried across.
	// Forever also has one crit and one hit where Classic split them by school, which matches
	// what its talents say: Thundering Strikes now reads "all spells and attacks".
	SpellPowerAlt  int32 `json:"splpwr"`
	AttackPowerAlt int32 `json:"atkpwr"`
	DefenseRating  int32 `json:"defrtng"`
	CritRating     int32 `json:"critstrkrtng"`
	HitRating      int32 `json:"hitrtng"`
	DodgeRating    int32 `json:"dodgertng"`
	BlockRating    int32 `json:"blockrtng"`
	ParryRating    int32 `json:"parryrtng"`
	HasteRating    int32 `json:"hastertng"`
	// Rating per 1% less chance to be dodged or parried.
	ExpertiseRating int32 `json:"exprtng"`

	// Present in the Classic dump but never mapped before, and Forever uses the rating form.
	SpellCrit int32 `json:"splcritstrkpct"`
	MeleeHit  int32 `json:"mlehitpct"`
	SpellHit  int32 `json:"splhitpct"`
	Parry     int32 `json:"parrypct"`
}

// The gear planner writes an item's stat block and a random suffix's with the same keys, so
// both go through here.
// Forever writes crit, hit, dodge, block and parry as retail ratings. This sim's own rating
// constants are all 1 - it works in percentages - so the dump's numbers are divided back down
// here. The factors are measured, not assumed: every item carrying both a Classic percentage
// and the Forever rating gives the same ratio, on 511 items for crit and 222 for hit.
const (
	critRatingPerPercent  = 14.0
	hitRatingPerPercent   = 10.0
	dodgeRatingPerPercent = 12.0
	blockRatingPerPercent = 5.0
	parryRatingPerPercent = 15.0
	// Haste and expertise have no item carrying both a Classic percentage and the Forever
	// rating, so they cannot be measured the way the others were. Taken instead from the
	// official wowsims/forever repo's client-generated constants (CombatRatings), which also
	// give exactly the five factors above - an independent check on those as well.
	hasteRatingPerPercent     = 10.0
	expertiseRatingPerPercent = 10.0 // 2.5 per quarter-percent
)

func statsOf(ws WowheadStats) Stats {
	return Stats{
		proto.Stat_StatArmor:             float64(ws.Armor + ws.ArmorBonus),
		proto.Stat_StatStrength:          float64(ws.Strength),
		proto.Stat_StatAgility:           float64(ws.Agility),
		proto.Stat_StatStamina:           float64(ws.Stamina),
		proto.Stat_StatIntellect:         float64(ws.Intellect),
		proto.Stat_StatSpirit:            float64(ws.Spirit),
		proto.Stat_StatSpellPower:        float64(ws.SpellPower + ws.SpellPowerAlt),
		proto.Stat_StatArcanePower:       float64(ws.ArcanePower),
		proto.Stat_StatFirePower:         float64(ws.FirePower),
		proto.Stat_StatFrostPower:        float64(ws.FrostPower),
		proto.Stat_StatHolyPower:         float64(ws.HolyPower),
		proto.Stat_StatNaturePower:       float64(ws.NaturePower),
		proto.Stat_StatShadowPower:       float64(ws.ShadowPower),
		proto.Stat_StatMeleeCrit:         float64(ws.MeleeCrit) + float64(ws.CritRating)/critRatingPerPercent,
		proto.Stat_StatSpellCrit:         float64(ws.SpellCrit) + float64(ws.CritRating)/critRatingPerPercent,
		proto.Stat_StatMeleeHit:          float64(ws.MeleeHit) + float64(ws.HitRating)/hitRatingPerPercent,
		proto.Stat_StatSpellHit:          float64(ws.SpellHit) + float64(ws.HitRating)/hitRatingPerPercent,
		proto.Stat_StatParry:             float64(ws.Parry) + float64(ws.ParryRating)/parryRatingPerPercent,
		proto.Stat_StatMeleeHaste:        float64(ws.HasteRating) / hasteRatingPerPercent,
		proto.Stat_StatSpellHaste:        float64(ws.HasteRating) / hasteRatingPerPercent,
		proto.Stat_StatExpertise:         float64(ws.ExpertiseRating) / expertiseRatingPerPercent,
		proto.Stat_StatMP5:               float64(ws.MP5),
		proto.Stat_StatAttackPower:       float64(ws.AttackPower + ws.AttackPowerAlt),
		proto.Stat_StatRangedAttackPower: float64(ws.RangedAttackPower),
		proto.Stat_StatDefense:           float64(ws.Defense + ws.DefenseRating),
		proto.Stat_StatBlock:             float64(ws.Block) + float64(ws.BlockRating)/blockRatingPerPercent,
		proto.Stat_StatDodge:             float64(ws.Dodge) + float64(ws.DodgeRating)/dodgeRatingPerPercent,
		proto.Stat_StatArcaneResistance:  float64(ws.ArcaneResistance),
		proto.Stat_StatFireResistance:    float64(ws.FireResistance),
		proto.Stat_StatFrostResistance:   float64(ws.FrostResistance),
		proto.Stat_StatNatureResistance:  float64(ws.NatureResistance),
		proto.Stat_StatShadowResistance:  float64(ws.ShadowResistance),
		proto.Stat_StatHealingPower:      float64(ws.Healing),
	}
}

func (wrs WowheadRandomSuffix) ToProto() *proto.ItemRandomSuffix {
	stats := statsOf(wrs.Stats)

	return &proto.ItemRandomSuffix{
		Id:    wrs.ID,
		Name:  wrs.Name,
		Stats: toSlice(stats),
	}
}

type WowheadItem struct {
	ID      int32  `json:"id"`
	Name    string `json:"name"`
	Icon    string `json:"icon"`
	Version int32  `json:"versionNum"`

	Quality       int32 `json:"quality"`
	Ilvl          int32 `json:"itemLevel"`
	Phase         int32 `json:"contentPhase"`
	RequiresLevel int32 `json:"requiredLevel"`
	// uint32, not uint16: Classic's masks fit in sixteen bits but Forever's do not - it has races
	// Classic never had, and 2097229 on item 4982 is what made the Forever dump fail to parse.
	RaceMask  uint32 `json:"raceMask"`
	ClassMask uint16 `json:"classMask"`

	Stats               WowheadStats `json:"stats"`
	RandomSuffixOptions []int32      `json:"randomEnchants"`

	SourceTypes   []int32             `json:"source"` // 1 = Crafted, 2 = Dropped by, 3 = sold by zone vendor? barely used, 4 = Quest, 5 = Sold by
	SourceDetails []WowheadItemSource `json:"sourcemore"`
}
type WowheadItemSource struct {
	C        int32  `json:"c"`
	Name     string `json:"n"`    // Name of crafting spell
	Icon     string `json:"icon"` // Icon corresponding to the named entity
	EntityID int32  `json:"ti"`   // Crafting Spell ID / NPC ID / ?? / Quest ID
	ZoneID   int32  `json:"z"`    // Only for drop / sold by sources
}

func (wi WowheadItem) ToProto() *proto.UIItem {
	var sources []*proto.UIItemSource
	for i, details := range wi.SourceDetails {
		switch wi.SourceTypes[i] {
		case 1: // Crafted
			// We'll get this from AtlasLoot instead because it can also tell us the profession.
			//sources = append(sources, &proto.UIItemSource{
			//	Source: &proto.UIItemSource_Crafted{
			//		Crafted: &proto.CraftedSource{
			//			SpellId: details.EntityID,
			//		},
			//	},
			//})
		case 2: // Dropped by
			sources = append(sources, &proto.UIItemSource{
				Source: &proto.UIItemSource_Drop{
					Drop: &proto.DropSource{
						NpcId:  details.EntityID,
						ZoneId: details.ZoneID,
					},
				},
			})
		case 3: // Sold by zone vendor? barely used
		case 4: // Quest
			if details.EntityID != 0 {
				sources = append(sources, &proto.UIItemSource{
					Source: &proto.UIItemSource_Quest{
						Quest: &proto.QuestSource{
							Id:   details.EntityID,
							Name: details.Name,
						},
					},
				})
			}
		case 5: // Sold by
			sources = append(sources, &proto.UIItemSource{
				Source: &proto.UIItemSource_SoldBy{
					SoldBy: &proto.SoldBySource{
						NpcId:   details.EntityID,
						NpcName: details.Name,
						ZoneId:  details.ZoneID,
					},
				},
			})
		}
	}

	return &proto.UIItem{
		Id:                  wi.ID,
		Name:                wi.Name,
		Icon:                wi.Icon,
		Ilvl:                wi.Ilvl,
		Phase:               wi.getPhase(),
		FactionRestriction:  wi.getFactionRstriction(),
		ClassAllowlist:      wi.getClassRestriction(),
		Sources:             sources,
		RandomSuffixOptions: wi.RandomSuffixOptions,
	}
}

var SoDVersionRegex = regexp.MustCompile(`115[0-9]+`)

// Get the SoD phase corresponding to the item's version number
// 11500 (1.15.0) = phase 1
// 11501 (1.15.1) = phase 2
// 11502 (1.15.2) = phase 3
// 11503 (1.15.3) = phase 4
// etc.
// Anything else we'll fall back to phase 1
func (wi WowheadItem) getPhase() int32 {
	versionNumStr := strconv.Itoa(int(wi.Version))
	if SoDVersionRegex.MatchString(versionNumStr) && wi.Phase != 0 {
		return wi.Phase
	}

	if wi.Version >= 11500 && wi.Version < 11600 {
		return wi.Version - 11500 + 1
	}

	return 1
}

func (wi WowheadItem) getFactionRstriction() proto.UIItem_FactionRestriction {
	if wi.RaceMask == 77 {
		return proto.UIItem_FACTION_RESTRICTION_ALLIANCE_ONLY
	} else if wi.RaceMask == 178 {
		return proto.UIItem_FACTION_RESTRICTION_HORDE_ONLY
	} else {
		return proto.UIItem_FACTION_RESTRICTION_UNSPECIFIED
	}
}

type ClassMask uint16

const (
	ClassMaskWarrior     ClassMask = 1 << iota
	ClassMaskPaladin               // 2
	ClassMaskHunter                // 4
	ClassMaskRogue                 // 8
	ClassMaskPriest                // 16
	ClassMaskDeathKnight           // 32
	ClassMaskShaman                // 64
	ClassMaskMage                  // 128
	ClassMaskWarlock               // 256
	ClassMaskUnknown               // 512 seemingly unused?
	ClassMaskDruid                 // 1024
)

func (wi WowheadItem) getClassRestriction() []proto.Class {
	classAllowlist := []proto.Class{}
	if wi.ClassMask&uint16(ClassMaskWarrior) != 0 {
		classAllowlist = append(classAllowlist, proto.Class_ClassWarrior)
	}
	if wi.ClassMask&uint16(ClassMaskPaladin) != 0 {
		classAllowlist = append(classAllowlist, proto.Class_ClassPaladin)
	}
	if wi.ClassMask&uint16(ClassMaskHunter) != 0 {
		classAllowlist = append(classAllowlist, proto.Class_ClassHunter)
	}
	if wi.ClassMask&uint16(ClassMaskRogue) != 0 {
		classAllowlist = append(classAllowlist, proto.Class_ClassRogue)
	}
	if wi.ClassMask&uint16(ClassMaskPriest) != 0 {
		classAllowlist = append(classAllowlist, proto.Class_ClassPriest)
	}
	if wi.ClassMask&uint16(ClassMaskDruid) != 0 {
		classAllowlist = append(classAllowlist, proto.Class_ClassDruid)
	}
	if wi.ClassMask&uint16(ClassMaskShaman) != 0 {
		classAllowlist = append(classAllowlist, proto.Class_ClassShaman)
	}
	if wi.ClassMask&uint16(ClassMaskMage) != 0 {
		classAllowlist = append(classAllowlist, proto.Class_ClassMage)
	}
	if wi.ClassMask&uint16(ClassMaskWarlock) != 0 {
		classAllowlist = append(classAllowlist, proto.Class_ClassWarlock)
	}

	return classAllowlist
}

// OverlayStats writes this dump's stats over an item's existing ones, but only the stats it can
// actually express. Replacing the whole slice would zero everything else: the Forever gear
// planner has no key for block value, so 189 items lost theirs the first time this ran.
//
// Within that set it does replace rather than add, which is the point - Forever took Spirit off
// the Valor pieces, and no value-by-value merge could have said so.
func (wi WowheadItem) OverlayStats(existing []float64) []float64 {
	incoming := toSlice(statsOf(wi.Stats))
	// An entry that publishes none of these keys is not an item with no stats, it is an item
	// whose stats this dump did not print - Totem of Infliction carries 50 Armor that neither
	// gear planner mentions. Nothing to say, so say nothing. The cost is that an item Forever
	// stripped to nothing keeps its old stats; the alternative was zeroing ten real ones.
	empty := true
	for _, stat := range foreverExpressible {
		if int(stat) < len(incoming) && incoming[int(stat)] != 0 {
			empty = false
			break
		}
	}
	if empty {
		return existing
	}

	merged := append([]float64(nil), existing...)
	for len(merged) < len(incoming) {
		merged = append(merged, 0)
	}
	for _, stat := range foreverExpressible {
		if int(stat) < len(incoming) {
			merged[int(stat)] = incoming[int(stat)]
		}
	}
	return merged
}

// The stats the gear-planner dumps carry a key for. Anything absent here comes from the item
// tooltips instead and is left alone.
var foreverExpressible = []proto.Stat{
	proto.Stat_StatArmor,
	proto.Stat_StatStrength,
	proto.Stat_StatAgility,
	proto.Stat_StatStamina,
	proto.Stat_StatIntellect,
	proto.Stat_StatSpirit,
	proto.Stat_StatSpellPower,
	proto.Stat_StatArcanePower,
	proto.Stat_StatFirePower,
	proto.Stat_StatFrostPower,
	proto.Stat_StatHolyPower,
	proto.Stat_StatNaturePower,
	proto.Stat_StatShadowPower,
	proto.Stat_StatMeleeCrit,
	proto.Stat_StatSpellCrit,
	proto.Stat_StatMeleeHit,
	proto.Stat_StatSpellHit,
	proto.Stat_StatMP5,
	proto.Stat_StatAttackPower,
	proto.Stat_StatRangedAttackPower,
	proto.Stat_StatDefense,
	proto.Stat_StatBlock,
	proto.Stat_StatDodge,
	proto.Stat_StatParry,
	proto.Stat_StatMeleeHaste,
	proto.Stat_StatSpellHaste,
	proto.Stat_StatExpertise,
	proto.Stat_StatArcaneResistance,
	proto.Stat_StatFireResistance,
	proto.Stat_StatFrostResistance,
	proto.Stat_StatNatureResistance,
	proto.Stat_StatShadowResistance,
	proto.Stat_StatHealingPower,
}
