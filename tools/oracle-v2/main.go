// The prepared v2 exporter and full-engine oracle. It runs only against the pinned
// Go engine in scratch storage and is never part of the production simulation path.
//
// prepare resets a real simulation for a RaidSimRequest and exports the resolved state
// Rust needs: stats, every registered spell and aura, timers, major cooldowns, the
// rotation and the parameters of the effects Rust must execute. Go callbacks are not
// exported as numbers. Anything the exporter cannot describe is listed in
// "unrepresented" so Rust rejects the input instead of approximating it.
//
// sim runs the actual Go engine serially and writes its raw result for comparison.
package main

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"reflect"
	"sort"
	"time"
	"unsafe"

	"github.com/wowsims/forever/sim"
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/buffs"
	"github.com/wowsims/forever/sim/core/dbcenums"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/simsignals"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
	"github.com/wowsims/forever/sim/mage"
	"google.golang.org/protobuf/encoding/protojson"
	googleProto "google.golang.org/protobuf/proto"
)

// The reference pin, set at build time from upstream/sources.json with -ldflags -X.
var (
	engineRevision string
	clientBuild    string
)

const schemaVersion = 2

func fail(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func readRequest(path string) (*proto.RaidSimRequest, string) {
	data, err := os.ReadFile(path)
	fail(err)
	request := &proto.RaidSimRequest{}
	fail((protojson.UnmarshalOptions{DiscardUnknown: false}).Unmarshal(data, request))
	if request.SimOptions == nil || request.SimOptions.Iterations <= 0 {
		fail(fmt.Errorf("missing iterations"))
	}
	canonical, err := googleProto.MarshalOptions{Deterministic: true}.Marshal(request)
	fail(err)
	digest := sha256.Sum256(canonical)
	return request, hex.EncodeToString(digest[:])
}

func writeJSON(path string, value any) {
	data, err := json.MarshalIndent(value, "", "  ")
	fail(err)
	fail(os.WriteFile(path, append(data, '\n'), 0644))
}

// Field access for unexported Go state. Reading is safe here: the exporter owns the
// simulation and never writes through these values.
func privateField(object any, name string) reflect.Value {
	value := reflect.ValueOf(object)
	for value.Kind() == reflect.Pointer {
		value = value.Elem()
	}
	field := value.FieldByName(name)
	if !field.IsValid() {
		fail(fmt.Errorf("pinned engine has no field %s", name))
	}
	return field
}

func nanos(d time.Duration) int64 { return int64(d) }

type ActionID struct {
	SpellID int32  `json:"spell_id,omitempty"`
	ItemID  int32  `json:"item_id,omitempty"`
	OtherID string `json:"other_id,omitempty"`
	Tag     int32  `json:"tag,omitempty"`
}

func actionID(id core.ActionID) *ActionID {
	if id.IsEmptyAction() {
		return nil
	}
	out := &ActionID{SpellID: id.SpellID, ItemID: id.ItemID, Tag: id.Tag}
	if id.OtherID != proto.OtherAction_OtherActionNone {
		out.OtherID = id.OtherID.String()
	}
	return out
}

// Timers are shared pointers in Go. Identities are assigned in first-seen order so
// spells that share a cooldown, such as the conjured items, share an identity.
type timerNames struct {
	names map[*core.Timer]string
	next  int
}

func (t *timerNames) name(timer *core.Timer) string {
	if timer == nil {
		return ""
	}
	if name, ok := t.names[timer]; ok {
		return name
	}
	name := fmt.Sprintf("timer-%d", t.next)
	t.next++
	t.names[timer] = name
	return name
}

type Cooldown struct {
	Timer      string `json:"timer"`
	DurationNs int64  `json:"duration_ns"`
}

func cooldown(cd core.Cooldown, timers *timerNames) *Cooldown {
	if cd.Timer == nil {
		return nil
	}
	return &Cooldown{Timer: timers.name(cd.Timer), DurationNs: nanos(cd.Duration)}
}

func flagNames(flags core.SpellFlag) []string {
	names := []string{}
	for bit := 0; bit < 64; bit++ {
		flag := core.SpellFlag(uint64(1) << bit)
		if flags&flag != 0 {
			names = append(names, flag.String())
		}
	}
	return names
}

func procMaskNames(mask core.ProcMask) []string {
	names := []string{}
	for bit := 0; bit < 32; bit++ {
		flag := core.ProcMask(uint32(1) << bit)
		if mask&flag != 0 {
			names = append(names, flag.String())
		}
	}
	return names
}

var schoolNames = []string{"none", "physical", "arcane", "fire", "frost", "holy", "nature", "shadow"}

func schoolValues(values [stats.SchoolLen]float64) map[string]float64 {
	out := map[string]float64{}
	for index, name := range schoolNames {
		out[name] = values[index]
	}
	return out
}

func statValues(values stats.Stats) map[string]float64 {
	out := map[string]float64{}
	for index := 0; index < int(stats.SimStatsLen); index++ {
		out[stats.Stat(index).StatName()] = values[index]
	}
	return out
}

// Mage class masks are Go-internal bit positions. Export the stable class spell name
// instead of the bit so a Go reordering cannot silently change Rust behavior.
var mageClassSpells = []struct {
	mask int64
	name string
}{
	{mage.MageSpellArcaneBlast, "arcane_blast"}, {mage.MageSpellArcaneExplosion, "arcane_explosion"},
	{mage.MageSpellArcanePower, "arcane_power"}, {mage.MageSpellArcaneMissilesCast, "arcane_missiles_cast"},
	{mage.MageSpellArcaneMissilesTick, "arcane_missiles_tick"}, {mage.MageSpellBlastWave, "blast_wave"},
	{mage.MageSpellBlizzard, "blizzard"}, {mage.MageSpellColdSnap, "cold_snap"},
	{mage.MageSpellConeOfCold, "cone_of_cold"}, {mage.MageSpellEvocation, "evocation"},
	{mage.MageSpellFireBlast, "fire_blast"}, {mage.MageSpellFireball, "fireball"},
	{mage.MageSpellFlamestrike, "flamestrike"}, {mage.MageSpellFlamestrikeDot, "flamestrike_dot"},
	{mage.MageSpellFrostArmor, "frost_armor"}, {mage.MageSpellFrostbolt, "frostbolt"},
	{mage.MageSpellFrostNova, "frost_nova"}, {mage.MageSpellIceBarrier, "ice_barrier"},
	{mage.MageSpellIceBlock, "ice_block"}, {mage.MageSpellIceLance, "ice_lance"},
	{mage.MageSpellIgnite, "ignite"}, {mage.MageSpellMageArmor, "mage_armor"},
	{mage.MageSpellManaGems, "mana_gems"}, {mage.MageSpellMoltenArmor, "molten_armor"},
	{mage.MageSpellPresenceOfMind, "presence_of_mind"}, {mage.MageSpellPyroblast, "pyroblast"},
	{mage.MageSpellPyroblastDot, "pyroblast_dot"}, {mage.MageSpellScorch, "scorch"},
	{mage.MageSpellManaGem, "mana_gem"}, {mage.MageSpellCombustion, "combustion"},
	{mage.MageSpellImprovedBlizzard, "improved_blizzard"}, {mage.MageSpellFrostfireBolt, "frostfire_bolt"},
}

func classSpell(mask int64, unrepresented *[]string, id core.ActionID) string {
	if mask == 0 {
		return ""
	}
	for _, entry := range mageClassSpells {
		if entry.mask == mask {
			return entry.name
		}
	}
	*unrepresented = append(*unrepresented, fmt.Sprintf("spell %s has unnamed class mask %d", id, mask))
	return ""
}

type Cost struct {
	Resource                string  `json:"resource"`
	BaseCost                int32   `json:"base_cost"`
	FlatModifier            int32   `json:"flat_modifier"`
	PercentModifier         float64 `json:"percent_modifier"`
	AdditivePercentModifier float64 `json:"additive_percent_modifier"`
}

type Cast struct {
	Cost       float64 `json:"cost"`
	GCDNs      int64   `json:"gcd_ns"`
	GCDMinNs   int64   `json:"gcd_min_ns"`
	CastTimeNs int64   `json:"cast_time_ns"`
	NonEmpty   bool    `json:"non_empty"`
}

type Dot struct {
	Unit                     string  `json:"unit"`
	AuraLabel                string  `json:"aura_label"`
	BaseTickCount            int32   `json:"base_tick_count"`
	BaseTickLengthNs         int64   `json:"base_tick_length_ns"`
	BonusCoefficient         float64 `json:"bonus_coefficient"`
	PeriodicDamageMultiplier float64 `json:"periodic_damage_multiplier"`
	BaseDurationMultiplier   float64 `json:"base_duration_multiplier"`
	BaseDurationFlatNs       int64   `json:"base_duration_flat_ns"`
	AffectedByCastSpeed      bool    `json:"affected_by_cast_speed"`
	AffectedByRealHaste      bool    `json:"affected_by_real_haste"`
	HasteReducesDuration     bool    `json:"haste_reduces_duration"`
	Channeled                bool    `json:"channeled"`
}

type DamageEffect struct {
	Average  float64 `json:"average"`
	Variance float64 `json:"variance"`
}

type Spell struct {
	ActionID                       *ActionID     `json:"action_id"`
	Rank                           int32         `json:"rank"`
	School                         uint8         `json:"school"`
	DefenseType                    string        `json:"defense_type"`
	ProcMask                       []string      `json:"proc_mask"`
	Flags                          []string      `json:"flags"`
	ClassSpell                     string        `json:"class_spell,omitempty"`
	MissileSpeed                   float64       `json:"missile_speed"`
	Cost                           *Cost         `json:"cost"`
	DefaultCast                    Cast          `json:"default_cast"`
	CastKind                       string        `json:"cast_kind"`
	IgnoreHaste                    bool          `json:"ignore_haste"`
	HasExtraCastCondition          bool          `json:"has_extra_cast_condition"`
	HasCastRequirement             bool          `json:"has_cast_requirement"`
	MinRange                       float64       `json:"min_range"`
	MaxRange                       float64       `json:"max_range"`
	MaxCharges                     int           `json:"max_charges"`
	CD                             *Cooldown     `json:"cd"`
	SharedCD                       *Cooldown     `json:"shared_cd"`
	BonusHitPercent                float64       `json:"bonus_hit_percent"`
	BonusCritPercent               float64       `json:"bonus_crit_percent"`
	BonusSpellDamage               float64       `json:"bonus_spell_damage"`
	BonusExpertisePercent          float64       `json:"bonus_expertise_percent"`
	CastTimeMultiplier             float64       `json:"cast_time_multiplier"`
	CdMultiplier                   float64       `json:"cd_multiplier"`
	DamageMultiplier               float64       `json:"damage_multiplier"`
	DamageMultiplierAdditive       float64       `json:"damage_multiplier_additive"`
	DirectDamageMultiplierAdditive float64       `json:"direct_damage_multiplier_additive"`
	CritMultiplierPct              float64       `json:"crit_multiplier_pct"`
	CritMultiplierAdditive         float64       `json:"crit_multiplier_additive"`
	BonusBaseDamage                float64       `json:"bonus_base_damage"`
	BonusCoefficient               float64       `json:"bonus_coefficient"`
	ThreatMultiplier               float64       `json:"threat_multiplier"`
	FlatThreatBonus                float64       `json:"flat_threat_bonus"`
	PushbackResist                 float64       `json:"pushback_resist"`
	Dot                            *Dot          `json:"dot"`
	DamageEffect                   *DamageEffect `json:"damage_effect"`
}

type Aura struct {
	Label           string    `json:"label"`
	Tag             string    `json:"tag,omitempty"`
	ActionID        *ActionID `json:"action_id"`
	ActionIDForProc *ActionID `json:"action_id_for_proc,omitempty"`
	DurationNs      int64     `json:"duration_ns"`
	MaxStacks       int32     `json:"max_stacks"`
	Active          bool      `json:"active"`
	Stacks          int32     `json:"stacks"`
	Callbacks       []string  `json:"callbacks"`
	ICD             *Cooldown `json:"icd"`
	Exclusive       int       `json:"exclusive_effects"`
}

func auraCallbacks(aura *core.Aura) []string {
	out := []string{}
	add := func(set bool, name string) {
		if set {
			out = append(out, name)
		}
	}
	add(aura.OnInit != nil, "on_init")
	add(aura.OnReset != nil, "on_reset")
	add(aura.OnDoneIteration != nil, "on_done_iteration")
	add(aura.OnGain != nil, "on_gain")
	add(aura.OnExpire != nil, "on_expire")
	add(aura.OnStacksChange != nil, "on_stacks_change")
	add(aura.OnApplyEffects != nil, "on_apply_effects")
	add(aura.OnCastComplete != nil, "on_cast_complete")
	add(aura.OnSpellHitDealt != nil, "on_spell_hit_dealt")
	add(aura.OnSpellHitTaken != nil, "on_spell_hit_taken")
	add(aura.OnPeriodicDamageDealt != nil, "on_periodic_damage_dealt")
	add(aura.OnPeriodicDamageTaken != nil, "on_periodic_damage_taken")
	add(aura.OnHealDealt != nil, "on_heal_dealt")
	add(aura.OnHealTaken != nil, "on_heal_taken")
	add(aura.OnPeriodicHealDealt != nil, "on_periodic_heal_dealt")
	add(aura.OnPeriodicHealTaken != nil, "on_periodic_heal_taken")
	add(aura.OnEncounterStart != nil, "on_encounter_start")
	return out
}

func exportAuras(unit *core.Unit, timers *timerNames) []Aura {
	out := []Aura{}
	for _, aura := range unit.GetAuras() {
		var icd *Cooldown
		if aura.Icd != nil {
			icd = cooldown(*aura.Icd, timers)
		}
		out = append(out, Aura{
			Label: aura.Label, Tag: aura.Tag, ActionID: actionID(aura.ActionID),
			ActionIDForProc: actionID(aura.ActionIDForProc), DurationNs: nanos(aura.Duration),
			MaxStacks: aura.MaxStacks, Active: aura.IsActive(), Stacks: aura.GetStacks(),
			Callbacks: auraCallbacks(aura), ICD: icd, Exclusive: len(aura.ExclusiveEffects),
		})
	}
	return out
}

type PseudoStats struct {
	SpellCostPercentModifier    int32              `json:"spell_cost_percent_modifier"`
	CastSpeedMultiplier         float64            `json:"cast_speed_multiplier"`
	SpiritRegenRateCasting      float64            `json:"spirit_regen_rate_casting"`
	ForceFullSpiritRegen        bool               `json:"force_full_spirit_regen"`
	SpiritRegenMultiplier       float64            `json:"spirit_regen_multiplier"`
	ThreatMultiplier            float64            `json:"threat_multiplier"`
	DamageDealtMultiplier       float64            `json:"damage_dealt_multiplier"`
	SchoolDamageDealtMultiplier map[string]float64 `json:"school_damage_dealt_multiplier"`
	DotDamageMultiplierAdditive float64            `json:"dot_damage_multiplier_additive"`
	CritDamageMultiplier        float64            `json:"crit_damage_multiplier"`
	DamageTakenMultiplier       float64            `json:"damage_taken_multiplier"`
	SchoolDamageTakenMultiplier map[string]float64 `json:"school_damage_taken_multiplier"`
	SchoolBonusSpellDamage      map[string]float64 `json:"school_bonus_spell_damage"`
	SchoolBonusHitChance        map[string]float64 `json:"school_bonus_hit_chance"`
	BonusSpellDamageTaken       float64            `json:"bonus_spell_damage_taken"`
	BonusSpellCritPercentTaken  float64            `json:"bonus_spell_crit_percent_taken"`
	ReducedCritTakenPercent     float64            `json:"reduced_crit_taken_percent"`
	Incapacitated               bool               `json:"incapacitated"`
}

func exportPseudo(p stats.PseudoStats) PseudoStats {
	return PseudoStats{
		SpellCostPercentModifier: p.SpellCostPercentModifier, CastSpeedMultiplier: p.CastSpeedMultiplier,
		SpiritRegenRateCasting: p.SpiritRegenRateCasting, ForceFullSpiritRegen: p.ForceFullSpiritRegen,
		SpiritRegenMultiplier: p.SpiritRegenMultiplier, ThreatMultiplier: p.ThreatMultiplier,
		DamageDealtMultiplier:       p.DamageDealtMultiplier,
		SchoolDamageDealtMultiplier: schoolValues(p.SchoolDamageDealtMultiplier),
		DotDamageMultiplierAdditive: p.DotDamageMultiplierAdditive, CritDamageMultiplier: p.CritDamageMultiplier,
		DamageTakenMultiplier:       p.DamageTakenMultiplier,
		SchoolDamageTakenMultiplier: schoolValues(p.SchoolDamageTakenMultiplier),
		SchoolBonusSpellDamage:      schoolValues(p.SchoolBonusSpellDamage),
		SchoolBonusHitChance:        schoolValues(p.SchoolBonusHitChance),
		BonusSpellDamageTaken:       p.BonusSpellDamageTaken, BonusSpellCritPercentTaken: p.BonusSpellCritPercentTaken,
		ReducedCritTakenPercent: p.ReducedCritTakenPercent, Incapacitated: p.Incapacitated,
	}
}

type AttackTable struct {
	BaseSpellMissChance   float64 `json:"base_spell_miss_chance"`
	SpellCritSuppression  float64 `json:"spell_crit_suppression"`
	BonusSpellCritPercent float64 `json:"bonus_spell_crit_percent"`
	CritMultiplier        float64 `json:"crit_multiplier"`
	DamageDealtMultiplier float64 `json:"damage_dealt_multiplier"`
	DamageTakenMultiplier float64 `json:"damage_taken_multiplier"`
}

type MajorCooldown struct {
	ActionID *ActionID `json:"action_id"`
	Priority int32     `json:"priority"`
	Type     []string  `json:"type"`
	Timings  []int64   `json:"timings_ns"`
}

func cooldownTypeNames(t core.CooldownType) []string {
	names := []string{}
	for _, entry := range []struct {
		t    core.CooldownType
		name string
	}{{core.CooldownTypeMana, "mana"}, {core.CooldownTypeDPS, "dps"}, {core.CooldownTypeExplosive, "explosive"}, {core.CooldownTypeSurvival, "survival"}} {
		if t.Matches(entry.t) {
			names = append(names, entry.name)
		}
	}
	return names
}

type Mana struct {
	Max                      float64 `json:"max"`
	Base                     float64 `json:"base"`
	SpiritRegenPerSecond     float64 `json:"spirit_regen_per_second"`
	RegenPerSecondCasting    float64 `json:"regen_per_second_casting"`
	RegenPerSecondNotCasting float64 `json:"regen_per_second_not_casting"`
	// The lowest maximum mana while Go deactivates every aura at the end of a fight.
	// Each Mana change clamps current mana, and time to OOM reads it afterwards.
	TeardownMax float64 `json:"teardown_max"`
}

// An action the metrics list for a unit, from Spell.doneIteration and addSpellMetrics.
type MetricsAction struct {
	ActionID     *ActionID `json:"action_id"`
	MeleeMetrics bool      `json:"melee_metrics"`
	School       uint8     `json:"school"`
}

type TargetUnit struct {
	Unit
	// Go AutoAttacks.reset rolls an opening swing offset for an enemy with a melee swing,
	// even when it never swings because no unit tanks it.
	AutoSwingMelee  bool `json:"auto_swing_melee"`
	AutoSwingRanged bool `json:"auto_swing_ranged"`
	// Registered actions the target reports with zero metrics, since it never acts.
	MetricsActions []MetricsAction `json:"metrics_actions"`
}

type Unit struct {
	Index       int32              `json:"index"`
	Label       string             `json:"label"`
	Level       int32              `json:"level"`
	MobType     string             `json:"mob_type,omitempty"`
	Stats       map[string]float64 `json:"stats"`
	PseudoStats PseudoStats        `json:"pseudo_stats"`
	Auras       []Aura             `json:"auras"`
}

type Player struct {
	Unit
	Name               string           `json:"name"`
	Class              string           `json:"class"`
	Race               string           `json:"race"`
	Professions        []string         `json:"professions"`
	TalentsString      string           `json:"talents_string"`
	Talents            map[string]int32 `json:"talents"`
	ClassOptions       json.RawMessage  `json:"class_options"`
	ReactionNs         int64            `json:"reaction_ns"`
	ChannelClipDelayNs int64            `json:"channel_clip_delay_ns"`
	DistanceYards      float64          `json:"distance_yards"`
	CastSpeed          float64          `json:"cast_speed"`
	Mana               Mana             `json:"mana"`
	AttackTable        AttackTable      `json:"attack_table"`
	Spells             []Spell          `json:"spells"`
	MajorCooldowns     []MajorCooldown  `json:"major_cooldowns"`
	Rotation           json.RawMessage  `json:"rotation"`
}

type Encounter struct {
	DurationNs          int64   `json:"duration_ns"`
	DurationVariationNs int64   `json:"duration_variation_ns"`
	ExecuteProportion20 float64 `json:"execute_proportion_20"`
	ExecuteProportion25 float64 `json:"execute_proportion_25"`
	ExecuteProportion35 float64 `json:"execute_proportion_35"`
	ExecuteProportion45 float64 `json:"execute_proportion_45"`
	ExecuteProportion90 float64 `json:"execute_proportion_90"`
}

type SimOptions struct {
	Iterations          int32 `json:"iterations"`
	Seed                int64 `json:"seed"`
	LabeledRng          bool  `json:"labeled_rng"`
	DebugFirstIteration bool  `json:"debug_first_iteration"`
	// Go logs every iteration into one buffer, as the application's averaged timeline uses.
	Debug bool `json:"debug"`
}

type Reference struct {
	EngineRevision string `json:"engine_revision"`
	ClientBuild    string `json:"client_build"`
	Exporter       string `json:"exporter"`
}

type Prepared struct {
	SchemaVersion int              `json:"schema_version"`
	Contract      string           `json:"contract"`
	Reference     Reference        `json:"reference"`
	RequestSHA256 string           `json:"request_sha256"`
	ScenarioID    string           `json:"scenario_id"`
	Sim           SimOptions       `json:"sim"`
	Encounter     Encounter        `json:"encounter"`
	Target        TargetUnit       `json:"target"`
	Player        Player           `json:"player"`
	Effects       []map[string]any `json:"effects"`
	Unrepresented []string         `json:"unrepresented"`
}

func exportSpell(spell *core.Spell, target *core.Unit, timers *timerNames, unrepresented *[]string) Spell {
	var cost *Cost
	if spell.Cost != nil {
		resource := "unknown"
		switch spell.Cost.ResourceCostImpl.(type) {
		case *core.ManaCost:
			resource = "mana"
		default:
			*unrepresented = append(*unrepresented, fmt.Sprintf("spell %s has a non-mana cost", spell.ActionID))
		}
		cost = &Cost{Resource: resource, BaseCost: spell.Cost.BaseCost, FlatModifier: spell.Cost.FlatModifier,
			PercentModifier: spell.Cost.PercentModifier, AdditivePercentModifier: spell.Cost.AdditivePercentModifier}
	}

	// Go chooses the cast function at registration from the default cast, extra condition,
	// cooldowns and cast requirement. Static modifiers never empty a non-empty default cast.
	hasRequirement := privateField(spell, "hasCastRequirement").Bool()
	castKind := "full"
	if spell.DefaultCast == (core.Cast{}) {
		if spell.ExtraCastCondition == nil && spell.CD.Timer == nil && spell.SharedCD.Timer == nil && !hasRequirement {
			castKind = "autos_or_procs"
		} else {
			castKind = "simple"
		}
	}

	var dot *Dot
	dots := privateField(spell, "dots")
	aoeDot := privateField(spell, "aoeDot")
	describe := func(pointer unsafe.Pointer, unit string) *Dot {
		d := (*core.Dot)(pointer)
		return &Dot{
			Unit: unit, AuraLabel: d.Aura.Label, BaseTickCount: d.BaseTickCount,
			BaseTickLengthNs: nanos(d.BaseTickLength), BonusCoefficient: d.BonusCoefficient,
			PeriodicDamageMultiplier: d.PeriodicDamageMultiplier, BaseDurationMultiplier: d.BaseDurationMultiplier,
			BaseDurationFlatNs:   nanos(d.BaseDurationFlat),
			AffectedByCastSpeed:  privateField(d, "affectedByCastSpeed").Bool(),
			AffectedByRealHaste:  privateField(d, "affectedByRealHaste").Bool(),
			HasteReducesDuration: privateField(d, "hasteReducesDuration").Bool(),
			Channeled:            privateField(d, "isChanneled").Bool(),
		}
	}
	if !aoeDot.IsNil() {
		dot = describe(aoeDot.UnsafePointer(), "self")
	} else if dots.Len() > 0 {
		entry := dots.Index(int(target.UnitIndex))
		if entry.IsNil() {
			*unrepresented = append(*unrepresented, fmt.Sprintf("spell %s has a dot on another unit", spell.ActionID))
		} else {
			dot = describe(entry.UnsafePointer(), "target")
		}
	}

	return Spell{
		ActionID: actionID(spell.ActionID), Rank: spell.Rank, School: uint8(spell.SpellSchool),
		DefenseType: spell.DefenseType.String(), ProcMask: procMaskNames(spell.ProcMask), Flags: flagNames(spell.Flags),
		ClassSpell: classSpell(spell.ClassSpellMask, unrepresented, spell.ActionID), MissileSpeed: spell.MissileSpeed,
		Cost: cost,
		DefaultCast: Cast{Cost: spell.DefaultCast.Cost, GCDNs: nanos(spell.DefaultCast.GCD), GCDMinNs: nanos(spell.DefaultCast.GCDMin),
			CastTimeNs: nanos(spell.DefaultCast.CastTime), NonEmpty: spell.DefaultCast.NonEmpty},
		CastKind: castKind, IgnoreHaste: spell.IgnoreHaste, HasExtraCastCondition: spell.ExtraCastCondition != nil,
		HasCastRequirement: hasRequirement, MinRange: spell.MinRange, MaxRange: spell.MaxRange, MaxCharges: spell.MaxCharges,
		CD: cooldown(spell.CD, timers), SharedCD: cooldown(spell.SharedCD, timers),
		BonusHitPercent: spell.BonusHitPercent, BonusCritPercent: spell.BonusCritPercent, BonusSpellDamage: spell.BonusSpellDamage,
		BonusExpertisePercent: spell.BonusExpertisePercent, CastTimeMultiplier: spell.CastTimeMultiplier, CdMultiplier: spell.CdMultiplier,
		DamageMultiplier: spell.DamageMultiplier, DamageMultiplierAdditive: spell.DamageMultiplierAdditive,
		DirectDamageMultiplierAdditive: spell.DirectDamageMultiplierAdditive, CritMultiplierPct: spell.CritMultiplierPct,
		CritMultiplierAdditive: spell.CritMultiplierAdditive, BonusBaseDamage: spell.BonusBaseDamage, BonusCoefficient: spell.BonusCoefficient,
		ThreatMultiplier: spell.ThreatMultiplier, FlatThreatBonus: spell.FlatThreatBonus, PushbackResist: spell.PushbackResist, Dot: dot,
	}
}

// Mage spell rows whose ApplyEffects roll a client damage effect. The ladders and the
// rank pairing mirror sim/mage/frostbolt.go, ice_lance.go and arcane_missiles.go.
var (
	frostboltLadder     = spelldata.Ranked(116, 205, 837, 7322, 8406, 8407, 8408, 10179, 10180, 10181, 25304)
	iceLanceLadder      = spelldata.Ranked(1312002, 400640, 1240044, 1240045, 1240046, 1240047)
	missilesLadder      = spelldata.Ranked(5143, 5144, 5145, 8416, 8417, 10211, 10212, 25345)
	missileTicksLadder  = spelldata.Ranked(7268, 7269, 7270, 8419, 8418, 10273, 10274, 25346)
	arcaneBlastLadder   = spelldata.Ranked(400574, 1239696, 1239697, 1239699, 1239700)
	arcaneBlastBuff     = spelldata.Ranked(400573)
	arcanePower         = spelldata.Ranked(12042)
	presenceOfMind      = spelldata.Ranked(12043)
	igniteTriggered     = spelldata.Ranked(412538)
	fireBlastLadder     = spelldata.Ranked(2136, 2137, 2138, 8412, 8413, 10197, 10199)
	scorchLadder        = spelldata.Ranked(2948, 8444, 8445, 8446, 10205, 10206, 10207)
	improvedScorch      = spelldata.Talent(11095, 3)
	fireVulnerability   = spelldata.Ranked(22959)
	masterOfElements    = spelldata.Talent(29074, 3)
	fireballLadder      = spelldata.Ranked(133, 143, 145, 3140, 8400, 8401, 8402, 10148, 10149, 10150, 10151, 25306)
	arcaneConcentration = spelldata.Talent(11213, 5)
	clearcastingTrigger = spelldata.Ranked(12536)
	fingersOfFrost      = spelldata.Talent(400647, 2)
	fingersTriggered    = spelldata.Ranked(400669)
	shatter             = spelldata.Talent(11170, 3)
	wintersChill        = spelldata.Talent(11180, 5)
	wintersChillTrigger = spelldata.Ranked(12579)
	evocationLadder     = spelldata.Ranked(12051)
	agateTriggered      = spelldata.Ranked(5405)
	jadeTriggered       = spelldata.Ranked(10052)
	citrineTriggered    = spelldata.Ranked(10057)
	rubyTriggered       = spelldata.Ranked(10058)
)

func damageEffect(row *spelldata.Spell) *DamageEffect {
	effect := row.DamageEffect()
	if effect == nil {
		fail(fmt.Errorf("spell %d has no damage effect", row.ID))
	}
	return &DamageEffect{Average: effect.Average(core.CharacterLevel), Variance: effect.Variance}
}

func attachDamageEffects(spells []Spell, character *core.Character) {
	rows := map[int32]*spelldata.Spell{}
	frostboltLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	missileTicksLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	if ice := iceLanceLadder.Highest(); ice != nil {
		rows[ice.ID] = ice
	}
	if blast := arcaneBlastLadder.Highest(); blast != nil {
		rows[blast.ID] = blast
	}
	if fireBlast := fireBlastLadder.Highest(); fireBlast != nil {
		rows[fireBlast.ID] = fireBlast
	}
	scorchLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	fireballLadder.Each(func(_ int32, row *spelldata.Spell) { rows[row.ID] = row })
	for i := range spells {
		id := spells[i].ActionID
		if id == nil || id.SpellID == 0 || id.Tag != 0 {
			continue
		}
		if row, ok := rows[id.SpellID]; ok {
			spells[i].DamageEffect = damageEffect(row)
		}
	}
}

func hasAura(unit *core.Unit, label string) bool { return unit.GetAura(label) != nil }

// Effects whose parameters live in Go closures. Each formula mirrors the cited Go file
// at the pinned revision; Rust reads these values rather than client tables.
func mageEffects(m *mage.Mage, character *core.Character) []map[string]any {
	talents := m.Talents
	effects := []map[string]any{}
	unit := &character.Unit
	if talents.ArcaneConcentration > 0 { // talents_arcane.go registerArcaneConcentration
		effects = append(effects, map[string]any{
			"kind": "arcane_concentration", "talent_rank": talents.ArcaneConcentration,
			"proc_chance": arcaneConcentration.EffectAt(1).FractionAt(talents.ArcaneConcentration),
			"icd_ns":      nanos(arcaneConcentration.Highest().ICD()), "trigger_aura": "Arcane Concentration",
			"aura": "Clearcasting", "aura_duration_ns": nanos(clearcastingTrigger.Highest().Duration()),
		})
	}
	if talents.MissileBarrage { // talents_arcane.go registerMissileBarrage: Go literals
		effects = append(effects, map[string]any{
			"kind": "missile_barrage", "trigger_aura": "Missile Barrage Trigger", "aura": "Missile Barrage",
			"arcane_blast_chance": 0.40, "bolt_chance": 0.20, "rng_label": "Missile Barrage",
			"cost_percent_add": -1.0, "tick_length_delta_ns": nanos(-500 * time.Millisecond),
		})
	}
	if talents.FingersOfFrost > 0 { // talents_frost.go registerFingersOfFrost
		effects = append(effects, map[string]any{
			"kind": "fingers_of_frost", "talent_rank": talents.FingersOfFrost, "shatter_rank": talents.Shatter,
			"proc_chance":  fingersOfFrost.EffectAt(2).FractionAt(talents.FingersOfFrost),
			"max_stacks":   int32(fingersOfFrost.EffectAt(1).ValueAt(talents.FingersOfFrost)),
			"shatter_crit": shatter.ValueAt(talents.Shatter), "duration_ns": nanos(fingersTriggered.Highest().Duration()),
			"trigger_aura": "Fingers of Frost Trigger", "aura": "Fingers of Frost",
		})
	}
	if talents.WintersChill > 0 { // talents_frost.go registerWinterChill
		effects = append(effects, map[string]any{
			"kind": "winters_chill", "talent_rank": talents.WintersChill,
			"proc_chance":    wintersChill.EffectAt(2).FractionAt(talents.WintersChill),
			"max_stacks":     int32(wintersChill.EffectAt(1).ValueAt(talents.WintersChill)),
			"crit_per_stack": wintersChillTrigger.Highest().EffectN(1).Average(core.CharacterLevel),
			"duration_ns":    nanos(wintersChillTrigger.Highest().Duration()),
			"trigger_aura":   "Winters Chill Talent", "aura": "Winter's Chill",
		})
	}
	if talents.IceLance { // ice_lance.go
		effects = append(effects, map[string]any{
			"kind": "ice_lance", "spell_id": iceLanceLadder.Highest().ID, "frozen_multiplier": mage.IceLanceFrozenMultiplier,
		})
	}
	if talents.ColdSnap { // cold_snap.go
		effects = append(effects, map[string]any{"kind": "cold_snap", "spell_id": spelldata.Ranked(12472).Highest().ID})
	}
	// evocation.go
	evocation := evocationLadder.Highest()
	effects = append(effects, map[string]any{
		"kind": "evocation", "spell_id": evocation.ID, "regen_aura": "Evocation Regen", "channel_aura": "Evocation",
		"regen_multiplier": evocation.Effect(dbcenums.A_MOD_POWER_REGEN_PERCENT, 0).Average(core.CharacterLevel) / 100,
	})
	// arcane_missiles.go: channel rank N fires tick rank N.
	missiles := []map[string]any{}
	missilesLadder.Each(func(rank int32, row *spelldata.Spell) {
		missiles = append(missiles, map[string]any{"channel_spell_id": row.ID, "tick_spell_id": missileTicksLadder.Rank(rank).ID})
	})
	effects = append(effects, map[string]any{"kind": "arcane_missiles", "ranks": missiles})
	// frostbolt.go
	effects = append(effects, map[string]any{"kind": "frostbolt"})
	if talents.ArcaneBlast { // arcane_blast.go and arcane_charge.go
		buff := arcaneBlastBuff.Highest()
		effects = append(effects, map[string]any{
			"kind": "arcane_blast", "spell_id": arcaneBlastLadder.Highest().ID, "aura": "Arcane Blast",
			"damage_per_stack": buff.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_DAMAGE)).Average(core.CharacterLevel) / 100,
			"cost_per_stack":   buff.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Average(core.CharacterLevel) / 100,
		})
	}
	// fire_blast.go
	effects = append(effects, map[string]any{"kind": "fire_blast"})
	// fireball.go: every rank; the hit lands after travel, then its dot snapshots and ticks.
	fireballs := []map[string]any{}
	fireballLadder.Each(func(_ int32, row *spelldata.Spell) {
		fireballs = append(fireballs, map[string]any{
			"spell_id": row.ID, "tick_base": row.PeriodicEffect().Average(core.CharacterLevel),
			"tick_can_crit": row.PeriodicCanCrit() && row.DefenseTypeCore() == core.DefenseTypeMagic,
		})
	})
	effects = append(effects, map[string]any{"kind": "fireball", "ranks": fireballs})
	// scorch.go: every rank; Improved Scorch stacks Fire Vulnerability on the mage itself.
	scorch := map[string]any{"kind": "scorch"}
	if talents.ImprovedScorch > 0 {
		scorch["improved_scorch"] = map[string]any{
			"aura": "Fire Vulnerability", "proc_chance": improvedScorch.FractionAt(talents.ImprovedScorch),
			"damage_per_stack": fireVulnerability.Highest().EffectN(1).Average(core.CharacterLevel) / 100,
		}
	}
	effects = append(effects, scorch)
	if talents.MasterOfElements > 0 { // talents_fire.go registerMasterOfElements
		effects = append(effects, map[string]any{
			"kind": "master_of_elements", "trigger_aura": "Master of Elements",
			"refund":            masterOfElements.FractionAt(talents.MasterOfElements),
			"metrics_action_id": actionID(core.ActionID{SpellID: masterOfElements.Highest().ID}),
		})
	}
	if talents.Ignite > 0 { // talents_fire.go registerIgnite: fire spell crits feed a dot
		effects = append(effects, map[string]any{
			"kind": "ignite", "trigger_aura": "Ignite Talent", "spell_id": igniteTriggered.Highest().ID,
		})
	}
	if talents.ArcanePower { // arcane_power.go
		rank := arcanePower.Highest()
		effects = append(effects, map[string]any{
			"kind": "arcane_power", "spell_id": rank.ID, "aura": "Arcane Power",
			"damage":           rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_DAMAGE)).Average(core.CharacterLevel) / 100,
			"cost_percent_add": rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_COST)).Average(core.CharacterLevel) / 100,
		})
	}
	if talents.PresenceOfMind { // presence_of_mind.go
		rank := presenceOfMind.Highest()
		effects = append(effects, map[string]any{
			"kind": "presence_of_mind", "spell_id": rank.ID, "aura": "Presence of Mind",
			"cast_time_percent": rank.Effect(dbcenums.A_ADD_PCT_MODIFIER, int32(dbcenums.SPELLMOD_CASTING_TIME)).Average(core.CharacterLevel) / 100,
		})
	}
	// mana_gems.go: smaller gems wait for larger ones; all share the conjured cooldown.
	gems := []map[string]any{}
	for _, gem := range []struct {
		item int32
		row  *spelldata.Spell
	}{{5514, agateTriggered.Highest()}, {5513, jadeTriggered.Highest()}, {8007, citrineTriggered.Highest()}, {8008, rubyTriggered.Highest()}} {
		gems = append(gems, map[string]any{"item_id": gem.item, "mana": gem.row.EnergizeEffect().Average(core.CharacterLevel)})
	}
	effects = append(effects, map[string]any{"kind": "mana_gems", "gems": gems, "regen_window_seconds": 2.0})
	// armors.go: the regen part is already in the prepared pseudo stats.
	if hasAura(unit, "Mage Armor") {
		effects = append(effects, map[string]any{"kind": "mage_armor", "aura": "Mage Armor"})
	}
	return effects
}

