use serde::{Deserialize, Serialize};

use crate::IdType;

/// Database configuration used when creating collections.
///
/// - `id_type` controls how document IDs are generated or interpreted.
/// - `id_key` is the JSON key used to store the document id inside each item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DbConfig {
    /// Strategy for generated/interpreted ids
    pub id_type: IdType,
    /// Field name inside documents that contains the id
    pub id_key: String,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            id_type: Default::default(),
            id_key: "id".to_string(),
        }
    }
}

impl DbConfig {
    /// Create the default configuration.
    ///
    /// Equivalent to [`DbConfig::default`]: UUID ids stored under `"id"`.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbConfig, IdType};
    ///
    /// let config = DbConfig::new();
    ///
    /// assert_eq!(config.id_type, IdType::Uuid);
    /// assert_eq!(config.id_key, "id");
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration with explicit [`IdType`] and id key.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbConfig, IdType};
    ///
    /// let config = DbConfig::from(IdType::Int, "row_id");
    ///
    /// assert_eq!(config.id_type, IdType::Int);
    /// assert_eq!(config.id_key, "row_id");
    /// ```
    pub fn from(id_type: IdType, id_key: &str) -> Self {
        Self {
            id_type,
            id_key: id_key.to_string(),
        }
    }

    /// Create a configuration that generates sequential integer ids.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    /// let inserted = people.add(json!({ "name": "Ada" })).unwrap();
    ///
    /// assert_eq!(inserted["id"], 1);
    /// ```
    pub fn int(id_key: &str) -> Self {
        Self {
            id_type: IdType::Int,
            id_key: id_key.to_string(),
        }
    }

    /// Create a configuration that generates UUID string ids.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// let people = DbCollection::new_coll("people", DbConfig::uuid("id"));
    /// let inserted = people.add(json!({ "name": "Ada" })).unwrap();
    ///
    /// assert!(inserted["id"].as_str().unwrap().contains('-'));
    /// ```
    pub fn uuid(id_key: &str) -> Self {
        Self {
            id_type: IdType::Uuid,
            id_key: id_key.to_string(),
        }
    }

    /// Create a configuration with no automatic id generation.
    ///
    /// Callers must provide ids in documents under `id_key`; documents without
    /// that key are rejected.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    ///
    /// assert!(people.add(json!({ "id": "ada", "name": "Ada" })).is_some());
    /// assert!(people.add(json!({ "name": "Grace" })).is_none());
    /// ```
    pub fn none(id_key: &str) -> Self {
        Self {
            id_type: IdType::None,
            id_key: id_key.to_string(),
        }
    }
}
