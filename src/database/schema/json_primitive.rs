use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Lightweight classification of JSON value shapes used by schema inference.
///
/// This enum represents the coarse-grained primitive kind of a JSON value
/// encountered while inspecting documents: Null, Bool, Int, Float, String,
/// Object (map) or Array.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JsonPrimitive {
    /// JSON null
    Null,
    /// JSON boolean
    Bool,
    /// Integer number
    Int,
    /// Floating-point number
    Float,
    /// String
    String,
    /// JSON object (map)
    Object,
    /// JSON array
    Array,
}

impl JsonPrimitive {
    /// Classify a serde_json `Value` into a `JsonPrimitive`.
    ///
    /// Returns a single variant indicating the basic shape of the value.
    pub fn of_value(v: &Value) -> JsonPrimitive {
        match v {
            Value::Null => JsonPrimitive::Null,
            Value::Bool(_) => JsonPrimitive::Bool,
            Value::Number(n) => {
                if n.is_i64() || n.is_u64() {
                    JsonPrimitive::Int
                } else {
                    JsonPrimitive::Float
                }
            }
            Value::String(_) => JsonPrimitive::String,
            Value::Array(_) => JsonPrimitive::Array,
            Value::Object(_) => JsonPrimitive::Object,
        }
    }

    /// Promote two primitive types to a common representative for schema merging.
    ///
    /// Numeric types promote `Int` + `Float` -> `Float`. For different
    /// non-numeric types the left-hand value is preserved except when it is
    /// `Null`, in which case the right-hand type is returned.
    pub fn promote(a: JsonPrimitive, b: JsonPrimitive) -> JsonPrimitive {
        use JsonPrimitive::*;
        if a == b { return a; }
        match (a, b) {
            (Int, Float) | (Float, Int) => Float,
            // Different non-numeric types stay as the left (first seen) type.
            // You can change this to a Mixed variant if you prefer.
            (x, y) => {
                // If one is Null, keep the other (nullability is tracked separately)
                if x == Null {
                    y
                } else {
                    x
                }
            }
        }
    }
}
