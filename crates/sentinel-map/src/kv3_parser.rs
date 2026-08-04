use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum KV3Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    Bytes(Vec<u8>),
    Array(Vec<KV3Value>),
    Object(HashMap<String, KV3Value>),
}

impl KV3Value {
    pub fn as_object(&self) -> Option<&HashMap<String, KV3Value>> {
        match self { KV3Value::Object(m) => Some(m), _ => None }
    }
    pub fn as_array(&self) -> Option<&Vec<KV3Value>> {
        match self { KV3Value::Array(a) => Some(a), _ => None }
    }
    pub fn as_bytes(&self) -> Option<&Vec<u8>> {
        match self { KV3Value::Bytes(b) => Some(b), _ => None }
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self { KV3Value::Float(f) => Some(*f), KV3Value::Int(i) => Some(*i as f64), _ => None }
    }
    pub fn as_i64(&self) -> Option<i64> {
        match self { KV3Value::Int(i) => Some(*i), KV3Value::Float(f) => Some(*f as i64), _ => None }
    }
    pub fn get(&self, key: &str) -> Option<&KV3Value> {
        match self { KV3Value::Object(m) => m.get(key), _ => None }
    }
    pub fn as_str(&self) -> Option<&str> {
        match self { KV3Value::String(s) => Some(s), _ => None }
    }
    pub fn as_bool(&self) -> Option<bool> {
        match self { KV3Value::Bool(b) => Some(*b), _ => None }
    }
}

impl fmt::Display for KV3Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KV3Value::Null => write!(f, "null"),
            KV3Value::Bool(b) => write!(f, "{}", b),
            KV3Value::Int(i) => write!(f, "{}", i),
            KV3Value::Float(fl) => write!(f, "{}", fl),
            KV3Value::String(s) => write!(f, "\"{}\"", s),
            KV3Value::Bytes(b) => write!(f, "#[{} bytes]", b.len()),
            KV3Value::Array(_) => write!(f, "[array]"),
            KV3Value::Object(_) => write!(f, "{{object}}"),
        }
    }
}

/// Error type for KV3 parsing
#[derive(Debug)]
pub enum KV3Error {
    UnexpectedEnd,
    UnexpectedChar(char),
    InvalidNumber(String),
    InvalidString(String),
    InvalidHex(String),
    UnexpectedToken(String),
}

impl fmt::Display for KV3Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KV3Error::UnexpectedEnd => write!(f, "Unexpected end of input"),
            KV3Error::UnexpectedChar(c) => write!(f, "Unexpected character: {}", c),
            KV3Error::InvalidNumber(s) => write!(f, "Invalid number: {}", s),
            KV3Error::InvalidString(s) => write!(f, "Invalid string: {}", s),
            KV3Error::InvalidHex(s) => write!(f, "Invalid hex: {}", s),
            KV3Error::UnexpectedToken(t) => write!(f, "Unexpected token: {}", t),
        }
    }
}

impl std::error::Error for KV3Error {}

/// KV3 parser for reading VPhys and other KV3 files
pub struct KV3Parser {
    chars: Vec<char>,
    pos: usize,
}

impl KV3Parser {
    pub fn new(input: &str) -> Self {
        Self { chars: input.chars().collect(), pos: 0 }
    }
    
    fn remaining(&self) -> usize { self.chars.len() - self.pos }
    fn peek(&self) -> Option<char> { self.chars.get(self.pos).copied() }
    fn next_char(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() { self.pos += 1; }
        c
    }
    fn skip_whitespace(&mut self) {
        while self.peek().map(|c| c.is_whitespace()).unwrap_or(false) {
            self.next_char();
        }
    }
    fn skip_comment(&mut self) {
        if self.peek() == Some('/') && self.chars.get(self.pos + 1) == Some(&'/') {
            self.pos += 2;
            while self.peek() != Some('\n') && self.peek().is_some() {
                self.next_char();
            }
        }
    }
    pub fn parse(&mut self) -> Result<KV3Value, KV3Error> {
        self.skip_whitespace();
        let value = self.parse_value()?;
        self.skip_whitespace();
        Ok(value)
    }
    
    fn parse_value(&mut self) -> Result<KV3Value, KV3Error> {
        self.skip_whitespace();
        match self.peek() {
            None => Err(KV3Error::UnexpectedEnd),
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('#') => self.parse_bytes(),
            Some('"') => self.parse_string().map(KV3Value::String),
            Some('t') => self.parse_true(),
            Some('f') => self.parse_false(),
            Some('n') => self.parse_null(),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => {
                let ch = self.next_char();
                Err(KV3Error::UnexpectedChar(ch.unwrap_or('?')))
            }
        }
    }
    
