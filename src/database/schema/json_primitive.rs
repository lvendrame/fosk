use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonPrimitive {
    Null,
    Bool,
    Int,
    Float,
    String,
    Object,
    Array,
}

impl JsonPrimitive {
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
