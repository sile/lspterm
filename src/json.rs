use std::collections::BTreeMap;

use nojson::JsonValueKind;

pub trait JsonRpcRequest {
    type Response: JsonRpcResponse;

    fn method(&self) -> &str;
    fn params(&self, f: &mut nojson::JsonObjectFormatter<'_, '_, '_>) -> std::fmt::Result;
}

pub trait JsonRpcResponse: Sized {
    fn from_result_value(
        value: nojson::RawJsonValue<'_, '_>,
    ) -> Result<Self, nojson::JsonParseError>;
}

pub fn json_object<F>(members: F) -> impl nojson::DisplayJson
where
    F: Fn(&mut nojson::JsonObjectFormatter<'_, '_, '_>) -> std::fmt::Result,
{
    nojson::json(move |f| f.object(|f| members(f)))
}

#[derive(Debug)]
pub enum JsonValue {
    Null,
    Boolean(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl<'text, 'raw> TryFrom<nojson::RawJsonValue<'text, 'raw>> for JsonValue {
    type Error = nojson::JsonParseError;

    fn try_from(value: nojson::RawJsonValue<'text, 'raw>) -> Result<Self, Self::Error> {
        match value.kind() {
            JsonValueKind::Null => Ok(JsonValue::Null),
            JsonValueKind::Boolean => Ok(JsonValue::Boolean(value.try_into()?)),
            JsonValueKind::Integer => Ok(JsonValue::Integer(value.try_into()?)),
            JsonValueKind::Float => Ok(JsonValue::Float(value.try_into()?)),
            JsonValueKind::String => Ok(JsonValue::String(value.try_into()?)),
            JsonValueKind::Array => Ok(JsonValue::Array(value.try_into()?)),
            JsonValueKind::Object => Ok(JsonValue::Object(value.try_into()?)),
        }
    }
}

impl nojson::DisplayJson for JsonValue {
    fn fmt(&self, f: &mut nojson::JsonFormatter<'_, '_>) -> std::fmt::Result {
        match self {
            JsonValue::Null => f.value(()),
            JsonValue::Boolean(v) => f.value(v),
            JsonValue::Integer(v) => f.value(v),
            JsonValue::Float(v) => f.value(v),
            JsonValue::String(v) => f.value(v),
            JsonValue::Array(v) => f.value(v),
            JsonValue::Object(v) => f.value(v),
        }
    }
}
