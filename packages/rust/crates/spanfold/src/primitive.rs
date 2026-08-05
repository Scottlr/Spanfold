use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A finite floating-point value.
///
/// Construction rejects NaN and positive or negative infinity. The inner
/// value is private so safe callers cannot bypass that invariant.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct FiniteFloat(f64);

impl FiniteFloat {
    /// Constructs a finite floating-point value.
    pub fn try_new(value: f64) -> Result<Self, PrimitiveValueError> {
        if value.is_finite() {
            Ok(Self(value))
        } else {
            Err(PrimitiveValueError::NonFiniteFloat)
        }
    }

    /// Returns the underlying finite floating-point value.
    #[must_use]
    pub const fn as_f64(self) -> f64 {
        self.0
    }
}

impl TryFrom<f64> for FiniteFloat {
    type Error = PrimitiveValueError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        Self::try_new(value)
    }
}

impl<'de> Deserialize<'de> for FiniteFloat {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Self::try_new(f64::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

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
    Float(FiniteFloat),
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
                    && float.as_f64().fract() == 0.0
                    && float.as_f64() == *integer as f64
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
        FiniteFloat::try_new(value).map(Self::Float)
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
            Float(FiniteFloat),
            String(String),
            Bool(bool),
            Null,
        }

        match RawPrimitive::deserialize(deserializer)? {
            RawPrimitive::Integer(value) => Ok(Self::Integer(value)),
            RawPrimitive::Float(value) => Ok(Self::Float(value)),
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
        assert_eq!(
            PrimitiveValue::Integer(1),
            PrimitiveValue::try_float(1.0).expect("finite float")
        );
        assert_ne!(
            PrimitiveValue::Integer(i64::MAX),
            PrimitiveValue::try_float(i64::MAX as f64).expect("finite float")
        );
        assert_eq!(
            PrimitiveValue::try_float(f64::INFINITY),
            Err(PrimitiveValueError::NonFiniteFloat)
        );
    }

    #[test]
    fn finite_float_serializes_as_a_json_number_and_round_trips() {
        let value = PrimitiveValue::try_float(1.5).expect("finite float");
        let json = serde_json::to_string(&value).expect("serialize finite float");

        assert_eq!(json, "1.5");
        assert_eq!(
            serde_json::from_str::<PrimitiveValue>(&json).expect("deserialize finite float"),
            value
        );
    }

    #[test]
    fn finite_float_has_fallible_construction_and_read_only_access() {
        let value = FiniteFloat::try_new(2.5).expect("finite float");

        assert_eq!(value.as_f64(), 2.5);
        assert_eq!(
            FiniteFloat::try_from(f64::NAN),
            Err(PrimitiveValueError::NonFiniteFloat)
        );
    }

    #[test]
    fn finite_float_deserialization_rejects_non_finite_values() {
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let deserializer =
                serde::de::value::F64Deserializer::<serde::de::value::Error>::new(value);

            assert!(FiniteFloat::deserialize(deserializer).is_err());
        }
    }
}
