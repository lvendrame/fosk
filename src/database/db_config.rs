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
    /// use fosk::{AddError, DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    /// let inserted = people
    ///     .add(json!({ "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// assert_eq!(inserted["id"], 1);
    /// # Ok(())
    /// # }
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
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::uuid("id"));
    /// let inserted = people
    ///     .add(json!({ "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let id = inserted["id"]
    ///     .as_str()
    ///     .ok_or_else(|| "generated id was not a string".to_string())?;
    /// assert!(id.contains('-'));
    /// # Ok(())
    /// # }
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
    /// use fosk::{AddError, DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    ///
    /// let inserted = people
    ///     .add(json!({ "id": "ada", "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    /// assert_eq!(inserted["id"], "ada");
    ///
    /// let missing_id = people.add(json!({ "name": "Grace" }));
    /// assert_eq!(
    ///     missing_id,
    ///     Err(AddError::MissingId {
    ///         id_key: "id".to_string()
    ///     })
    /// );
    /// # Ok(())
    /// # }
    /// ```
    pub fn none(id_key: &str) -> Self {
        Self {
            id_type: IdType::None,
            id_key: id_key.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DbConfig;
    use crate::IdType;

    #[test]
    fn constructors_set_id_type_and_key() {
        assert_eq!(
            DbConfig::new(),
            DbConfig {
                id_type: IdType::Uuid,
                id_key: "id".to_string(),
            }
        );
        assert_eq!(
            DbConfig::from(IdType::Int, "row_id"),
            DbConfig {
                id_type: IdType::Int,
                id_key: "row_id".to_string(),
            }
        );
        assert_eq!(
            DbConfig::int("number"),
            DbConfig {
                id_type: IdType::Int,
                id_key: "number".to_string(),
            }
        );
        assert_eq!(
            DbConfig::uuid("uuid"),
            DbConfig {
                id_type: IdType::Uuid,
                id_key: "uuid".to_string(),
            }
        );
        assert_eq!(
            DbConfig::none("external_id"),
            DbConfig {
                id_type: IdType::None,
                id_key: "external_id".to_string(),
            }
        );
    }
}