    fn parse_string(&mut self) -> Result<String, KV3Error> {
        self.next_char(); // skip opening quote
        let mut s = String::new();
        loop {
            match self.next_char() {
                None => return Err(KV3Error::InvalidString(s)),
                Some('"') => return Ok(s),
                Some('\\') => {
                    match self.next_char() {
                        None => return Err(KV3Error::InvalidString(s)),
                        Some('"') => s.push('"'),
                        Some('\\') => s.push('\\'),
                        Some('/') => s.push('/'),
                        Some('n') => s.push('\n'),
                        Some('r') => s.push('\r'),
                        Some('t') => s.push('\t'),
                        Some('u') => {
                            let mut hex = String::new();
                            for _ in 0..4 {
                                match self.next_char() {
                                    Some(c) if c.is_ascii_hexdigit() => hex.push(c),
                                    _ => return Err(KV3Error::InvalidString(s)),
                                }
                            }
                            if let Ok(code) = u32::from_str_radix(&hex, 16) {
                                if let Some(ch) = char::from_u32(code) {
                                    s.push(ch);
                                }
                            }
                        }
                        _ => return Err(KV3Error::InvalidString(s)),
                    }
                }
                Some(c) => s.push(c),
            }
        }
    }
    fn parse_number(&mut self) -> Result<KV3Value, KV3Error> {
        let start = self.pos;
        let mut has_dot = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.next_char();
            } else if c == '-' && self.pos == start {
                self.next_char();
            } else if c == '.' {
                if has_dot { break; }
                has_dot = true;
                self.next_char();
            } else if c == 'e' || c == 'E' {
                self.next_char();
                if self.peek() == Some('+') || self.peek() == Some('-') {
                    self.next_char();
                }
                break;
            } else {
                break;
            }
        }
        let num_str: String = self.chars[start..self.pos].iter().collect();
        if has_dot {
            num_str.parse::<f64>()
                .map(KV3Value::Float)
                .map_err(|_| KV3Error::InvalidNumber(num_str))
        } else {
            num_str.parse::<i64>()
                .map(KV3Value::Int)
                .map_err(|_| KV3Error::InvalidNumber(num_str))
        }
    }
    
    fn parse_object(&mut self) -> Result<KV3Value, KV3Error> {
        self.next_char(); // skip {
        let mut map = HashMap::new();
        self.skip_whitespace();
        while self.peek() != Some('}') {
            self.skip_whitespace();
            if self.peek() == None { return Err(KV3Error::UnexpectedEnd); }
            let key = self.parse_string()?;
            self.skip_whitespace();
            match self.next_char() {
                Some(':') => {},
                _ => return Err(KV3Error::UnexpectedChar(self.peek().unwrap_or('?'))),
            }
            self.skip_whitespace();
            let value = self.parse_value()?;
            map.insert(key, value);
            self.skip_whitespace();
            if self.peek() == Some(',') {
                self.next_char();
            }
        }
        self.next_char(); // skip }
        Ok(KV3Value::Object(map))
    }
    fn parse_array(&mut self) -> Result<KV3Value, KV3Error> {
        self.next_char(); // skip [
        let mut arr = Vec::new();
        self.skip_whitespace();
        while self.peek() != Some(']') {
            self.skip_whitespace();
            if self.peek() == None { return Err(KV3Error::UnexpectedEnd); }
            let value = self.parse_value()?;
            arr.push(value);
            self.skip_whitespace();
            if self.peek() == Some(',') {
                self.next_char();
            }
        }
        self.next_char(); // skip ]
        Ok(KV3Value::Array(arr))
    }
    
    fn parse_bytes(&mut self) -> Result<KV3Value, KV3Error> {
        self.next_char(); // skip #
        self.next_char(); // skip [
        let mut bytes = Vec::new();
        self.skip_whitespace();
        while self.peek() != Some(']') {
            self.skip_whitespace();
            if self.peek() == None { return Err(KV3Error::UnexpectedEnd); }
            // Read hex bytes like "0A 1B 2C" or just "AB"
            let mut hex_byte = String::new();
            while let Some(c) = self.peek() {
                if c.is_ascii_hexdigit() {
                    hex_byte.push(c);
                    self.next_char();
                } else if c.is_whitespace() || c == ',' {
                    break;
                } else {
                    break;
                }
            }
            if !hex_byte.is_empty() {
                if hex_byte.len() == 1 {
                    hex_byte.insert(0, '0');
                }
                if let Ok(b) = u8::from_str_radix(&hex_byte, 16) {
                    bytes.push(b);
                } else {
                    return Err(KV3Error::InvalidHex(hex_byte));
                }
            }
            self.skip_whitespace();
            if self.peek() == Some(',') {
                self.next_char();
            }
        }
        self.next_char(); // skip ]
        Ok(KV3Value::Bytes(bytes))
    }
    
    fn parse_true(&mut self) -> Result<KV3Value, KV3Error> {
        for expected in "true".chars() {
            if self.next_char() != Some(expected) {
                return Err(KV3Error::UnexpectedToken("true".to_string()));
            }
        }
        Ok(KV3Value::Bool(true))
    }
    
    fn parse_false(&mut self) -> Result<KV3Value, KV3Error> {
        for expected in "false".chars() {
            if self.next_char() != Some(expected) {
                return Err(KV3Error::UnexpectedToken("false".to_string()));
            }
        }
        Ok(KV3Value::Bool(false))
    }
    
    fn parse_null(&mut self) -> Result<KV3Value, KV3Error> {
        for expected in "null".chars() {
            if self.next_char() != Some(expected) {
                return Err(KV3Error::UnexpectedToken("null".to_string()));
            }
        }
        Ok(KV3Value::Null)
    }
}

