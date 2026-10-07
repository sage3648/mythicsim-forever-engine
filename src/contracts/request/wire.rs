//! Protojson decoding and deterministic protobuf encoding over the schema.

use std::collections::BTreeMap;

use super::{
    json::Json,
    schema::{self, FieldKind, FieldSchema, MessageSchema},
    Message, Value,
};

pub(super) fn decode_message(
    schema: &'static MessageSchema,
    raw: &Json,
    path: &str,
) -> Result<Message, String> {
    let Json::Object(members) = raw else {
        return Err(format!("{path}: expected an object, found {}", raw.kind()));
    };
    let mut fields = BTreeMap::new();
    let mut seen = Vec::<u32>::new();
    let mut oneofs = Vec::<&str>::new();
    for (name, value) in members {
        let field = schema
            .field(name)
            .ok_or_else(|| format!("{path}: unknown field {name:?}"))?;
        if seen.contains(&field.number) {
            return Err(format!("{path}: duplicate field {name:?}"));
        }
        seen.push(field.number);
        let field_path = format!("{path}.{}", field.name);
        // protojson: null leaves any field unset.
        if *value == Json::Null {
            continue;
        }
        if let Some(oneof) = field.oneof.as_deref() {
            if oneofs.contains(&oneof) {
                return Err(format!("{path}: oneof {oneof} is already set"));
            }
            oneofs.push(oneof);
        }
        let decoded = if let (Some(key), Some(item)) = (&field.map_key, &field.map_value) {
            let Json::Object(entries) = value else {
                return Err(format!(
                    "{field_path}: expected an object, found {}",
                    value.kind()
                ));
            };
            let mut map = Vec::new();
            for (key_text, item_value) in entries {
                let key_value = map_key(key, key_text, &field_path)?;
                if map.iter().any(|(existing, _)| *existing == key_value) {
                    return Err(format!("{field_path}: duplicate map key {key_text:?}"));
                }
                if *item_value == Json::Null {
                    return Err(format!("{field_path}[{key_text}]: null map value"));
                }
                let entry_path = format!("{field_path}[{key_text}]");
                map.push((key_value, singular(item, item_value, &entry_path)?));
            }
            if map.is_empty() {
                continue;
            }
            Value::Map(map)
        } else if field.repeated {
            let Json::Array(items) = value else {
                return Err(format!(
                    "{field_path}: expected an array, found {}",
                    value.kind()
                ));
            };
            let mut list = Vec::new();
            for (index, item) in items.iter().enumerate() {
                let item_path = format!("{field_path}[{index}]");
                if *item == Json::Null {
                    return Err(format!("{item_path}: null list element"));
                }
                list.push(singular(field, item, &item_path)?);
            }
            if list.is_empty() {
                continue;
            }
            Value::List(list)
        } else {
            let decoded = singular(field, value, &field_path)?;
            if !field.has_presence() && is_default(&decoded) {
                continue;
            }
            decoded
        };
        fields.insert(field.number, decoded);
    }
    Ok(Message {
        type_name: schema.name,
        fields,
    })
}

fn is_default(value: &Value) -> bool {
    match value {
        Value::Bool(value) => !value,
        Value::Int(value) => *value == 0,
        Value::Uint(value) => *value == 0,
        // Go keeps a negative zero: it has a sign bit.
        Value::Float(value) => value.to_bits() == 0,
        Value::Double(value) => value.to_bits() == 0,
        Value::String(value) => value.is_empty(),
        Value::Bytes(value) => value.is_empty(),
        Value::Enum(value) => *value == 0,
        Value::Message(_) | Value::List(_) | Value::Map(_) => false,
    }
}

fn map_key(field: &FieldSchema, text: &str, path: &str) -> Result<Value, String> {
    match field.kind {
        FieldKind::String => Ok(Value::String(text.to_string())),
        FieldKind::Bool => match text {
            "true" => Ok(Value::Bool(true)),
            "false" => Ok(Value::Bool(false)),
            _ => Err(format!("{path}: invalid bool map key {text:?}")),
        },
        _ => singular(field, &Json::Number(text.to_string()), path),
    }
}