func commonEffects(character *core.Character, target *core.Unit, request *proto.RaidSimRequest, unrepresented *[]string) []map[string]any {
	effects := []map[string]any{}
	for _, aura := range target.GetAuras() {
		if aura.Label == "Judgement of Wisdom (External)" { // buffs/paladin.go AttachJudgementOfWisdomMana
			effects = append(effects, map[string]any{
				"kind": "judgement_of_wisdom", "aura": aura.Label, "proc_chance": 0.5,
				"proc_mask": procMaskNames(core.ProcMaskDirect), "mana": buffs.JudgementOfWisdomMaxRank.Value,
				"metrics_action_id": actionID(core.ActionID{SpellID: buffs.JudgementOfWisdomMaxRank.SpellID}),
				"delay_ns":          nanos(core.SpellBatchWindow),
			})
		}
	}
	// racials.go applyTouchOfTheGrave: Go literals.
	if hasAura(&character.Unit, "Touch of the Grave") {
		chance := 0.1
		switch character.Class {
		case proto.Class_ClassWarrior, proto.Class_ClassPaladin, proto.Class_ClassRogue:
			chance = 0.05
		}
		effects = append(effects, map[string]any{
			"kind": "touch_of_the_grave", "trigger_aura": "Touch of the Grave", "drain_spell_id": int32(1260198),
			"proc_chance": chance, "proc_mask": procMaskNames(core.ProcMaskMelee | core.ProcMaskRanged | core.ProcMaskSpellDamage),
			"health_fraction": 0.05, "delay_ns": nanos(core.SpellBatchWindow),
		})
	}
	consumes := request.Raid.Parties[0].Players[0].Consumables
	for _, cd := range character.GetMajorCooldowns() {
		spell := cd.Spell
		// Mage gems are described by the mana_gems effect.
		if spell.ActionID.ItemID == 0 || spell.Matches(mage.MageSpellManaGem) {
			continue
		}
		item := spell.ActionID.ItemID
		consumable := core.GetConsumableByID(item)
		switch {
		case consumable.Id != 0 && spell.Flags.Matches(core.SpellFlagPotion): // consumes.go potions
			gains := []map[string]any{}
			for _, effectID := range consumable.EffectIds {
				e := core.GetSpellEffectByID(effectID)
				if e.GetResourceType() == proto.ResourceType_ResourceTypeMana && e.AuraPeriodMs == 0 && e.Type == proto.EffectType_EffectTypeResourceGain {
					gains = append(gains, map[string]any{"min": e.MinEffectSize, "spread": e.EffectSpread})
				} else {
					*unrepresented = append(*unrepresented, fmt.Sprintf("potion %d effect %d is not an instant mana gain", item, effectID))
				}
			}
			if consumable.BuffDuration > 0 {
				*unrepresented = append(*unrepresented, fmt.Sprintf("potion %d has a stat buff", item))
			}
			effects = append(effects, map[string]any{
				"kind": "potion_mana", "item_id": item, "rng_label": consumable.Name, "gains": gains,
				"stone_multiplier": map[bool]float64{true: 1.4, false: 1.0}[character.HasAlchStone()], "regen_window_seconds": 5.0,
			})
		case consumable.Id != 0 && spell.Flags.Matches(core.SpellFlagConjured): // consumes.go conjured
			gains := []map[string]any{}
			for _, effectID := range consumable.EffectIds {
				e := core.GetSpellEffectByID(effectID)
				resource := e.GetResourceType()
				if (e.Type == proto.EffectType_EffectTypeResourceGain || e.Type == proto.EffectType_EffectTypeHeal) && resource != 0 {
					if resource != proto.ResourceType_ResourceTypeMana {
						*unrepresented = append(*unrepresented, fmt.Sprintf("conjured %d restores %s", item, resource))
						continue
					}
					gains = append(gains, map[string]any{"min": e.MinEffectSize, "spread": e.EffectSpread})
				}
			}
			if consumable.BuffDuration > 0 {
				*unrepresented = append(*unrepresented, fmt.Sprintf("conjured %d has a stat buff", item))
			}
			effects = append(effects, map[string]any{
				"kind": "conjured_mana", "item_id": item, "rng_label": consumable.Name, "gains": gains,
				"selected": consumes.GetConjuredId() == item, "regen_window_seconds": 5.0,
			})
		default:
			// shared.NewSpellDataEnergizeOnUse: an item use spell that restores mana.
			found := false
			if dbItem := core.GetItemByID(item); dbItem != nil {
				for _, itemEffect := range dbItem.ItemEffects {
					if itemEffect.GetOnUse() == nil {
						continue
					}
					row := spelldata.Find(itemEffect.BuffId)
					effect := row.ProcEnergizeEffect()
					if effect == nil || dbcenums.PowerType(effect.Misc) != dbcenums.POWER_MANA || effect.Aura == dbcenums.A_PERIODIC_ENERGIZE {
						continue
					}
					found = true
					effects = append(effects, map[string]any{
						"kind": "energize_on_use", "item_id": item, "spell_id": row.ID,
						"average": effect.Average(character.Level), "variance": effect.Variance, "whole": effect.Max(character.Level),
					})
				}
			}
			if !found {
				*unrepresented = append(*unrepresented, fmt.Sprintf("major cooldown item %d has no exported effect", item))
			}
		}
	}
	return effects
}

