use serde_json::Value;

use crate::JsonPrimitive;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldInfo {
    pub ty: JsonPrimitive,
    pub nullable: bool,
}

impl FieldInfo {
    pub fn infer_field_info(value: &Value) -> FieldInfo {
        let ty = JsonPrimitive::of_value(value);
        FieldInfo {
            ty: if ty == JsonPrimitive::Null { JsonPrimitive::Null } else { ty },
            nullable: ty == JsonPrimitive::Null,
        }
    }

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
