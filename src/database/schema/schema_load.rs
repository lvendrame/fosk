use crate::database::{
    DbCollection, DbConfig, ParsedCompactSchema, parse_compact_schema, validate_schema_config,
};
use serde_json::Value;

/// Parse a compact schema value for loading.
pub(crate) fn parse_schema_for_load(value: &Value) -> Result<ParsedCompactSchema, String> {
    parse_compact_schema(value)
}

/// Apply a parsed schema to an existing collection without mutating rows or ID state.
pub(crate) fn apply_schema_to_collection(
    collection: &DbCollection,
    parsed: ParsedCompactSchema,
) -> Result<(), String> {
    let existing_config = match collection.get_config() {
        Ok(config) => config,
        Err(error) => return Err(error.to_string()),
    };
    if let Some(loaded_config) = &parsed.config {
        validate_loaded_config_matches_existing(loaded_config, &existing_config)?;
    }

    validate_schema_config(&parsed.schema, &existing_config)?;
    match collection.set_schema(parsed.schema) {
        Ok(()) => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

/// Choose the collection config to use when a DB schema load must create a collection.
pub(crate) fn config_for_missing_collection(
    parsed: &ParsedCompactSchema,
    default_config: &DbConfig,
) -> DbConfig {
    parsed
        .config
        .clone()
        .unwrap_or_else(|| default_config.clone())
}

fn validate_loaded_config_matches_existing(
    loaded_config: &DbConfig,
    existing_config: &DbConfig,
) -> Result<(), String> {
    if loaded_config != existing_config {
        return Err(format!(
            "Loaded schema ID marker conflicts with existing collection config: loaded {:?}, existing {:?}",
            loaded_config, existing_config
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::database::{DbCollection, IdType, JsonPrimitive};

    use super::*;

    #[test]
    fn applies_schema_without_clearing_rows_or_resetting_int_ids() {
        let collection = DbCollection::new_coll("users", DbConfig::int("user_id"));
        collection.add(json!({ "name": "Existing" })).unwrap();

        let parsed = parse_schema_for_load(&json!({
            "user_id": "Id",
            "name": "String!"
        }))
        .unwrap();
        apply_schema_to_collection(&collection, parsed).unwrap();

        assert_eq!(collection.count().unwrap(), 1);
        let inserted = collection.add(json!({ "name": "Next" })).unwrap();
        assert_eq!(inserted["user_id"], 2);
    }

    #[test]
    fn rejects_conflicting_existing_collection_config() {
        let collection = DbCollection::new_coll("users", DbConfig::uuid("user_id"));
        let parsed = parse_schema_for_load(&json!({ "user_id": "Id" })).unwrap();

        assert!(apply_schema_to_collection(&collection, parsed).is_err());
    }

    #[test]
    fn applies_schema_when_loaded_config_matches_existing_collection_config() {
        let collection = DbCollection::new_coll("users", DbConfig::int("user_id"));
        let parsed = parse_schema_for_load(&json!({
            "user_id": "Id",
            "email": "String"
        }))
        .unwrap();

        apply_schema_to_collection(&collection, parsed).unwrap();

        let schema = collection.schema().unwrap().unwrap();
        assert_eq!(schema.fields["email"].ty, JsonPrimitive::String);
    }

    #[test]
    fn chooses_marker_config_for_missing_collection() {
        let parsed = parse_schema_for_load(&json!({ "session_uuid": "Uuid" })).unwrap();
        let config = config_for_missing_collection(&parsed, &DbConfig::int("id"));

        assert_eq!(config.id_type, IdType::Uuid);
        assert_eq!(config.id_key, "session_uuid");
    }

    #[test]
    fn uses_default_config_when_missing_collection_schema_has_no_id_marker() {
        let parsed = parse_schema_for_load(&json!({ "name": "String" })).unwrap();
        let config = config_for_missing_collection(&parsed, &DbConfig::int("fallback_id"));

        assert_eq!(config, DbConfig::int("fallback_id"));
    }

    #[test]
    fn later_inserts_merge_with_loaded_schema() {
        let collection = DbCollection::new_coll("users", DbConfig::int("id"));
        let parsed = parse_schema_for_load(&json!({
            "id": "Id",
            "name": "String!"
        }))
        .unwrap();
        apply_schema_to_collection(&collection, parsed).unwrap();

        collection.add(json!({ "name": "Ada", "age": 37 })).unwrap();
        let schema = collection.schema().unwrap().unwrap();

        assert_eq!(schema.fields["age"].ty, JsonPrimitive::Int);
    }
}
