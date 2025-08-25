#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JoinType {
    Inner,
    Left,
    Right,
}

pub struct Join {
    pub join_type: JoinType,
    pub collection: String,
    pub alias: Option<String>,
}
