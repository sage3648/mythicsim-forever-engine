// Command rust-data writes the data Rust preparation reads from the pinned Go reference.
//
// It is built inside a checkout of the pinned reference, as tools/oracle-v2 is, and writes
// versioned JSON files into one output folder. tools/rust_data.py builds and runs it and
// records each file's digest in data/manifest.json. Rust never runs it.
package main

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sort"

	"google.golang.org/protobuf/encoding/protojson"
	"google.golang.org/protobuf/reflect/protoreflect"
	"google.golang.org/protobuf/reflect/protoregistry"

	"github.com/wowsims/forever/assets/database"
	"github.com/wowsims/forever/sim"
	"github.com/wowsims/forever/sim/core"
	"github.com/wowsims/forever/sim/core/proto"
	"github.com/wowsims/forever/sim/core/spelldata"
	"github.com/wowsims/forever/sim/core/stats"
)

func fail(err error) {
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}

func write(dir, name string, value any) {
	data, err := json.MarshalIndent(value, "", " ")
	fail(err)
	fail(os.WriteFile(filepath.Join(dir, name), append(data, '\n'), 0o644))
}

type Field struct {
	Name     string `json:"name"`
	JSONName string `json:"json_name"`
	Number   int32  `json:"number"`
	Kind     string `json:"kind"`
	Repeated bool   `json:"repeated,omitempty"`
	Packed   bool   `json:"packed,omitempty"`
	Optional bool   `json:"optional,omitempty"`
	Oneof    string `json:"oneof,omitempty"`
	// The oneof's declaration index: Go marshals oneof members after every other field,
	// ordered by this index (protobuf-go order.LegacyFieldOrder).
	OneofIndex *int   `json:"oneof_index,omitempty"`
	Type       string `json:"type,omitempty"`
	MapKey     *Field `json:"map_key,omitempty"`
	MapValue   *Field `json:"map_value,omitempty"`
}

type Message struct {
	Fields []Field `json:"fields"`
}

type EnumValue struct {
	Name   string `json:"name"`
	Number int32  `json:"number"`
}

type Schema struct {
	Messages map[string]Message     `json:"messages"`
	Enums    map[string][]EnumValue `json:"enums"`
}

func field(f protoreflect.FieldDescriptor) Field {
	out := Field{Name: string(f.Name()), JSONName: f.JSONName(), Number: int32(f.Number()), Kind: f.Kind().String()}
	if f.IsMap() {
		key, value := field(f.MapKey()), field(f.MapValue())
		out.MapKey, out.MapValue = &key, &value
		return out
	}
	out.Repeated = f.Cardinality() == protoreflect.Repeated
	out.Packed = f.IsPacked()
	out.Optional = f.HasOptionalKeyword()
	if oneof := f.ContainingOneof(); oneof != nil && !oneof.IsSynthetic() {
		out.Oneof = string(oneof.Name())
		index := oneof.Index()
		out.OneofIndex = &index
	}
	switch f.Kind() {
	case protoreflect.MessageKind, protoreflect.GroupKind:
		out.Type = string(f.Message().FullName())
	case protoreflect.EnumKind:
		out.Type = string(f.Enum().FullName())
	}
	return out
}

// Every message and enum in the reference's proto package, with field numbers, so Rust can
// read protojson as Go does and encode the deterministic protobuf the request digest hashes.
func schema() Schema {
	out := Schema{Messages: map[string]Message{}, Enums: map[string][]EnumValue{}}
	var addMessage func(m protoreflect.MessageDescriptor)
	var addEnum func(e protoreflect.EnumDescriptor)
	addEnum = func(e protoreflect.EnumDescriptor) {
		values := []EnumValue{}
		for i := 0; i < e.Values().Len(); i++ {
			v := e.Values().Get(i)
			values = append(values, EnumValue{Name: string(v.Name()), Number: int32(v.Number())})
		}
		out.Enums[string(e.FullName())] = values
	}
	addMessage = func(m protoreflect.MessageDescriptor) {
		if m.IsMapEntry() {
			return
		}
		message := Message{Fields: []Field{}}
		for i := 0; i < m.Fields().Len(); i++ {
			message.Fields = append(message.Fields, field(m.Fields().Get(i)))
		}
		sort.Slice(message.Fields, func(i, j int) bool { return message.Fields[i].Number < message.Fields[j].Number })
		out.Messages[string(m.FullName())] = message
		for i := 0; i < m.Messages().Len(); i++ {
			addMessage(m.Messages().Get(i))
		}
		for i := 0; i < m.Enums().Len(); i++ {
			addEnum(m.Enums().Get(i))
		}
	}
	package_ := (&proto.RaidSimRequest{}).ProtoReflect().Descriptor().ParentFile().Package()
	protoregistry.GlobalFiles.RangeFilesByPackage(package_, func(file protoreflect.FileDescriptor) bool {
		for i := 0; i < file.Messages().Len(); i++ {
			addMessage(file.Messages().Get(i))
		}
		for i := 0; i < file.Enums().Len(); i++ {
			addEnum(file.Enums().Get(i))
		}
		return true
	})
	return out
}

