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
	"math"
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
	"google.golang.org/protobuf/reflect/protoreflect"
	"google.golang.org/protobuf/types/known/emptypb"
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

// A class's export: stable names for its Go-internal class mask bits, so a Go reordering
// cannot silently change Rust behavior; the client rows its spells roll damage from; and
// the effects whose parameters Go keeps in closures. Each class file registers one.
type classSpellName struct {
	mask int64
	name string
}

type classExport struct {
	spells     []classSpellName
	damageRows func(rows map[int32]*spelldata.Spell)
	effects    func(agent core.Agent, character *core.Character) []map[string]any
	// How many of the target's dynamic damage taken modifiers the class effects describe.
	damageTakenModifiers func(agent core.Agent) int
	// Why a registered pet never acts in this build, or "" when it may.
	inertPet func(agent core.Agent, pet *core.Pet) string
	// Optional: class behavior the effects cannot describe, one reason each.
	unrepresented func(agent core.Agent, character *core.Character) []string
	// Optional: class auras that change stats through AddStatsDynamic when gained or lost.
	statAuras func(agent core.Agent, character *core.Character) []string
}

var classExports = map[proto.Class]classExport{}

// Set by prepare while class effects run: the request, so an effect can reset a separate
// simulation, and the unrepresented list, so an effect can name what it cannot describe.
var (
	exportRequest *proto.RaidSimRequest
	classNotes    *[]string
)

func classSpell(class classExport, mask int64, unrepresented *[]string, id core.ActionID) string {
	if mask == 0 {
		return ""
	}
	for _, entry := range class.spells {
		if entry.mask == mask {
			return entry.name
		}
	}
	*unrepresented = append(*unrepresented, fmt.Sprintf("spell %s has unnamed class mask %d", id, mask))
	return ""
}

func damageEffect(row *spelldata.Spell) *DamageEffect {
	effect := row.DamageEffect()
	if effect == nil {
		fail(fmt.Errorf("spell %d has no damage effect", row.ID))
	}
	return &DamageEffect{Average: effect.Average(core.CharacterLevel), Variance: effect.Variance}
}

