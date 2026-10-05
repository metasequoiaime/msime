//! Field access for a request object. A missing field or a field of the wrong type is an invalid request, as a `nlohmann::json` type error was in the C++ bridge.

use serde_json::{Map, Value};

use super::BackendError;

/// The largest request the process reads from stdin.
pub const MAXIMUM_REQUEST_BYTES: usize = 64 * 1024;
/// The longest `text` any operation accepts, in bytes.
pub(crate) const MAXIMUM_TEXT_BYTES: usize = 8192;
pub(crate) const DEFAULT_LIMIT: i64 = 20;
pub(crate) const MAXIMUM_LIMIT: i64 = 200;
/// Batch operations take between one and this many entries.
pub(crate) const MAXIMUM_BATCH: usize = 50;

pub(crate) struct Request<'a> {
    fields: &'a Map<String, Value>,
    operation: &'a str,
    text: &'a str,
    limit: usize,
}

impl<'a> Request<'a> {
    /// Every request names an operation; `text` and `limit` are checked up front for all of them.
    pub(crate) fn new(raw: &'a Value) -> Result<Self, BackendError> {
        let fields = raw.as_object().ok_or(BackendError::InvalidRequest)?;
        let operation = fields
            .get("operation")
            .and_then(Value::as_str)
            .ok_or(BackendError::InvalidRequest)?;
        let text = match fields.get("text") {
            None => "",
            Some(value) => value.as_str().ok_or(BackendError::InvalidRequest)?,
        };
        let limit = match fields.get("limit") {
            None => DEFAULT_LIMIT,
            Some(value) => value.as_i64().ok_or(BackendError::InvalidRequest)?,
        };
        if !(1..=MAXIMUM_LIMIT).contains(&limit) || text.len() > MAXIMUM_TEXT_BYTES {
            return Err(BackendError::InvalidRequest);
        }
        Ok(Self {
            fields,
            operation,
            text,
            limit: limit as usize,
        })
    }

    pub(crate) fn operation(&self) -> &'a str {
        self.operation
    }

    pub(crate) fn text(&self) -> &'a str {
        self.text
    }

    pub(crate) fn limit(&self) -> usize {
        self.limit
    }

    pub(crate) fn field(&self, name: &str) -> Option<&'a Value> {
        self.fields.get(name)
    }

    pub(crate) fn required(&self, name: &str) -> Result<&'a Value, BackendError> {
        self.field(name).ok_or(BackendError::InvalidRequest)
    }

    pub(crate) fn string(&self, name: &str) -> Result<&'a str, BackendError> {
        string(self.required(name)?)
    }

    /// The string under `name`, or `default` when the field is absent.
    pub(crate) fn string_or(&self, name: &str, default: &'a str) -> Result<&'a str, BackendError> {
        self.field(name).map_or(Ok(default), string)
    }

    /// The array under `name`, holding between one and `MAXIMUM_BATCH` items.
    pub(crate) fn batch(&self, name: &str) -> Result<&'a [Value], BackendError> {
        let items = self
            .required(name)?
            .as_array()
            .ok_or(BackendError::InvalidRequest)?;
        if items.is_empty() || items.len() > MAXIMUM_BATCH {
            return Err(BackendError::InvalidRequest);
        }
        Ok(items)
    }
}

pub(crate) fn string(value: &Value) -> Result<&str, BackendError> {
    value.as_str().ok_or(BackendError::InvalidRequest)
}

/// The string field `name` of an object inside a request, such as one batch entry.
pub(crate) fn member<'v>(object: &'v Value, name: &str) -> Result<&'v str, BackendError> {
    object
        .get(name)
        .ok_or(BackendError::InvalidRequest)
        .and_then(string)
}
