use std::collections::HashMap;

use indexmap::IndexMap;
use serde_json::{Map, Value};

use crate::{database::{FieldInfo, ReferenceColumn}, Db};

/// A small dictionary describing the inferred schema for a collection.
///
/// The `fields` map stores `FieldInfo` entries keyed by field name. This type
/// is used by the in-memory collection to track types and nullability across
/// multiple documents.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaDict {
    /// Map of field name -> field metadata
    pub fields: IndexMap<String, FieldInfo>,
}

impl SchemaDict {
    /// Return the `FieldInfo` for a field name if present.
    pub fn get(&self, name: &str) -> Option<&FieldInfo> {
        self.fields.get(name)
    }

    /// Build a `SchemaDict` from a single JSON object by inferring each field's
    /// primitive type and nullability.
    pub fn infer_schema_from_object(obj: &Map<String, Value>) -> SchemaDict {
        let mut fields = IndexMap::new();
        for (k, v) in obj {
            fields.insert(k.clone(), FieldInfo::infer_field_info(v));
        }

        SchemaDict { fields }
    }

    /// Merge a new JSON object into the schema, promoting types where
    /// necessary and marking fields as nullable if they are absent or null in
    /// the new object.
    pub fn merge_schema(&mut self, obj: &Map<String, Value>) {
        // First, mark missing keys as nullable (they weren't present on this row)
        // (Optional) If you want "missing means nullable", uncomment:
        for (key, field_info) in self.fields.iter_mut() {
            if !obj.contains_key(key) {
                field_info.nullable = true;
            }
        }

        // Merge present keys
        for (key, value) in obj {
            let new_info = FieldInfo::infer_field_info(value);
            match self.fields.get_mut(key) {
                Some(old) => {
                    *old = old.merge_field_info(&new_info);
                }
                None => {
                    self.fields.insert(key.clone(), new_info);
                }
            }
        }
    }
}


#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaWithRefs {
    pub name: String,
    /// Map of field name -> field metadata
    pub fields: IndexMap<String, FieldInfo>,
    pub outbound_refs: HashMap<String, ReferenceColumn>,
    pub inbound_refs: HashMap<String, ReferenceColumn>,
}

impl SchemaWithRefs {
    pub fn new(collection_name: &str, schema_dict: &SchemaDict, db: &Db) -> Self {
        let fields = schema_dict.fields.clone();
        let mut outbound_refs = HashMap::new();
        let mut inbound_refs = HashMap::new();

        if let Some(refs) = db.get_collection_refs(collection_name) {
            for s_ref in refs.into_values() {
                if s_ref.is_referrer {
                    outbound_refs.insert(s_ref.ref_column.clone(), s_ref.clone());
                } else {
                    inbound_refs.insert(s_ref.column.clone(), s_ref.clone());
                }
            }
        }

        Self { name: collection_name.to_string(), fields, outbound_refs, inbound_refs }
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
