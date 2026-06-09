use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub struct Row(pub Map<String, Value>);

impl Row {
    pub fn get(&self, key: &str) -> Option<&Value> { self.0.get(key) }
    pub fn into_value(self) -> Value { Value::Object(self.0) }
}

#[cfg(test)]
mod tests {
    use super::Row;
    use serde_json::{json, Map, Value};

    #[test]
    fn get_returns_existing_value_and_none_for_missing_key() {
        let mut map = Map::new();
        map.insert("name".to_string(), json!("Ada"));
        let row = Row(map);

        assert_eq!(row.get("name"), Some(&json!("Ada")));
        assert_eq!(row.get("missing"), None);
    }

    #[test]
    fn into_value_returns_object_with_original_fields() {
        let mut map = Map::new();
        map.insert("id".to_string(), json!(1));
        let row = Row(map);

        assert_eq!(row.into_value(), Value::Object(Map::from_iter([("id".to_string(), json!(1))])));
    }
}