func prepare(request *proto.RaidSimRequest, digest, scenario string) Prepared {
	unrepresented := []string{}
	note := func(condition bool, message string) {
		if condition {
			unrepresented = append(unrepresented, message)
		}
	}
	note(len(request.Raid.GetParties()) != 1 || len(request.Raid.Parties[0].GetPlayers()) != 1, "exactly one player is supported")
	note(len(request.Encounter.GetTargets()) != 1, "exactly one target is supported")
	note(request.Encounter.GetUseHealth(), "health-based fights are unsupported")
	note(len(request.Raid.GetTanks()) != 0, "tank assignments are unsupported")

	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	agent := simulation.Raid.Parties[0].Players[0]
	character := agent.GetCharacter()
	target := simulation.Encounter.ActiveTargetUnits[0]
	timers := &timerNames{names: map[*core.Timer]string{}}

	presimmer, presims := agent.(core.Presimmer)
	note(presims && presimmer.GetPresimOptions(request.Raid.Parties[0].Players[0]) != nil, "agent requires presims")
	note(request.Raid.Parties[0].Players[0].GetHealingModel() != nil, "healing models are unsupported")
	note(len(character.Pets) != 0, "pets are unsupported")
	note(character.AutoAttacks.AutoSwingMelee || character.AutoAttacks.AutoSwingRanged, "player auto attacks are unsupported")
	// A target only swings when it has a current target, i.e. an assigned tank.
	note((target.AutoAttacks.AutoSwingMelee || target.AutoAttacks.AutoSwingRanged) && target.CurrentTarget != nil, "target auto attacks are unsupported")
	note(character.ItemSwap.IsEnabled(), "item swapping is unsupported")
	note(simulation.Encounter.EndFightAtHealth != 0, "health-based fights are unsupported")
	note(simulation.PrepullStartTime() != 0, "prepull actions are unsupported")
	note(privateField(simulation, "executePhaseCallbacks").Len() != 0, "execute phase callbacks are unsupported")
	note(simulation.Encounter.AllTargets[0].AI != nil, "target AI is unsupported")
	table := character.AttackTables[target.UnitIndex]
	note(table.DamageDoneByCasterMultiplier != nil || len(table.DamageDoneByCasterExtraMultiplier) != 0, "caster damage callbacks are unsupported")
	note(len(target.DynamicDamageTakenModifiers) != 0, "dynamic damage taken modifiers are unsupported")
	for mobType, bonus := range table.MobTypeBonusStats {
		note(bonus != (stats.Stats{}), fmt.Sprintf("mob type bonus stats for %s are unsupported", mobType))
	}

	mageAgent, isMage := agent.(mage.MageAgent)
	note(!isMage, "only Mage agents are exported")

	spells := []Spell{}
	for _, spell := range character.Spellbook {
		spells = append(spells, exportSpell(spell, target, timers, &unrepresented))
	}
	attachDamageEffects(spells, character)

	mcds := []MajorCooldown{}
	for _, id := range character.GetMajorCooldownIDs() {
		mcd := character.GetInitialMajorCooldown(core.ProtoToActionID(id))
		timings := []int64{}
		for _, t := range mcd.GetTimings() {
			timings = append(timings, nanos(t))
		}
		mcds = append(mcds, MajorCooldown{ActionID: actionID(mcd.Spell.ActionID), Priority: mcd.Priority,
			Type: cooldownTypeNames(mcd.Type), Timings: timings})
	}

	player := request.Raid.Parties[0].Players[0]
	rotation, err := protojson.MarshalOptions{UseProtoNames: false}.Marshal(player.GetRotation())
	fail(err)
	classOptions, err := protojson.Marshal(player.GetMage().GetOptions().GetClassOptions())
	fail(err)

	talents := map[string]int32{}
	effects := []map[string]any{}
	if isMage {
		m := mageAgent.GetMage()
		reflected := m.Talents.ProtoReflect()
		fields := reflected.Descriptor().Fields()
		for i := 0; i < fields.Len(); i++ {
			field := fields.Get(i)
			value := reflected.Get(field)
			switch {
			case field.Kind().String() == "bool" && value.Bool():
				talents[string(field.Name())] = 1
			case field.Kind().String() == "int32" && value.Int() != 0:
				talents[string(field.Name())] = int32(value.Int())
			}
		}
		effects = append(effects, mageEffects(m, character)...)
	}
	effects = append(effects, commonEffects(character, target, request, &unrepresented)...)
	// Listeners that receive the player's spell events but act only on events outside the
	// supported scope: health.go trackChanceOfDeath and attack.go Parry Haste.
	for _, inert := range []struct {
		unit   *core.Unit
		side   string
		label  string
		reason string
	}{
		{&character.Unit, "player", core.ChanceOfDeathAuraLabel, "acts only when the player takes damage"},
		{target, "target", "Parry Haste", "acts only on parried attacks"},
	} {
		if inert.unit.GetAura(inert.label) != nil {
			effects = append(effects, map[string]any{"kind": "inert_listener", "unit": inert.side, "aura": inert.label, "reason": inert.reason})
		}
	}

	professions := []string{}
	for _, profession := range []proto.Profession{player.Profession1, player.Profession2} {
		if profession != proto.Profession_ProfessionUnknown {
			professions = append(professions, profession.String())
		}
	}

	prepared := Prepared{
		SchemaVersion: schemaVersion, Contract: "forever-prepared",
		Reference:     Reference{EngineRevision: engineRevision, ClientBuild: clientBuild, Exporter: "tools/oracle-v2"},
		RequestSHA256: digest, ScenarioID: scenario,
		Sim: SimOptions{Iterations: request.SimOptions.Iterations, Seed: request.SimOptions.RandomSeed,
			LabeledRng: request.SimOptions.UseLabeledRands || request.SimOptions.IsTest, DebugFirstIteration: request.SimOptions.DebugFirstIteration,
			Debug: request.SimOptions.Debug},
		Encounter: Encounter{DurationNs: nanos(simulation.BaseDuration), DurationVariationNs: nanos(simulation.DurationVariation),
			ExecuteProportion20: simulation.Encounter.ExecuteProportion_20, ExecuteProportion25: simulation.Encounter.ExecuteProportion_25,
			ExecuteProportion35: simulation.Encounter.ExecuteProportion_35, ExecuteProportion45: simulation.Encounter.ExecuteProportion_45,
			ExecuteProportion90: simulation.Encounter.ExecuteProportion_90},
		Target: TargetUnit{
			Unit: Unit{Index: target.UnitIndex, Label: target.Label, Level: target.Level, MobType: target.MobType.String(),
				Stats: statValues(target.GetStats()), PseudoStats: exportPseudo(target.PseudoStats), Auras: exportAuras(target, timers)},
			AutoSwingMelee: target.AutoAttacks.AutoSwingMelee, AutoSwingRanged: target.AutoAttacks.AutoSwingRanged,
			MetricsActions: metricsActions(target),
		},
		Player: Player{
			Unit: Unit{Index: character.UnitIndex, Label: character.Label, Level: character.Level,
				Stats: statValues(character.GetStats()), PseudoStats: exportPseudo(character.PseudoStats), Auras: exportAuras(&character.Unit, timers)},
			Name: player.Name, Class: player.Class.String(), Race: player.Race.String(), Professions: professions,
			TalentsString: player.TalentsString, Talents: talents, ClassOptions: classOptions,
			ReactionNs: nanos(character.ReactionTime), ChannelClipDelayNs: nanos(character.ChannelClipDelay),
			DistanceYards: character.DistanceFromTarget, CastSpeed: character.CastSpeed,
			Mana: Mana{Max: character.MaxMana(), Base: character.BaseMana, SpiritRegenPerSecond: character.SpiritManaRegenPerSecond(),
				RegenPerSecondCasting: character.ManaRegenPerSecondWhileCasting(), RegenPerSecondNotCasting: character.ManaRegenPerSecondWhileNotCasting()},
			AttackTable: AttackTable{BaseSpellMissChance: table.BaseSpellMissChance, SpellCritSuppression: table.SpellCritSuppression,
				BonusSpellCritPercent: table.BonusSpellCritPercent, CritMultiplier: table.CritMultiplier,
				DamageDealtMultiplier: table.DamageDealtMultiplier, DamageTakenMultiplier: table.DamageTakenMultiplier},
			Spells: spells, MajorCooldowns: mcds, Rotation: rotation,
		},
		Effects: effects, Unrepresented: unrepresented,
	}
	// Last: the teardown changes the simulation.
	prepared.Player.Mana.TeardownMax = teardownMaxMana(simulation, &character.Unit, &prepared.Unrepresented)
	return prepared
}

