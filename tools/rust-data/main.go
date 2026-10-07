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

	"google.golang.org/protobuf/reflect/protoreflect"
	"google.golang.org/protobuf/reflect/protoregistry"

	"github.com/wowsims/forever/sim"
	"github.com/wowsims/forever/sim/core/proto"
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

func main() {
	sim.RegisterAll()
	if len(os.Args) != 2 {
		fail(fmt.Errorf("usage: rust-data OUTPUT_FOLDER"))
	}
	dir := os.Args[1]
	fail(os.MkdirAll(dir, 0o755))
	write(dir, "proto-schema.json", schema())
}