/// An integer written as a JSON number or its text, as Go's protojson reads one: a fraction
/// or exponent is allowed when the value is whole.
fn integral(text: &str) -> Option<i128> {
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (mantissa, exponent) = match rest.find(['e', 'E']) {
        Some(at) => (&rest[..at], rest[at + 1..].parse::<i64>().ok()?),
        None => (rest, 0),
    };
    let (whole, fraction) = match mantissa.find('.') {
        Some(at) => (&mantissa[..at], &mantissa[at + 1..]),
        None => (mantissa, ""),
    };
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut digits = format!("{whole}{fraction}");
    let mut scale = exponent - fraction.len() as i64;
    while scale < 0 {
        // Dropped digits must be zeros for the value to be whole.
        if !digits.ends_with('0') {
            if digits.trim_start_matches('0').is_empty() {
                break;
            }
            return None;
        }
        digits.pop();
        scale += 1;
    }
    if scale > 40 {
        return None;
    }
    let digits = digits.trim_start_matches('0');
    if digits.len() + scale.max(0) as usize > 38 {
        return None;
    }
    let mut value: i128 = if digits.is_empty() {
        0
    } else {
        digits.parse().ok()?
    };
    for _ in 0..scale.max(0) {
        value = value.checked_mul(10)?;
    }
    Some(if negative { -value } else { value })
}

fn number_text<'a>(raw: &'a Json, path: &str) -> Result<&'a str, String> {
    match raw {
        Json::Number(text) => Ok(text),
        Json::String(text) => Ok(text),
        other => Err(format!("{path}: expected a number, found {}", other.kind())),
    }
}

fn float(raw: &Json, path: &str) -> Result<f64, String> {
    let text = number_text(raw, path)?;
    if let Json::String(_) = raw {
        match text {
            "NaN" => return Ok(f64::NAN),
            "Infinity" => return Ok(f64::INFINITY),
            "-Infinity" => return Ok(f64::NEG_INFINITY),
            _ => {}
        }
        // The text must itself be a JSON number.
        if super::json::parse(text.as_bytes())
            .ok()
            .filter(|value| matches!(value, Json::Number(_)))
            .is_none()
        {
            return Err(format!("{path}: invalid number {text:?}"));
        }
    }
    let value: f64 = text
        .parse()
        .map_err(|_| format!("{path}: invalid number {text:?}"))?;
    if value.is_infinite() {
        return Err(format!("{path}: number {text:?} is out of range"));
    }
    Ok(value)
}

fn singular(field: &FieldSchema, raw: &Json, path: &str) -> Result<Value, String> {
    let int_in = |low: i128, high: i128| -> Result<i128, String> {
        let text = number_text(raw, path)?;
        let value = integral(text).ok_or_else(|| format!("{path}: {text:?} is not an integer"))?;
        if value < low || value > high {
            return Err(format!("{path}: {text} is out of range"));
        }
        Ok(value)
    };
    Ok(match field.kind {
        FieldKind::Bool => match raw {
            Json::Bool(value) => Value::Bool(*value),
            other => return Err(format!("{path}: expected a bool, found {}", other.kind())),
        },
        FieldKind::Int32 | FieldKind::Sint32 | FieldKind::Sfixed32 => {
            Value::Int(int_in(i32::MIN.into(), i32::MAX.into())? as i64)
        }
        FieldKind::Int64 | FieldKind::Sint64 | FieldKind::Sfixed64 => {
            Value::Int(int_in(i64::MIN.into(), i64::MAX.into())? as i64)
        }
        FieldKind::Uint32 | FieldKind::Fixed32 => Value::Uint(int_in(0, u32::MAX.into())? as u64),
        FieldKind::Uint64 | FieldKind::Fixed64 => Value::Uint(int_in(0, u64::MAX.into())? as u64),
        FieldKind::Double => Value::Double(float(raw, path)?),
        FieldKind::Float => {
            let value = float(raw, path)?;
            if value.is_finite() && (value as f32).is_infinite() {
                return Err(format!("{path}: number is out of float range"));
            }
            Value::Float(value as f32)
        }
        FieldKind::String => match raw {
            Json::String(value) => Value::String(value.clone()),
            other => return Err(format!("{path}: expected a string, found {}", other.kind())),
        },
        FieldKind::Bytes => match raw {
            Json::String(value) => {
                Value::Bytes(base64(value).ok_or_else(|| format!("{path}: invalid base64"))?)
            }
            other => return Err(format!("{path}: expected base64, found {}", other.kind())),
        },
        FieldKind::Enum => {
            let enum_name = field
                .type_name
                .as_deref()
                .expect("an enum field names its enum");
            match raw {
                Json::String(name) => {
                    Value::Enum(schema::enum_value(enum_name, name).ok_or_else(|| {
                        format!("{path}: unknown enum value {name:?} for {enum_name}")
                    })?)
                }
                Json::Number(_) => Value::Enum(int_in(i32::MIN.into(), i32::MAX.into())? as i32),
                other => return Err(format!("{path}: expected an enum, found {}", other.kind())),
            }
        }
        FieldKind::Message => {
            let type_name = field
                .type_name
                .as_deref()
                .expect("a message field names its type");
            Value::Message(decode_message(schema::message(type_name)?, raw, path)?)
        }
    })
}