// Go Spell.doneIteration: every spell without SpellFlagNoMetrics reports under its action
// ID, or one tagged ID per metric split; addSpellMetrics keeps the first registration.
func metricsActions(unit *core.Unit) []MetricsAction {
	actions := []MetricsAction{}
	if len(unit.AttackTables) == 0 {
		return actions
	}
	seen := map[core.ActionID]bool{}
	for _, spell := range unit.Spellbook {
		if spell.Flags.Matches(core.SpellFlagNoMetrics) {
			continue
		}
		ids := []core.ActionID{spell.ActionID}
		if splits := spell.GetMetricSplitCount(); splits > 1 {
			ids = ids[:0]
			for i := 0; i < splits; i++ {
				ids = append(ids, spell.ActionID.WithTag(int32(i)))
			}
		}
		for _, id := range ids {
			if seen[id] {
				continue
			}
			seen[id] = true
			actions = append(actions, MetricsAction{ActionID: actionID(id),
				MeleeMetrics: spell.Flags.Matches(core.SpellFlagMeleeMetrics), School: uint8(spell.SpellSchool)})
		}
	}
	return actions
}

// Go auraTracker.doneIteration deactivates every aura, permanent ones included, in
// registration order. Prepared buffs are the only auras that change maximum mana, and
// they are active from reset, so the reset state tears down as every fight's end does.
func teardownMaxMana(sim *core.Simulation, unit *core.Unit, unrepresented *[]string) float64 {
	lowest := unit.MaxMana()
	for {
		var active *core.Aura
		for _, aura := range unit.GetAuras() {
			if aura.IsActive() {
				active = aura
				break
			}
		}
		if active == nil {
			return lowest
		}
		before := unit.MaxMana()
		active.Deactivate(sim)
		if unit.MaxMana() > before {
			*unrepresented = append(*unrepresented, fmt.Sprintf("aura %s raises maximum mana when it fades", active.Label))
		}
		lowest = min(lowest, unit.MaxMana())
	}
}