func attachDamageEffects(spells []Spell, class classExport) {
	rows := map[int32]*spelldata.Spell{}
	if class.damageRows != nil {
		class.damageRows(rows)
	}
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

// The spec's class options, the options.class_options of whichever spec the player sets.
func specClassOptions(player *proto.Player) googleProto.Message {
	reflected := player.ProtoReflect()
	field := reflected.WhichOneof(reflected.Descriptor().Oneofs().ByName("spec"))
	if field == nil || field.Message() == nil {
		return &emptypb.Empty{}
	}
	spec := reflected.Get(field).Message()
	options := spec.Descriptor().Fields().ByName("options")
	if options == nil || !spec.Has(options) {
		return &emptypb.Empty{}
	}
	message := spec.Get(options).Message()
	classOptions := message.Descriptor().Fields().ByName("class_options")
	if classOptions == nil || !message.Has(classOptions) {
		return &emptypb.Empty{}
	}
	return message.Get(classOptions).Message().Interface()
}

// The class talents proto, found as the Talents field of the agent or a struct it embeds.
func classTalents(agent core.Agent) protoreflect.Message {
	var search func(value reflect.Value) protoreflect.Message
	search = func(value reflect.Value) protoreflect.Message {
		for value.Kind() == reflect.Pointer || value.Kind() == reflect.Interface {
			if value.IsNil() {
				return nil
			}
			value = value.Elem()
		}
		if value.Kind() != reflect.Struct {
			return nil
		}
		if field := value.FieldByName("Talents"); field.IsValid() && field.CanInterface() {
			if message, ok := field.Interface().(googleProto.Message); ok && !field.IsNil() {
				return message.ProtoReflect()
			}
		}
		for i := 0; i < value.NumField(); i++ {
			if value.Type().Field(i).Anonymous {
				if found := search(value.Field(i)); found != nil {
					return found
				}
			}
		}
		return nil
	}
	return search(reflect.ValueOf(agent))
}

type Cost struct {
	Resource                string  `json:"resource"`
	BaseCost                int32   `json:"base_cost"`
	FlatModifier            int32   `json:"flat_modifier"`
	PercentModifier         float64 `json:"percent_modifier"`
	AdditivePercentModifier float64 `json:"additive_percent_modifier"`
	// energy.go EnergyCost.Refund: the share of the cost a missed strike gives back.
	Refund float64 `json:"refund,omitempty"`
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
	// The spellbook position of the spell whose dot Spell.Dot resolves to when this spell
	// has none of its own.
	RelatedDotSpell *int `json:"related_dot_spell,omitempty"`
	// spell.go splitSpellMetrics: how many tagged metric entries the spell reports under.
	MetricSplits int `json:"metric_splits,omitempty"`
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

// energy.go energyBar: the bar Rust runs, with its regeneration ticks and combo points.
type Energy struct {
	MaxEnergy      float64 `json:"max_energy"`
	MaxComboPoints int32   `json:"max_combo_points"`
	TickDurationNs int64   `json:"tick_duration_ns"`
	EnergyPerTick  float64 `json:"energy_per_tick"`
}

func exportEnergy(character *core.Character, unrepresented *[]string) *Energy {
	if !character.HasEnergyBar() {
		return nil
	}
	if privateField(&character.Unit, "energyBar").FieldByName("hasNoRegen").Bool() {
		*unrepresented = append(*unrepresented, "an energy bar without regeneration is unsupported")
	}
	return &Energy{MaxEnergy: character.MaximumEnergy(), MaxComboPoints: character.MaxComboPoints(),
		TickDurationNs: nanos(character.EnergyTickDuration), EnergyPerTick: character.EnergyPerTick}
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
	Energy             *Energy          `json:"energy,omitempty"`
	AttackTable        AttackTable      `json:"attack_table"`
	Spells             []Spell          `json:"spells"`
	MajorCooldowns     []MajorCooldown  `json:"major_cooldowns"`
	Rotation           json.RawMessage  `json:"rotation"`
	// Every prepull action Go registered, the rotation's and any a class or item adds.
	PrepullActions int `json:"prepull_actions"`
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

type Weapon struct {
	BaseDamageMin        float64 `json:"base_damage_min"`
	BaseDamageMax        float64 `json:"base_damage_max"`
	AttackPowerPerDPS    float64 `json:"attack_power_per_dps"`
	SwingSpeed           float64 `json:"swing_speed"`
	NormalizedSwingSpeed float64 `json:"normalized_swing_speed"`
	School               uint8   `json:"school"`
	MinRange             float64 `json:"min_range"`
	MaxRange             float64 `json:"max_range"`
}

func exportWeapon(weapon *core.Weapon) Weapon {
	return Weapon{BaseDamageMin: weapon.BaseDamageMin, BaseDamageMax: weapon.BaseDamageMax,
		AttackPowerPerDPS: weapon.AttackPowerPerDPS, SwingSpeed: weapon.SwingSpeed,
		NormalizedSwingSpeed: weapon.NormalizedSwingSpeed, School: uint8(weapon.SpellSchool),
		MinRange: weapon.MinRange, MaxRange: weapon.MaxRange}
}

// The player's weapon attacks and the physical attack table against the target. Every value
// the physical outcome rolls read is static in scope, so it is exported resolved.
type Melee struct {
	AutoSwingMelee  bool   `json:"auto_swing_melee"`
	AutoSwingRanged bool   `json:"auto_swing_ranged"`
	DualWielding    bool   `json:"dual_wielding"`
	MainHand        Weapon `json:"main_hand"`
	OffHand         Weapon `json:"off_hand"`
	Ranged          Weapon `json:"ranged"`

	BaseMissChance       float64 `json:"base_miss_chance"`
	BaseGlanceChance     float64 `json:"base_glance_chance"`
	GlanceMultiplier     float64 `json:"glance_multiplier"`
	GlanceSpread         float64 `json:"glance_spread"`
	HitSuppression       float64 `json:"hit_suppression"`
	MeleeCritSuppression float64 `json:"melee_crit_suppression"`
	IgnoreArmor          bool    `json:"ignore_armor"`
	ArmorIgnoreFactor    float64 `json:"armor_ignore_factor"`

	InFrontOfTarget       bool    `json:"in_front_of_target"`
	AttackSpeedMultiplier float64 `json:"attack_speed_multiplier"`
	MeleeSpeedMultiplier  float64 `json:"melee_speed_multiplier"`
	DodgeReduction        float64 `json:"dodge_reduction"`
	DisableDWMissPenalty  bool    `json:"disable_dw_miss_penalty"`

	// The defender's chances before the attacker's expertise: base pseudo stat, table base
	// and rating, as GetTotalDodgeChanceAsDefender and its siblings add them.
	DefenderDodge                 float64 `json:"defender_dodge"`
	DefenderParry                 float64 `json:"defender_parry"`
	DefenderBlock                 float64 `json:"defender_block"`
	DefenderArmor                 float64 `json:"defender_armor"`
	DefenderBlockReduction        float64 `json:"defender_block_reduction"`
	DefenderBonusAttackPower      float64 `json:"defender_bonus_attack_power"`
	DefenderBonusPhysicalTaken    float64 `json:"defender_bonus_physical_damage_taken"`
	DefenderReducedPhysicalHitPct float64 `json:"defender_reduced_physical_hit_taken"`
}

func exportMelee(character *core.Character, target *core.Unit, table *core.AttackTable, unrepresented *[]string) Melee {
	aa := &character.AutoAttacks
	mh := privateField(aa, "mh")
	if aa.AutoSwingMelee && !mh.FieldByName("replaceSwing").IsNil() {
		*unrepresented = append(*unrepresented, "main hand swings can be replaced")
	}
	if aa.AutoSwingRanged {
		*unrepresented = append(*unrepresented, "ranged auto attacks are unsupported")
	}
	if len(character.OnMeleeAttackSpeedChanged) != 0 {
		*unrepresented = append(*unrepresented, "melee attack speed listeners are unsupported")
	}
	pseudo := &character.PseudoStats
	defender := &target.PseudoStats
	return Melee{
		AutoSwingMelee: aa.AutoSwingMelee, AutoSwingRanged: aa.AutoSwingRanged, DualWielding: aa.IsDualWielding,
		MainHand: exportWeapon(aa.MH()), OffHand: exportWeapon(aa.OH()), Ranged: exportWeapon(aa.Ranged()),
		BaseMissChance: table.BaseMissChance, BaseGlanceChance: table.BaseGlanceChance,
		GlanceMultiplier: table.GlanceMultiplier, GlanceSpread: table.GlanceSpread,
		HitSuppression: table.HitSuppression, MeleeCritSuppression: table.MeleeCritSuppression,
		IgnoreArmor: table.IgnoreArmor, ArmorIgnoreFactor: table.ArmorIgnoreFactor,
		InFrontOfTarget: pseudo.InFrontOfTarget, AttackSpeedMultiplier: pseudo.AttackSpeedMultiplier,
		MeleeSpeedMultiplier: pseudo.MeleeSpeedMultiplier, DodgeReduction: pseudo.DodgeReduction,
		DisableDWMissPenalty: pseudo.DisableDWMissPenalty,
		DefenderDodge:        defender.BaseDodgeChance + table.BaseDodgeChance + target.GetDodgeFromRating(),
		DefenderParry:        defender.BaseParryChance + table.BaseParryChance + target.GetParryFromRating(),
		DefenderBlock:        defender.BaseBlockChance + table.BaseBlockChance + target.GetBlockFromRating(),
		DefenderArmor:        target.Armor(), DefenderBlockReduction: target.BlockDamageReduction(),
		DefenderBonusAttackPower: defender.BonusAttackPower, DefenderBonusPhysicalTaken: defender.BonusPhysicalDamageTaken,
		DefenderReducedPhysicalHitPct: defender.ReducedPhysicalHitTakenChance,
	}
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
	Melee         Melee            `json:"melee"`
	Effects       []map[string]any `json:"effects"`
	Unrepresented []string         `json:"unrepresented"`
}

func exportSpell(spell *core.Spell, target *core.Unit, class classExport, timers *timerNames, unrepresented *[]string) Spell {
	var cost *Cost
	if spell.Cost != nil {
		resource := "unknown"
		refund := 0.0
		switch impl := spell.Cost.ResourceCostImpl.(type) {
		case *core.ManaCost:
			resource = "mana"
		case *core.EnergyCost:
			resource = "energy"
			refund = impl.Refund
			// A refund is credited to the energy bar's refund metrics, the default and the
			// only one any class passes.
			if impl.Refund > 0 && impl.RefundMetrics != spell.Unit.EnergyRefundMetrics {
				*unrepresented = append(*unrepresented, fmt.Sprintf("spell %s refunds energy to its own metrics", spell.ActionID))
			}
		default:
			*unrepresented = append(*unrepresented, fmt.Sprintf("spell %s has an unsupported cost", spell.ActionID))
		}
		cost = &Cost{Resource: resource, BaseCost: spell.Cost.BaseCost, FlatModifier: spell.Cost.FlatModifier,
			PercentModifier: spell.Cost.PercentModifier, AdditivePercentModifier: spell.Cost.AdditivePercentModifier, Refund: refund}
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
		ClassSpell: classSpell(class, spell.ClassSpellMask, unrepresented, spell.ActionID), MissileSpeed: spell.MissileSpeed,
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
		MetricSplits: map[bool]int{true: spell.GetMetricSplitCount(), false: 0}[spell.GetMetricSplitCount() > 1],
	}
}

// racials.go applyEureka: the class names its spells by masks only Go can read, so the
// exporter resolves them, with spell_mod.go shouldApply's rules, into spell positions.
func eurekaEffect(agent core.Agent, character *core.Character) map[string]any {
	aura := character.GetAura("Eureka!")
	if aura == nil {
		return nil
	}
	masks := core.EurekaSpells{Cost: math.MaxInt64, Damage: math.MaxInt64}
	if eurekaAgent, ok := agent.(core.EurekaAgent); ok {
		masks = eurekaAgent.EurekaSpells()
	}
	procMask := core.ProcMaskSpecial
	if character.Class == proto.Class_ClassPriest {
		procMask |= core.ProcMaskSpellHealing
	}
	spent := masks.Cost | masks.Damage | masks.Tick
	directOnly := masks.Damage &^ masks.Tick
	modded := func(spell *core.Spell, mask int64) bool {
		return mask != 0 && !spell.Flags.Matches(core.SpellFlagNoSpellMods) && spell.Matches(mask) && procMask.Matches(spell.ProcMask)
	}
	cost, damage, ticks, spending := []int{}, []int{}, []int{}, []int{}
	for i, spell := range character.Spellbook {
		// The cost modifier names the class's resource: energy for a Rogue, mana for a caster.
		paid := false
		if spell.Cost != nil {
			switch spell.Cost.ResourceCostImpl.(type) {
			case *core.ManaCost:
				paid = character.Class != proto.Class_ClassRogue && character.Class != proto.Class_ClassWarrior
			case *core.EnergyCost:
				paid = character.Class == proto.Class_ClassRogue
			}
		}
		if paid && modded(spell, masks.Cost) {
			cost = append(cost, i)
		}
		if modded(spell, masks.Damage|masks.Tick) {
			damage = append(damage, i)
		}
		if modded(spell, directOnly) {
			ticks = append(ticks, i)
		}
		if spell.Matches(spent) && spell.ProcMask.Matches(procMask) {
			spending = append(spending, i)
		}
	}
	return map[string]any{
		"kind": "eureka", "spell_id": aura.ActionID.SpellID, "aura": aura.Label,
		"cost_percent": -0.1, "damage_percent": 0.1, "tick_cancel_percent": 1/1.1 - 1,
		"cost_spells": cost, "damage_spells": damage, "tick_cancel_spells": ticks, "spending_spells": spending,
	}
}

// The player stats that change while the named aura is active, read from a separate reset
// simulation so the exported one is untouched.
func activeStats(request *proto.RaidSimRequest, label string) map[string]float64 {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	character := simulation.Raid.Parties[0].Players[0].GetCharacter()
	before := statValues(character.GetStats())
	character.GetAura(label).Activate(simulation)
	changed := map[string]float64{}
	for name, value := range statValues(character.GetStats()) {
		if value != before[name] {
			changed[name] = value
		}
	}
	return changed
}

// The target's armor with the named aura active at the given stacks, from a separate reset
// simulation so the exported one is untouched.
func targetArmorWithStacks(request *proto.RaidSimRequest, label string, stacks int32) float64 {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	target := simulation.Encounter.ActiveTargetUnits[0]
	if stacks > 0 {
		aura := target.GetAura(label)
		aura.Activate(simulation)
		// A stronger aura in the exclusive armor category, such as Expose Armor, keeps it out.
		if !aura.IsActive() {
			return math.NaN()
		}
		aura.SetStacks(simulation, stacks)
	}
	return target.Armor()
}

// Whether activating the aura right after a reset fails, as an exclusive category with a
// stronger active member makes it.
func sunderBlocked(request *proto.RaidSimRequest, label string) bool {
	simulation := core.NewSim(request, simsignals.CreateSignals())
	simulation.Reset()
	aura := simulation.Encounter.ActiveTargetUnits[0].GetAura(label)
	aura.Activate(simulation)
	return !aura.IsActive()
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
	// racials.go Troll Berserking: AttachMultiplyAttackSpeed, then AttachMultiplyCastSpeed, with
	// Go literals.
	if aura := character.GetAura("Berserking"); aura != nil {
		effects = append(effects, map[string]any{
			"kind": "berserking", "spell_id": aura.ActionID.SpellID, "aura": aura.Label, "attack_speed_multiplier": 1.1, "cast_speed_multiplier": 1.1,
		})
	}
	// racials.go Orc Blood Fury: Go computes the buffed stats through its dynamic stat
	// dependencies, so a separate reset simulation activates the aura and reports them.
	if aura := character.GetAura("Blood Fury"); aura != nil {
		effects = append(effects, map[string]any{
			"kind": "blood_fury", "spell_id": aura.ActionID.SpellID, "aura": aura.Label,
			"active_stats": activeStats(request, aura.Label),
		})
	}
	// racials.go Night Elf Elune's Light: RegisterTemporaryStatsOnUseCD with Go literal stats.
	if aura := character.GetAura("Elune's Light"); aura != nil {
		buffs := stats.Stats{stats.PhysicalCritPercent: 10, stats.SpellCritPercent: 10}
		effects = append(effects, map[string]any{
			"kind": "temporary_stats", "spell_id": aura.ActionID.SpellID, "aura": aura.Label,
			"active_stats": activeStats(request, aura.Label),
			"gain_log":     fmt.Sprintf("Gained %s from %s.", buffs.FlatString(), aura.ActionID),
			"expire_log":   fmt.Sprintf("Lost %s from fading %s.", buffs.FlatString(), aura.ActionID),
		})
	}
	// buffs/drivers.go driveSunderArmor: the raid's Sunder Armor ramps to its maximum stacks, one a
	// default GCD from the pull, Go literals. Target armor at each stack count is read from
	// separate reset simulations, since the stacks act through exclusive armor effects.
	// A stronger permanent member of its exclusive category, such as the raid's Expose Armor,
	// blocks every activation, which Go still counts as a proc, and the armor never changes.
	// buffs.go ApplyFixedShoutAura: the party's Battle Shout is up for good, through
	// ApplyFixedUptimeAura's rolls: a period of its duration and a nanosecond, and a first try a
	// nanosecond before the pull with a rolled duration. Behind the player's own shout it chains
	// instead.
	if aura := character.GetAura("Battle Shout (External)"); aura != nil {
		for _, own := range character.GetAurasWithTag(buffs.BattleShoutCategory) {
			if own.ActionID.Tag == 0 {
				*unrepresented = append(*unrepresented, "the party's Battle Shout chains behind the player's own")
			}
		}
		effects = append(effects, map[string]any{
			"kind": "fixed_uptime_aura", "aura": aura.Label, "uptime": 1.0,
			"tick_length_ns": nanos(aura.Duration + 1), "start_time_ns": int64(-1),
		})
	}
	if aura := target.GetAura("Sunder Armor (External)"); aura != nil {
		blocked := sunderBlocked(request, aura.Label)
		for _, other := range target.GetAuras() {
			if other != aura && other.Tag == aura.Tag && other.IsActive() && other.Duration != core.NeverExpires {
				*unrepresented = append(*unrepresented, fmt.Sprintf("%s shares its category with expiring %s", aura.Label, other.Label))
			}
		}
		armor := []float64{}
		for stacks := int32(0); stacks <= aura.MaxStacks; stacks++ {
			if blocked {
				armor = append(armor, targetArmorWithStacks(request, aura.Label, 0))
			} else {
				armor = append(armor, targetArmorWithStacks(request, aura.Label, stacks))
			}
		}
		ramp := map[string]any{
			"kind": "sunder_armor_ramp", "aura": aura.Label, "period_ns": nanos(core.GCDDefault),
			"ticks": int32(5), "armor_by_stacks": armor,
		}
		if blocked {
			ramp["blocked"] = true
		}
		effects = append(effects, ramp)
	}
	// racials.go Orc Shatter Curse: its aura multiplies the player's magic damage taken, a Go
	// literal on each magic school.
	if aura := character.GetAura("Shatter Curse"); aura != nil {
		effects = append(effects, map[string]any{"kind": "shatter_curse", "spell_id": aura.ActionID.SpellID, "aura": aura.Label,
			"school_damage_taken_multiplier": 0.85, "schools": []string{"arcane", "fire", "frost", "holy", "nature", "shadow"}})
	}
	// racials.go Dwarf Stoneform: its aura changes only the player's physical damage taken.
	if aura := character.GetAura("Stoneform"); aura != nil {
		effects = append(effects, map[string]any{"kind": "stoneform", "spell_id": aura.ActionID.SpellID, "aura": aura.Label})
	}
	// racials.go High Order Skyborne Read Ley Line: Energized doubles mana regeneration, a
	// Go literal undone with 0.5.
	if aura := character.GetAura("Energized"); aura != nil {
		for _, spell := range character.Spellbook {
			if spell.RelatedSelfBuff == aura {
				effects = append(effects, map[string]any{"kind": "read_ley_line", "spell_id": spell.ActionID.SpellID, "aura": aura.Label, "regen_multiplier": 2.0})
			}
		}
	}
	consumes := request.Raid.Parties[0].Players[0].Consumables
	// Major cooldown items, then the potions and conjured items a rotation casts itself, which
	// Go removed from the major cooldowns.
	items := []*core.Spell{}
	cooldowns := map[*core.Spell]bool{}
	for _, cd := range character.GetMajorCooldowns() {
		items = append(items, cd.Spell)
		cooldowns[cd.Spell] = true
	}
	for _, spell := range character.Spellbook {
		if spell.ActionID.ItemID != 0 && !cooldowns[spell] && spell.Flags.Matches(core.SpellFlagPotion|core.SpellFlagConjured) {
			items = append(items, spell)
		}
	}
	for _, spell := range items {
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
			gains, energyGains := []map[string]any{}, []map[string]any{}
			for _, effectID := range consumable.EffectIds {
				e := core.GetSpellEffectByID(effectID)
				resource := e.GetResourceType()
				if (e.Type == proto.EffectType_EffectTypeResourceGain || e.Type == proto.EffectType_EffectTypeHeal) && resource != 0 {
					switch {
					case resource == proto.ResourceType_ResourceTypeMana:
						gains = append(gains, map[string]any{"min": e.MinEffectSize, "spread": e.EffectSpread})
					case resource == proto.ResourceType_ResourceTypeEnergy && character.HasEnergyBar():
						energyGains = append(energyGains, map[string]any{"min": e.MinEffectSize, "spread": e.EffectSpread})
					default:
						*unrepresented = append(*unrepresented, fmt.Sprintf("conjured %d restores %s", item, resource))
					}
				}
			}
			if consumable.BuffDuration > 0 {
				*unrepresented = append(*unrepresented, fmt.Sprintf("conjured %d has a stat buff", item))
			}
			if len(energyGains) == 0 {
				effects = append(effects, map[string]any{
					"kind": "conjured_mana", "item_id": item, "rng_label": consumable.Name, "gains": gains,
					"selected": consumes.GetConjuredId() == item, "regen_window_seconds": 5.0,
				})
				break
			}
			if len(gains) != 0 {
				*unrepresented = append(*unrepresented, fmt.Sprintf("conjured %d restores mana and energy", item))
			}
			// Thistle Tea gives 2 energy less a level above 40, a Go literal.
			reduction := 0.0
			if item == 7676 {
				reduction = float64(2 * max(0, core.CharacterLevel-40))
			}
			effects = append(effects, map[string]any{
				"kind": "conjured_energy", "item_id": item, "rng_label": consumable.Name, "gains": energyGains,
				"selected": consumes.GetConjuredId() == item, "level_reduction": reduction,
			})
		case spell.ActionID.SameAction(core.GoblinSapperActionID): // consumes.go newGoblinSapperSpell
			effects = append(effects, goblinSapperEffect(character, unrepresented))
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
	// A target only swings when it has a current target, i.e. an assigned tank.
	note((target.AutoAttacks.AutoSwingMelee || target.AutoAttacks.AutoSwingRanged) && target.CurrentTarget != nil, "target auto attacks are unsupported")
	note(character.ItemSwap.IsEnabled(), "item swapping is unsupported")
	note(simulation.Encounter.EndFightAtHealth != 0, "health-based fights are unsupported")
	note(privateField(simulation, "executePhaseCallbacks").Len() != 0, "execute phase callbacks are unsupported")
	note(simulation.Encounter.AllTargets[0].AI != nil, "target AI is unsupported")
	table := character.AttackTables[target.UnitIndex]
	note(table.DamageDoneByCasterMultiplier != nil || len(table.DamageDoneByCasterExtraMultiplier) != 0, "caster damage callbacks are unsupported")
	class, exported := classExports[character.Class]
	described := 0
	if exported && class.damageTakenModifiers != nil {
		described = class.damageTakenModifiers(agent)
	}
	note(len(target.DynamicDamageTakenModifiers) != described, "dynamic damage taken modifiers are unsupported")
	note(len(character.OnCastSpeedChanged) != 0, "cast speed listeners are unsupported")
	note(len(character.OnTemporaryStatsChanges) != 0, "temporary stat listeners are unsupported")
	if threshold := request.Raid.Parties[0].Players[0].GetCooldowns().GetHpPercentForDefensives(); threshold != 0 {
		for _, cd := range character.GetMajorCooldowns() {
			note(cd.Type.Matches(core.CooldownTypeSurvival), fmt.Sprintf("survival cooldown %s waits for health %v", cd.Spell.ActionID, threshold))
		}
	}
	for mobType, bonus := range table.MobTypeBonusStats {
		note(bonus != (stats.Stats{}), fmt.Sprintf("mob type bonus stats for %s are unsupported", mobType))
	}

	note(!exported, fmt.Sprintf("%s agents are not exported", character.Class))
	inertPets := []map[string]any{}
	for _, pet := range character.Pets {
		reason := ""
		if class.inertPet != nil {
			reason = class.inertPet(agent, pet)
		}
		if reason == "" {
			note(true, "pets are unsupported")
			continue
		}
		inertPets = append(inertPets, inertPetEffect(pet, reason))
	}

	spells := []Spell{}
	for _, spell := range character.Spellbook {
		spells = append(spells, exportSpell(spell, target, class, timers, &unrepresented))
	}
	attachDamageEffects(spells, class)
	for i, spell := range character.Spellbook {
		if spell.RelatedDotSpell == nil {
			continue
		}
		for j, related := range character.Spellbook {
			if related == spell.RelatedDotSpell {
				position := j
				spells[i].RelatedDotSpell = &position
			}
		}
	}

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
	classOptions, err := protojson.Marshal(specClassOptions(player))
	fail(err)

	talents := map[string]int32{}
	effects := []map[string]any{}
	if reflected := classTalents(agent); reflected != nil {
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
	}
	if exported {
		exportRequest, classNotes = request, &unrepresented
		effects = append(effects, class.effects(agent, character)...)
		exportRequest, classNotes = nil, nil
		if class.unrepresented != nil {
			unrepresented = append(unrepresented, class.unrepresented(agent, character)...)
		}
	}
	effects = append(effects, commonEffects(character, target, request, &unrepresented)...)
	effects = append(effects, inertPets...)
	effects = append(effects, meleeProcEffects(simulation, character, &unrepresented)...)
	effects = append(effects, energyProcEffects(simulation, character, &unrepresented)...)
	if statAuras := statAurasEffect(request, character, class, agent); statAuras != nil {
		effects = append(effects, statAuras)
	}
	if eureka := eurekaEffect(agent, character); eureka != nil {
		effects = append(effects, eureka)
	}
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
		{&character.Unit, "player", "Parry Haste", "acts only on attacks the player parries, and nothing attacks the player"},
	} {
		if inert.label == core.ChanceOfDeathAuraLabel && playerTakesDamage(character) {
			continue
		}
		if inert.unit.GetAura(inert.label) != nil {
			effects = append(effects, map[string]any{"kind": "inert_listener", "unit": inert.side, "aura": inert.label, "reason": inert.reason})
		}
	}
	// health.go trackChanceOfDeath: once a spell can hit the player, the listener removes health.
	if playerTakesDamage(character) && character.GetAura(core.ChanceOfDeathAuraLabel) != nil {
		effects = append(effects, map[string]any{"kind": "chance_of_death", "aura": core.ChanceOfDeathAuraLabel})
	}
	effects = append(effects, meleeItemListeners(character)...)

	professions := []string{}
	for _, profession := range []proto.Profession{player.Profession1, player.Profession2} {
		if profession != proto.Profession_ProfessionUnknown {
			professions = append(professions, profession.String())
		}
	}

	energy := exportEnergy(character, &unrepresented)
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
			Energy: energy,
			AttackTable: AttackTable{BaseSpellMissChance: table.BaseSpellMissChance, SpellCritSuppression: table.SpellCritSuppression,
				BonusSpellCritPercent: table.BonusSpellCritPercent, CritMultiplier: table.CritMultiplier,
				DamageDealtMultiplier: table.DamageDealtMultiplier, DamageTakenMultiplier: table.DamageTakenMultiplier},
			Spells: spells, MajorCooldowns: mcds, Rotation: rotation,
			PrepullActions: privateField(simulation.Environment, "prepullActions").Len(),
		},
		Effects: effects, Unrepresented: unrepresented,
	}
	prepared.Melee = exportMelee(character, target, table, &prepared.Unrepresented)
	// Last: the teardown changes the simulation.
	prepared.Player.Mana.TeardownMax = teardownMaxMana(simulation, &character.Unit, &prepared.Unrepresented)
	return prepared
}

// A pet that is registered but never enabled. Each reset enables its unit and its agent's
// Reset dismisses it, logging its stats; each fight's end logs that no pet is summoned. Its
// metrics report zero, with every action and aura it registered.
func inertPetEffect(pet *core.Pet, reason string) map[string]any {
	auras := []*ActionID{}
	for _, aura := range pet.GetAuras() {
		if id := actionID(aura.ActionID); id != nil {
			auras = append(auras, id)
		}
	}
	return map[string]any{
		"kind": "inert_pet", "name": pet.Name, "label": pet.Label, "unit_index": pet.UnitIndex,
		"metrics_actions": metricsActions(&pet.Unit), "auras": auras,
		"dismissed_log": pet.GetStats().FlatString(), "reason": reason,
	}
}

// Item procs (common/forever/stat_bonus_procs_auto_gen.go) whose listener, decoded by spelldata's
// ProcTrigger from the trigger row, hears only melee hits. Melee autos need auto attacks, which are
// unrepresented, so the listener never acts unless a spell with a melee special mask exists.
var meleeItemProcs = []struct {
	label   string
	trigger int32
}{{"Storm Gauntlets", 16615}}

func meleeItemListeners(character *core.Character) []map[string]any {
	for _, spell := range character.Spellbook {
		if spell.ProcMask.Matches(core.ProcMaskMeleeSpecial) {
			return nil
		}
	}
	effects := []map[string]any{}
	for _, item := range meleeItemProcs {
		if character.GetAura(item.label) == nil {
			continue
		}
		listener := spelldata.ProcTrigger(character, spelldata.Find(item.trigger), nil)
		if listener.ProcMask == core.ProcMaskUnknown || listener.ProcMask&^core.ProcMaskMelee != 0 {
			continue
		}
		effects = append(effects, map[string]any{
			"kind": "inert_listener", "unit": "player", "aura": item.label, "reason": "acts only on melee hits",
		})
	}
	return effects
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
