//! Typed JSON wrapper for Diesel TEXT columns.
//!
//! This module provides a generic wrapper type that automatically handles
//! serialization/deserialization of typed data stored as JSON strings in
//! TEXT columns.

use diesel::deserialize::{FromSql, FromSqlRow};
use diesel::expression::AsExpression;
use diesel::pg::{Pg, PgValue};
use diesel::serialize::{IsNull, Output, ToSql};
use diesel::sql_types::Text;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::fmt;
use std::io::Write;
use std::ops::{Deref, DerefMut};

/// A wrapper that stores typed data as JSON in TEXT columns.
///
/// This wrapper automatically serializes to/from JSON when reading/writing
/// to the database, providing type safety at the database boundary.
///
/// # Example
///
/// ```ignore
/// use shared_types::{JsonWrapper, RuleConditions};
///
/// // In a database model:
/// pub struct AgentRule {
///     pub conditions: JsonWrapper<RuleConditions>,
/// }
///
/// // The wrapper transparently serializes/deserializes
/// let conditions = JsonWrapper::new(RuleConditions { ... });
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, AsExpression, FromSqlRow)]
#[serde(transparent)]
#[diesel(sql_type = Text)]
pub struct JsonWrapper<T>(pub T);

impl<T> JsonWrapper<T> {
    /// Create a new wrapper around a value.
    pub fn new(value: T) -> Self {
        JsonWrapper(value)
    }

    /// Unwrap and return the inner value.
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T: Default> Default for JsonWrapper<T> {
    fn default() -> Self {
        JsonWrapper(T::default())
    }
}

impl<T> Deref for JsonWrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for JsonWrapper<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> From<T> for JsonWrapper<T> {
    fn from(value: T) -> Self {
        JsonWrapper(value)
    }
}

impl<T: fmt::Display> fmt::Display for JsonWrapper<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// Diesel integration for JsonWrapper

impl<T> FromSql<Text, Pg> for JsonWrapper<T>
where
    T: DeserializeOwned,
{
    fn from_sql(bytes: PgValue<'_>) -> diesel::deserialize::Result<Self> {
        let s = <String as FromSql<Text, Pg>>::from_sql(bytes)?;
        let value: T = serde_json::from_str(&s)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
        Ok(JsonWrapper(value))
    }
}

impl<T> ToSql<Text, Pg> for JsonWrapper<T>
where
    T: Serialize + fmt::Debug,
{
    fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Pg>) -> diesel::serialize::Result {
        let s = serde_json::to_string(&self.0)?;
        out.write_all(s.as_bytes())?;
        Ok(IsNull::No)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_wrapper_creation() {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let wrapped = JsonWrapper::new(data.clone());
        assert_eq!(wrapped.0, data);
    }

    #[test]
    fn test_wrapper_deref() {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let wrapped = JsonWrapper::new(data);
        assert_eq!(wrapped.name, "test");
        assert_eq!(wrapped.value, 42);
    }

    #[test]
    fn test_wrapper_into_inner() {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let wrapped = JsonWrapper::new(data.clone());
        let unwrapped = wrapped.into_inner();
        assert_eq!(unwrapped, data);
    }

    #[test]
    fn test_wrapper_serde() {
        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };
        let wrapped = JsonWrapper::new(data);

        // Serialize
        let json = serde_json::to_string(&wrapped).unwrap();
        assert_eq!(json, r#"{"name":"test","value":42}"#);

        // Deserialize
        let parsed: JsonWrapper<TestData> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.0.name, "test");
        assert_eq!(parsed.0.value, 42);
    }
}