func run(request *proto.RaidSimRequest, output string) {
	started := time.Now()
	result := core.RunRaidSim(request)
	elapsed := time.Since(started).Nanoseconds()
	if result.Error != nil && result.Error.Message != "" {
		fail(fmt.Errorf("Go engine: %s", result.Error.Message))
	}
	data, err := protojson.Marshal(result)
	fail(err)
	var raw map[string]any
	fail(json.Unmarshal(data, &raw))
	raw["elapsedNs"] = elapsed
	writeJSON(output, raw)
}

func main() {
	if engineRevision == "" || clientBuild == "" {
		fail(fmt.Errorf("build with the pin from upstream/sources.json (tools/prepared_v2.py does this)"))
	}
	sim.RegisterAll()
	if len(os.Args) < 2 {
		fail(fmt.Errorf("expected prepare or sim"))
	}
	flags := flag.NewFlagSet(os.Args[1], flag.ExitOnError)
	infile := flags.String("infile", "", "Go RaidSimRequest")
	outfile := flags.String("outfile", "", "output file")
	scenario := flags.String("scenario", "", "scenario identifier for prepared output")
	fail(flags.Parse(os.Args[2:]))
	if *infile == "" || *outfile == "" {
		fail(fmt.Errorf("--infile and --outfile are required"))
	}
	request, digest := readRequest(*infile)
	switch os.Args[1] {
	case "prepare":
		if *scenario == "" {
			fail(fmt.Errorf("--scenario is required"))
		}
		prepared := prepare(request, digest, *scenario)
		sort.Strings(prepared.Unrepresented)
		writeJSON(*outfile, prepared)
	case "sim":
		run(request, *outfile)
	default:
		fail(fmt.Errorf("unsupported command %s", os.Args[1]))
	}
}