fn base64(text: &str) -> Option<Vec<u8>> {
    let url = text.contains(['-', '_']);
    let trimmed = text.trim_end_matches('=');
    let mut out = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;
    for byte in trimmed.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' if !url => 62,
            b'/' if !url => 63,
            b'-' if url => 62,
            b'_' if url => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }
    Some(out)
}

fn varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn tag(number: u32, wire_type: u8, out: &mut Vec<u8>) {
    varint((u64::from(number) << 3) | u64::from(wire_type), out);
}

fn wire_type(kind: FieldKind) -> u8 {
    match kind {
        FieldKind::Double | FieldKind::Fixed64 | FieldKind::Sfixed64 => 1,
        FieldKind::Float | FieldKind::Fixed32 | FieldKind::Sfixed32 => 5,
        FieldKind::String | FieldKind::Bytes | FieldKind::Message => 2,
        _ => 0,
    }
}

/// A scalar's payload without its tag.
fn scalar(kind: FieldKind, value: &Value, out: &mut Vec<u8>) {
    match (kind, value) {
        (FieldKind::Bool, Value::Bool(value)) => varint(u64::from(*value), out),
        (FieldKind::Int32 | FieldKind::Int64, Value::Int(value)) => varint(*value as u64, out),
        (FieldKind::Enum, Value::Enum(value)) => varint(i64::from(*value) as u64, out),
        (FieldKind::Uint32 | FieldKind::Uint64, Value::Uint(value)) => varint(*value, out),
        (FieldKind::Sint32, Value::Int(value)) => {
            let value = *value as i32;
            varint(u64::from(((value << 1) ^ (value >> 31)) as u32), out)
        }
        (FieldKind::Sint64, Value::Int(value)) => {
            varint(((value << 1) ^ (value >> 63)) as u64, out)
        }
        (FieldKind::Fixed32, Value::Uint(value)) => out.extend((*value as u32).to_le_bytes()),
        (FieldKind::Sfixed32, Value::Int(value)) => out.extend((*value as i32).to_le_bytes()),
        (FieldKind::Fixed64, Value::Uint(value)) => out.extend(value.to_le_bytes()),
        (FieldKind::Sfixed64, Value::Int(value)) => out.extend(value.to_le_bytes()),
        (FieldKind::Float, Value::Float(value)) => out.extend(value.to_le_bytes()),
        (FieldKind::Double, Value::Double(value)) => out.extend(value.to_le_bytes()),
        (FieldKind::String, Value::String(value)) => {
            varint(value.len() as u64, out);
            out.extend(value.as_bytes());
        }
        (FieldKind::Bytes, Value::Bytes(value)) => {
            varint(value.len() as u64, out);
            out.extend(value);
        }
        (FieldKind::Message, Value::Message(message)) => {
            let body = message.encode();
            varint(body.len() as u64, out);
            out.extend(body);
        }
        (kind, value) => panic!("a {kind:?} field holds {value:?}"),
    }
}

