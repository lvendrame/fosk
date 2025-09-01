use serde_json::Value;

pub struct RowCell {
    pub collection: String,
    pub value: Value,
}
