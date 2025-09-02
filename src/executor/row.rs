use serde_json::{Map, Value};

#[derive(Debug, Clone)]
pub struct Row(pub Map<String, Value>);

impl Row {
    pub fn get(&self, key: &str) -> Option<&Value> { self.0.get(key) }
    pub fn into_value(self) -> Value { Value::Object(self.0) }
}
