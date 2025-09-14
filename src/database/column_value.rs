use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnValue {
    pub column: String,
    pub value: Value,
}

impl ColumnValue {
    pub fn new(column: String, value: Value) -> Self {
        Self { column, value }
    }
}
