use crate::IdType;

/// Database configuration used when creating collections.
///
/// - `id_type` controls how document IDs are generated or interpreted.
/// - `id_key` is the JSON key used to store the document id inside each item.
#[derive(Debug, Clone, PartialEq)]
pub struct DbConfig {
    /// Strategy for generated/interpreted ids
    pub id_type: IdType,
    /// Field name inside documents that contains the id
    pub id_key: String,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self { id_type: Default::default(), id_key: "id".to_string() }
    }
}

impl DbConfig {
    /// Create default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration with explicit `id_type` and `id_key`.
    pub fn from(id_type: IdType, id_key: &str) -> Self {
        Self {
            id_type,
            id_key: id_key.to_string(),
        }
    }

    /// Convenience: create a config for integer ids using `id_key`.
    pub fn int(id_key: &str) -> Self {
        Self {
            id_type: IdType::Int,
            id_key: id_key.to_string(),
        }
    }

    /// Convenience: create a config for UUID ids using `id_key`.
    pub fn uuid(id_key: &str) -> Self {
        Self {
            id_type: IdType::Uuid,
            id_key: id_key.to_string(),
        }
    }

    /// Convenience: create a config with no id generation; caller must
    /// provide ids in documents under `id_key`.
    pub fn none(id_key: &str) -> Self {
        Self {
            id_type: IdType::None,
            id_key: id_key.to_string(),
        }
    }
}

