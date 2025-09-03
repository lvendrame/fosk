use serde_json::Value;

use crate::JsonPrimitive;

/// Metadata for a single field inferred from JSON documents.
///
/// - `ty`: the coarse-grained primitive type of the field.
/// - `nullable`: whether the field was observed as `null` or missing in any
///   analyzed document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    /// Primitive type of the field
    pub ty: JsonPrimitive,
    /// Whether the field may be null / missing
    pub nullable: bool,
}

impl FieldInfo {
    /// Infer a `FieldInfo` from a single `serde_json::Value`.
    ///
    /// If the value is `null` the returned `FieldInfo` will have `nullable = true`.
    pub fn infer_field_info(value: &Value) -> FieldInfo {
        let ty = JsonPrimitive::of_value(value);
        FieldInfo {
            ty: if ty == JsonPrimitive::Null { JsonPrimitive::Null } else { ty },
            nullable: ty == JsonPrimitive::Null,
        }
    }

    /// Merge this `FieldInfo` with another observation returning the promoted
    /// result. Promotion handles numeric widening (Int -> Float) and
    /// preserves nullability if either side is nullable.
    pub fn merge_field_info(&self, new: &FieldInfo) -> FieldInfo {
        let promoted = JsonPrimitive::promote(self.ty, new.ty);
        FieldInfo {
            ty: if promoted == JsonPrimitive::Null { self.ty } else { promoted },
            nullable: self.nullable || new.nullable || new.ty == JsonPrimitive::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_promotion_int_to_float() {
        let a = FieldInfo { ty: JsonPrimitive::Int,   nullable: false };
        let b = FieldInfo { ty: JsonPrimitive::Float, nullable: false };
        let c = a.merge_field_info(&b);
        assert_eq!(c.ty, JsonPrimitive::Float);
        assert!(!c.nullable);
    }
}
