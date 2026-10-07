//! A JSON tree that keeps what protojson needs and serde_json drops: object members in
//! order with duplicates, and each number's text, so integers and 32 bit floats are read
//! from the digits as Go reads them.

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Json {
    Null,
    Bool(bool),
    /// The number exactly as written.
    Number(String),
    String(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub(crate) fn kind(&self) -> &'static str {
        match self {
            Json::Null => "null",
            Json::Bool(_) => "a bool",
            Json::Number(_) => "a number",
            Json::String(_) => "a string",
            Json::Array(_) => "an array",
            Json::Object(_) => "an object",
        }
    }
}

/// Parses one JSON document, which may be surrounded by whitespace.
pub(crate) fn parse(bytes: &[u8]) -> Result<Json, String> {
    let text = std::str::from_utf8(bytes).map_err(|err| format!("invalid UTF-8: {err}"))?;
    let mut parser = Parser { text, at: 0 };
    let value = parser.value(0)?;
    parser.space();
    if parser.at != text.len() {
        return Err(parser.error("unexpected text after the document"));
    }
    Ok(value)
}

/// Nesting deeper than this is refused rather than overflowing the stack.
const MAX_DEPTH: usize = 128;

struct Parser<'a> {
    text: &'a str,
    at: usize,
}

impl Parser<'_> {
    fn error(&self, message: &str) -> String {
        format!("invalid JSON at byte {}: {message}", self.at)
    }

    fn peek(&self) -> Option<u8> {
        self.text.as_bytes().get(self.at).copied()
    }

    fn space(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn literal(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.text[self.at..].starts_with(word) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(self.error("unknown literal"))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Json, String> {
        if depth > MAX_DEPTH {
            return Err(self.error("nested too deeply"));
        }
        self.space();
        match self.peek() {
            None => Err(self.error("unexpected end")),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'"') => self.string().map(Json::String),
            Some(b'[') => {
                self.at += 1;
                let mut items = Vec::new();
                self.space();
                if self.peek() == Some(b']') {
                    self.at += 1;
                    return Ok(Json::Array(items));
                }
                loop {
                    items.push(self.value(depth + 1)?);
                    self.space();
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b']') => {
                            self.at += 1;
                            return Ok(Json::Array(items));
                        }
                        _ => return Err(self.error("expected , or ]")),
                    }
                }
            }
            Some(b'{') => {
                self.at += 1;
                let mut members = Vec::new();
                self.space();
                if self.peek() == Some(b'}') {
                    self.at += 1;
                    return Ok(Json::Object(members));
                }
                loop {
                    self.space();
                    if self.peek() != Some(b'"') {
                        return Err(self.error("expected a member name"));
                    }
                    let name = self.string()?;
                    self.space();
                    if self.peek() != Some(b':') {
                        return Err(self.error("expected :"));
                    }
                    self.at += 1;
                    members.push((name, self.value(depth + 1)?));
                    self.space();
                    match self.peek() {
                        Some(b',') => self.at += 1,
                        Some(b'}') => {
                            self.at += 1;
                            return Ok(Json::Object(members));
                        }
                        _ => return Err(self.error("expected , or }")),
                    }
                }
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("unexpected character")),
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        let bytes = self.text.as_bytes();
        let digits = |at: &mut usize| {
            let from = *at;
            while matches!(bytes.get(*at), Some(b'0'..=b'9')) {
                *at += 1;
            }
            *at > from
        };
        let mut at = self.at;
        if bytes.get(at) == Some(&b'-') {
            at += 1;
        }
        if bytes.get(at) == Some(&b'0') {
            at += 1;
        } else if !digits(&mut at) {
            return Err(self.error("invalid number"));
        }
        if bytes.get(at) == Some(&b'.') {
            at += 1;
            if !digits(&mut at) {
                return Err(self.error("invalid fraction"));
            }
        }
        if matches!(bytes.get(at), Some(b'e' | b'E')) {
            at += 1;
            if matches!(bytes.get(at), Some(b'+' | b'-')) {
                at += 1;
            }
            if !digits(&mut at) {
                return Err(self.error("invalid exponent"));
            }
        }
        self.at = at;
        Ok(Json::Number(self.text[start..at].to_string()))
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let digits = self
            .text
            .get(self.at..self.at + 4)
            .ok_or_else(|| self.error("short \\u escape"))?;
        let value = u32::from_str_radix(digits, 16).map_err(|_| self.error("bad \\u escape"))?;
        self.at += 4;
        Ok(value)
    }

    fn string(&mut self) -> Result<String, String> {
        self.at += 1; // opening quote
        let mut out = String::new();
        loop {
            let rest = &self.text[self.at..];
            let Some(next) = rest.find(['"', '\\']) else {
                return Err(self.error("unterminated string"));
            };
            let chunk = &rest[..next];
            if chunk.chars().any(|c| (c as u32) < 0x20) {
                return Err(self.error("control character in string"));
            }
            out.push_str(chunk);
            self.at += next;
            if self.peek() == Some(b'"') {
                self.at += 1;
                return Ok(out);
            }
            self.at += 1; // backslash
            let escape = self
                .peek()
                .ok_or_else(|| self.error("unterminated escape"))?;
            self.at += 1;
            match escape {
                b'"' => out.push('"'),
                b'\\' => out.push('\\'),
                b'/' => out.push('/'),
                b'b' => out.push('\u{8}'),
                b'f' => out.push('\u{c}'),
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'u' => {
                    let high = self.hex4()?;
                    let code = if (0xd800..0xdc00).contains(&high)
                        && self.text[self.at..].starts_with("\\u")
                    {
                        self.at += 2;
                        let low = self.hex4()?;
                        if !(0xdc00..0xe000).contains(&low) {
                            return Err(self.error("invalid surrogate pair"));
                        }
                        0x10000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                    } else {
                        high
                    };
                    // Go replaces an unpaired surrogate with U+FFFD.
                    out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                }
                _ => return Err(self.error("unknown escape")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_order_duplicates_and_number_text() {
        let value =
            parse(r#" {"b": 1.50, "a": [true, null], "b": "x\u00e9"} "#.as_bytes()).unwrap();
        assert_eq!(
            value,
            Json::Object(vec![
                ("b".into(), Json::Number("1.50".into())),
                ("a".into(), Json::Array(vec![Json::Bool(true), Json::Null])),
                ("b".into(), Json::String("x\u{e9}".into())),
            ])
        );
    }

    #[test]
    fn refuses_malformed_documents() {
        for input in [
            "",
            "{",
            "[1,]",
            "01",
            "1.",
            "{\"a\" 1}",
            "tru",
            "\"a",
            "{} x",
        ] {
            assert!(parse(input.as_bytes()).is_err(), "{input}");
        }
    }
}