// The item database as core/database_load.go builds it from the embedded db.bin and
// leftover_db.bin, as protojson of proto.SimDatabase.
func simDatabase() []byte {
	db := database.Load()
	simDB := &proto.SimDatabase{}
	for _, item := range db.Items {
		simDB.Items = append(simDB.Items, &proto.SimItem{
			Id: item.Id, Name: item.Name, Type: item.Type, ArmorType: item.ArmorType,
			WeaponType: item.WeaponType, HandType: item.HandType, RangedWeaponType: item.RangedWeaponType,
			GemSockets: item.GemSockets, SocketBonus: item.SocketBonus, PseudoStats: item.PseudoStats,
			WeaponSpeed: item.WeaponSpeed, QualityModifier: item.QualityModifier, Unique: item.Unique,
			LimitCategory: item.LimitCategory, SetName: item.SetName, SetId: item.SetId,
			ClassAllowlist: item.ClassAllowlist, ScalingOptions: item.ScalingOptions, ItemEffects: item.ItemEffects,
		})
	}
	for _, suffix := range db.RandomSuffixes {
		simDB.RandomSuffixes = append(simDB.RandomSuffixes, &proto.ItemRandomSuffix{Id: suffix.Id, Name: suffix.Name, Stats: suffix.Stats})
	}
	for _, enchant := range db.Enchants {
		simDB.Enchants = append(simDB.Enchants, &proto.SimEnchant{
			EffectId: enchant.EffectId, Stats: enchant.Stats, PseudoStats: enchant.PseudoStats,
			WeaponDamage: enchant.WeaponDamage, EnchantEffects: enchant.EnchantEffects, Name: enchant.Name,
			Type: enchant.Type, EnchantType: enchant.EnchantType, ExtraTypes: enchant.ExtraTypes,
		})
	}
	for _, gem := range db.Gems {
		simDB.Gems = append(simDB.Gems, &proto.SimGem{Id: gem.Id, Name: gem.Name, Color: gem.Color, Stats: gem.Stats})
	}
	for _, points := range db.ItemEffectRandPropPoints {
		simDB.ItemEffectRandPropPoints = append(simDB.ItemEffectRandPropPoints,
			&proto.ItemEffectRandPropPoints{Ilvl: points.Ilvl, RandPropPoints: points.RandPropPoints})
	}
	simDB.Consumables = db.Consumables
	simDB.SpellEffects = db.SpellEffects
	data, err := protojson.MarshalOptions{UseProtoNames: true}.Marshal(simDB)
	fail(err)
	return data
}

// The client spell rows the generated spelldata store carries, with its talent curves and the
// spells server side handlers cast.
type spellRows struct {
	Spells       []*spelldata.Spell    `json:"spells"`
	Curves       map[int32][][]float64 `json:"curves"`
	HandTriggers map[int32][]int32     `json:"hand_triggers"`
}

func statsByName(values stats.Stats) map[string]float64 {
	out := map[string]float64{}
	for index, value := range values {
		if value != 0 {
			out[stats.Stat(index).StatName()] = value
		}
	}
	return out
}

// Go tables preparation reads, keyed by enum names.
type tables struct {
	BaseStats          map[string]map[string]map[string]float64 `json:"base_stats"`
	CritPerAgiMaxLevel map[string]float64                       `json:"crit_per_agi_max_level"`
	CritPerIntMaxLevel map[string]float64                       `json:"crit_per_int_max_level"`
	// Items and enchants whose effects Go registers in code: Rust must implement each one it
	// meets or refuse the request.
	ItemEffectIDs    []int32 `json:"item_effect_ids"`
	EnchantEffectIDs []int32 `json:"enchant_effect_ids"`
	// Preset target IDs whose target has an AI.
	PresetTargetsWithAI []int32 `json:"preset_targets_with_ai"`
	// The item sets Go registers, in the order its set bonus search reads them.
	ItemSets []core.RustDataItemSet `json:"item_sets"`
}

func goTables() tables {
	out := tables{BaseStats: map[string]map[string]map[string]float64{},
		CritPerAgiMaxLevel: map[string]float64{}, CritPerIntMaxLevel: map[string]float64{}}
	for key, values := range core.BaseStats {
		race, class := key.Race.String(), key.Class.String()
		if out.BaseStats[class] == nil {
			out.BaseStats[class] = map[string]map[string]float64{}
		}
		out.BaseStats[class][race] = statsByName(values)
	}
	for class, value := range core.CritPerAgiMaxLevel {
		out.CritPerAgiMaxLevel[class.String()] = value
	}
	for class, value := range core.CritPerIntMaxLevel {
		out.CritPerIntMaxLevel[class.String()] = value
	}
	out.ItemEffectIDs = core.RegisteredItemEffectIDs()
	out.EnchantEffectIDs = core.RegisteredEnchantEffectIDs()
	out.PresetTargetsWithAI = core.RustDataPresetTargetsWithAI()
	out.ItemSets = core.RustDataItemSets()
	return out
}

func main() {
	sim.RegisterAll()
	if len(os.Args) != 2 {
		fail(fmt.Errorf("usage: rust-data OUTPUT_FOLDER"))
	}
	dir := os.Args[1]
	fail(os.MkdirAll(dir, 0o755))
	write(dir, "proto-schema.json", schema())
	fail(os.WriteFile(filepath.Join(dir, "sim-database.json"), append(simDatabase(), '\n'), 0o644))
	data, err := json.Marshal(spellRows{Spells: spelldata.All(), Curves: spelldata.RustDataCurves(),
		HandTriggers: spelldata.RustDataHandTriggers()})
	fail(err)
	fail(os.WriteFile(filepath.Join(dir, "spells.json"), append(data, '\n'), 0o644))
	write(dir, "go-tables.json", goTables())
}