/// Go's deterministic map order: keys ascending, strings by bytes, bools false first.
fn key_order(a: &Value, b: &Value) -> std::cmp::Ordering {
    match (a, b) {
        (Value::String(a), Value::String(b)) => a.as_bytes().cmp(b.as_bytes()),
        (Value::Int(a), Value::Int(b)) => a.cmp(b),
        (Value::Uint(a), Value::Uint(b)) => a.cmp(b),
        (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
        (a, b) => panic!("map keys of different kinds: {a:?} and {b:?}"),
    }
}

pub(super) fn encode_message(message: &Message, out: &mut Vec<u8>) {
    let schema = schema::message(message.type_name).expect("a decoded message has a schema");
    // protobuf-go marshals in order.LegacyFieldOrder: fields outside a oneof by number, then
    // oneof members by their oneof's declaration index and then by number.
    let mut ordered: Vec<(&FieldSchema, &Value)> = message
        .fields
        .iter()
        .map(|(number, value)| (schema.by_number(*number).expect("set field"), value))
        .collect();
    ordered
        .sort_by_key(|(field, _)| (field.oneof_index.map_or(0, |index| index + 1), field.number));
    for (field, value) in ordered {
        let number = &field.number;
        match value {
            Value::Map(entries) => {
                let (key, item) = (
                    field.map_key.as_deref().expect("map key"),
                    field.map_value.as_deref().expect("map value"),
                );
                let mut sorted: Vec<&(Value, Value)> = entries.iter().collect();
                sorted.sort_by(|a, b| key_order(&a.0, &b.0));
                for (key_value, item_value) in sorted {
                    // An entry always writes its key and value, defaults included.
                    let mut entry = Vec::new();
                    tag(1, wire_type(key.kind), &mut entry);
                    scalar(key.kind, key_value, &mut entry);
                    tag(2, wire_type(item.kind), &mut entry);
                    scalar(item.kind, item_value, &mut entry);
                    tag(*number, 2, out);
                    varint(entry.len() as u64, out);
                    out.extend(entry);
                }
            }
            Value::List(items) if field.packed => {
                let mut body = Vec::new();
                for item in items {
                    scalar(field.kind, item, &mut body);
                }
                tag(*number, 2, out);
                varint(body.len() as u64, out);
                out.extend(body);
            }
            Value::List(items) => {
                for item in items {
                    tag(*number, wire_type(field.kind), out);
                    scalar(field.kind, item, out);
                }
            }
            value => {
                tag(*number, wire_type(field.kind), out);
                scalar(field.kind, value, out);
            }
        }
    }
}

fn json_float(value: f64) -> serde_json::Value {
    if value.is_nan() {
        "NaN".into()
    } else if value.is_infinite() {
        if value > 0.0 { "Infinity" } else { "-Infinity" }.into()
    } else {
        serde_json::Number::from_f64(value)
            .map(serde_json::Value::Number)
            .expect("a finite float")
    }
}

fn json_value(field: &FieldSchema, value: &Value) -> serde_json::Value {
    match value {
        Value::Bool(value) => (*value).into(),
        Value::Int(value) => match field.kind {
            FieldKind::Int64 | FieldKind::Sint64 | FieldKind::Sfixed64 => value.to_string().into(),
            _ => (*value).into(),
        },
        Value::Uint(value) => match field.kind {
            FieldKind::Uint64 | FieldKind::Fixed64 => value.to_string().into(),
            _ => (*value).into(),
        },
        Value::Float(value) => json_float(f64::from(*value)),
        Value::Double(value) => json_float(*value),
        Value::String(value) => value.clone().into(),
        Value::Bytes(value) => {
            const ALPHABET: &[u8; 64] =
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut out = String::new();
            for chunk in value.chunks(3) {
                let bytes = [
                    chunk[0],
                    *chunk.get(1).unwrap_or(&0),
                    *chunk.get(2).unwrap_or(&0),
                ];
                let n =
                    (u32::from(bytes[0]) << 16) | (u32::from(bytes[1]) << 8) | u32::from(bytes[2]);
                for i in 0..4 {
                    if i <= chunk.len() {
                        out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
                    } else {
                        out.push('=');
                    }
                }
            }
            out.into()
        }
        Value::Enum(number) => {
            schema::enum_value_name(field.type_name.as_deref().expect("enum type"), *number)
                .map(serde_json::Value::from)
                .unwrap_or_else(|| (*number).into())
        }
        Value::Message(message) => to_protojson(message),
        Value::List(items) => items.iter().map(|item| json_value(field, item)).collect(),
        Value::Map(entries) => {
            let item = field.map_value.as_deref().expect("map value");
            serde_json::Value::Object(
                entries
                    .iter()
                    .map(|(key, value)| {
                        let key = match key {
                            Value::String(key) => key.clone(),
                            Value::Int(key) => key.to_string(),
                            Value::Uint(key) => key.to_string(),
                            Value::Bool(key) => key.to_string(),
                            other => panic!("map key {other:?}"),
                        };
                        (key, json_value(item, value))
                    })
                    .collect(),
            )
        }
    }
}

pub(super) fn to_protojson(message: &Message) -> serde_json::Value {
    let schema = schema::message(message.type_name).expect("a decoded message has a schema");
    serde_json::Value::Object(
        message
            .fields
            .iter()
            .map(|(number, value)| {
                let field = schema.by_number(*number).expect("set field");
                (field.json_name.clone(), json_value(field, value))
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_whole_numbers_in_any_json_form() {
        for (text, value) in [
            ("0", Some(0)),
            ("-7", Some(-7)),
            ("1.0", Some(1)),
            ("1e2", Some(100)),
            ("1.5e1", Some(15)),
            ("120000000000", Some(120_000_000_000)),
            ("1.5", None),
            ("1e-1", None),
            ("abc", None),
        ] {
            assert_eq!(integral(text), value, "{text}");
        }
    }

    #[test]
    fn decodes_base64_in_either_alphabet() {
        assert_eq!(base64("aGk=").unwrap(), b"hi");
        assert_eq!(base64("_-8").unwrap(), vec![0xff, 0xef]);
        assert!(base64("a*").is_none());
    }
}
