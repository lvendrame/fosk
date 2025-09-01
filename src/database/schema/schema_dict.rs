use std::collections::BTreeMap;

use serde_json::{Map, Value};

use crate::database::FieldInfo;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaDict {
    pub fields: BTreeMap<String, FieldInfo>,
}

impl SchemaDict {
    pub fn get(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.get(name)
    }

    pub fn infer_schema_from_object(obj: &Map<String, Value>) -> SchemaDict {
        let mut fields = BTreeMap::new();
        for (k, v) in obj {
            fields.insert(k.clone(), FieldInfo::infer_field_info(v));
        }
        SchemaDict { fields }
    }

    pub fn merge_schema(&mut self, obj: &Map<String, Value>) {
        // First, mark missing keys as nullable (they weren't present on this row)
        // (Optional) If you want "missing means nullable", uncomment:
        for (k, fi) in self.fields.iter_mut() {
            if !obj.contains_key(k) {
                fi.nullable = true;
            }
        }

        // Merge present keys
        for (k, v) in obj {
            let new_info = FieldInfo::infer_field_info(v);
            match self.fields.get_mut(k) {
                Some(old) => {
                    *old = old.merge_field_info(&new_info);
                }
                None => {
                    self.fields.insert(k.clone(), new_info);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::JsonPrimitive;

    use super::*;
    use serde_json::json;

    #[test]
    fn test_nullability_on_missing_and_null_values() {
        let mut s = SchemaDict::default();

        // first row: present "age" as Int
        let r1 = json!({"id": 1, "name": "Ana", "age": 30}).as_object().unwrap().clone();
        s.merge_schema(&r1);
        assert_eq!(s.get("age").unwrap().ty, JsonPrimitive::Int);
        assert!(!s.get("age").unwrap().nullable);

        // second row: age is missing -> nullable should flip only for "age"
        let r2 = json!({"id": 2, "name": "Bob"}).as_object().unwrap().clone();
        s.merge_schema(&r2);
        assert!(s.get("age").unwrap().nullable);
        assert!(!s.get("name").unwrap().nullable);

        // third row: age is explicitly null -> nullable remains true
        let r3 = json!({"id": 3, "name": "Cara", "age": null}).as_object().unwrap().clone();
        s.merge_schema(&r3);
        assert!(s.get("age").unwrap().nullable);
    }

    #[test]
    fn test_new_field_added() {
        let mut s = SchemaDict::default();
        let r1 = json!({"id": 1, "name": "Ana"}).as_object().unwrap().clone();
        s.merge_schema(&r1);
        assert!(s.get("email").is_none());

        let r2 = json!({"id": 2, "name": "Bob", "email": "b@x.com"}).as_object().unwrap().clone();
        s.merge_schema(&r2);
        let email = s.get("email").unwrap();
        assert_eq!(email.ty, JsonPrimitive::String);
        assert!(!email.nullable);
    }

    #[test]
    fn test_numeric_promotion_over_time() {
        let mut s = SchemaDict::default();
        let r1 = json!({"price": 10}).as_object().unwrap().clone();      // Int
        s.merge_schema(&r1);
        assert_eq!(s.get("price").unwrap().ty, JsonPrimitive::Int);

        let r2 = json!({"price": 10.5}).as_object().unwrap().clone();    // Float
        s.merge_schema(&r2);
        assert_eq!(s.get("price").unwrap().ty, JsonPrimitive::Float);    // promoted
    }
}
