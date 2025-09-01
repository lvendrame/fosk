use crate::IdType;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub id_type: IdType,
    pub id_key: String,
}

impl Default for Config {
    fn default() -> Self {
        Self { id_type: Default::default(), id_key: "id".to_string() }
    }
}

impl Config {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from(id_type: IdType, id_key: &str) -> Self {
        Self {
            id_type,
            id_key: id_key.to_string(),
        }
    }

    pub fn int(id_key: &str) -> Self {
        Self {
            id_type: IdType::Int,
            id_key: id_key.to_string(),
        }
    }
    pub fn uuid(id_key: &str) -> Self {
        Self {
            id_type: IdType::Uuid,
            id_key: id_key.to_string(),
        }
    }
    pub fn none(id_key: &str) -> Self {
        Self {
            id_type: IdType::None,
            id_key: id_key.to_string(),
        }
    }
}