/// Convert byte array to f32 vector (little-endian)
pub fn bytes_to_f32_vec(bytes: &[u8]) -> Result<Vec<f32>, KV3Error> {
    if bytes.len() % 4 != 0 {
        return Err(KV3Error::InvalidNumber(format!("Bytes length {} is not multiple of 4", bytes.len())));
    }
    let mut result = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        result.push(f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(result)
}

/// Convert byte array to i32 vector (little-endian)
pub fn bytes_to_i32_vec(bytes: &[u8]) -> Result<Vec<i32>, KV3Error> {
    if bytes.len() % 4 != 0 {
        return Err(KV3Error::InvalidNumber(format!("Bytes length {} is not multiple of 4", bytes.len())));
    }
    let mut result = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        result.push(i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(result)
}

/// Parse KV3 from file content
pub fn parse_kv3(content: &str) -> Result<KV3Value, KV3Error> {
    let mut parser = KV3Parser::new(content);
    parser.parse()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_null() {
        let result = parse_kv3("null");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), KV3Value::Null);
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_kv3("true").unwrap(), KV3Value::Bool(true));
        assert_eq!(parse_kv3("false").unwrap(), KV3Value::Bool(false));
    }

    #[test]
    fn test_parse_int() {
        let result = parse_kv3("42").unwrap();
        assert_eq!(result, KV3Value::Int(42));
        
        let result = parse_kv3("-10").unwrap();
        assert_eq!(result, KV3Value::Int(-10));
    }

    #[test]
    fn test_parse_float() {
        let result = parse_kv3("3.14").unwrap();
        assert!(matches!(result, KV3Value::Float(f) if (f - 3.14).abs() < 0.001));
    }

    #[test]
    fn test_parse_string() {
        let result = parse_kv3("\"hello world\"").unwrap();
        assert_eq!(result, KV3Value::String("hello world".to_string()));
    }

    #[test]
    fn test_parse_object() {
        let result = parse_kv3(r#"{"name": "test", "value": 42}"#).unwrap();
        if let KV3Value::Object(map) = result {
            assert_eq!(map.get("name").unwrap().as_str().unwrap(), "test");
            assert_eq!(map.get("value").unwrap().as_i64().unwrap(), 42);
        } else {
            panic!("Expected Object");
        }
    }

    #[test]
    fn test_parse_array() {
        let result = parse_kv3("[1, 2, 3]").unwrap();
        if let KV3Value::Array(arr) = result {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0].as_i64().unwrap(), 1);
            assert_eq!(arr[1].as_i64().unwrap(), 2);
            assert_eq!(arr[2].as_i64().unwrap(), 3);
        } else {
            panic!("Expected Array");
        }
    }

    #[test]
    fn test_parse_bytes() {
        let result = parse_kv3("#[0A 1B 2C 3D]").unwrap();
        if let KV3Value::Bytes(bytes) = result {
            assert_eq!(bytes, vec![0x0A, 0x1B, 0x2C, 0x3D]);
        } else {
            panic!("Expected Bytes");
        }
    }

    #[test]
    fn test_bytes_to_f32() {
        // 0.0f32 = [0x00, 0x00, 0x00, 0x00], 1.0f32 = [0x00, 0x00, 0x80, 0x3F]
        let bytes = vec![0, 0, 0, 0, 0, 0, 128, 63];
        let result = bytes_to_f32_vec(&bytes).unwrap();
        assert_eq!(result.len(), 2);
        assert!((result[0] - 0.0).abs() < 0.001);
        assert!((result[1] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_bytes_to_i32() {
        let bytes = vec![1, 0, 0, 0, 2, 0, 0, 0]; // 1, 2
        let result = bytes_to_i32_vec(&bytes).unwrap();
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn test_nested_object() {
        let input = r#"{
            "m_parts": [
                {
                    "m_rnShape": {
                        "m_meshes": [
                            {
                                "m_Mesh": {
                                    "m_Vertices": #[00 00 00 00 00 00 80 3F],
                                    "m_Triangles": #[00 00 00 00 01 00 00 00]
                                }
                            }
                        ]
                    }
                }
            ]
        }"#;
        let result = parse_kv3(input);
        assert!(result.is_ok(), "Failed to parse nested object: {:?}", result.err());
    }
}
