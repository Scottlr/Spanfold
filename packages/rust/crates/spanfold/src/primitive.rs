use serde::{Deserialize, Serialize};
use thiserror::Error;

/// JSON-compatible primitive value used by tags, segments, annotations, and
/// fixture/import boundaries.
#[derive(Clone, Debug, Serialize)]
#[serde(untagged)]
pub enum PrimitiveValue {
    /// String value.
    String(String),
    /// Signed integer value.
    Integer(i64),
    /// Floating point value.
    Float(f64),
    /// Boolean value.
    Bool(bool),
    /// Null value.
    Null,
}

impl PartialEq for PrimitiveValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(left), Self::Integer(right)) => left == right,
            (Self::Float(left), Self::Float(right)) => left == right,
            (Self::Integer(integer), Self::Float(float))
            | (Self::Float(float), Self::Integer(integer)) => {
                integer.unsigned_abs() <= (1_u64 << 53)
                    && float.is_finite()
                    && float.fract() == 0.0
                    && *float == *integer as f64
            }
            (Self::String(left), Self::String(right)) => left == right,
            (Self::Bool(left), Self::Bool(right)) => left == right,
            (Self::Null, Self::Null) => true,
            _ => false,
        }
    }
}

/// Error returned when a primitive value cannot be represented safely.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PrimitiveValueError {
    /// Floating-point values must be finite.
    #[error("primitive float must be finite")]
    NonFiniteFloat,
}

impl PrimitiveValue {
    /// Constructs a finite floating-point primitive.
    pub fn try_float(value: f64) -> Result<Self, PrimitiveValueError> {
        if value.is_finite() {
            Ok(Self::Float(value))
        } else {
            Err(PrimitiveValueError::NonFiniteFloat)
        }
    }
}

impl<'de> Deserialize<'de> for PrimitiveValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum RawPrimitive {
            Integer(i64),
            Float(f64),
            String(String),
            Bool(bool),
            Null,
        }

        match RawPrimitive::deserialize(deserializer)? {
            RawPrimitive::Integer(value) => Ok(Self::Integer(value)),
            RawPrimitive::Float(value) => Self::try_float(value).map_err(serde::de::Error::custom),
            RawPrimitive::String(value) => Ok(Self::String(value)),
            RawPrimitive::Bool(value) => Ok(Self::Bool(value)),
            RawPrimitive::Null => Ok(Self::Null),
        }
    }
}

impl From<&str> for PrimitiveValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_owned())
    }
}

impl From<String> for PrimitiveValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<i64> for PrimitiveValue {
    fn from(value: i64) -> Self {
        Self::Integer(value)
    }
}

impl From<i32> for PrimitiveValue {
    fn from(value: i32) -> Self {
        Self::Integer(i64::from(value))
    }
}

impl From<bool> for PrimitiveValue {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_equality_is_explicit_and_non_finite_values_are_rejected() {
        assert_eq!(PrimitiveValue::Integer(1), PrimitiveValue::Float(1.0));
        assert_ne!(
            PrimitiveValue::Integer(i64::MAX),
            PrimitiveValue::Float(i64::MAX as f64)
        );
        assert_eq!(
            PrimitiveValue::try_float(f64::INFINITY),
            Err(PrimitiveValueError::NonFiniteFloat)
        );
    }
}
