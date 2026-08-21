use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Kv3Document {
    pub format_guid: [u8; 16],
    pub root: Kv3Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Kv3Field {
    pub key: String,
    pub value: Kv3Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Kv3Value {
    Null,
    Bool(bool),
    Int32(i32),
    Int64(i64),
    UInt32(u32),
    UInt64(u64),
    Float(f32),
    Double(f64),
    String(String),
    Array(Vec<Kv3Value>),
    Object(Vec<Kv3Field>),
    BinaryBlob(Vec<u8>),
}

impl Kv3Value {
    pub fn object_field(&self, key: &str) -> Option<&Kv3Value> {
        match self {
            Self::Object(fields) => fields
                .iter()
                .find(|field| field.key == key)
                .map(|field| &field.value),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Kv3Value]> {
        match self {
            Self::Array(values) => Some(values),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}
