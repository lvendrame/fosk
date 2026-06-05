use indexmap::IndexMap;
use serde_json::Value;

use crate::database::{DbConfig, FieldInfo, IdType, JsonPrimitive, SchemaDict};

/// Parsed compact schema plus any collection configuration derived from an ID marker.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ParsedCompactSchema {
    /// Parsed schema fields.
    pub schema: SchemaDict,
    /// Optional collection config derived from `Id`, `Uuid`, or `None:Type`.
    pub config: Option<DbConfig>,
}

/// Parse a compact schema JSON object into a schema and optional ID configuration.
pub(crate) fn parse_compact_schema(value: &Value) -> Result<ParsedCompactSchema, String> {
    let Value::Object(fields) = value else {
        return Err("Schema JSON must be an object of field names to type strings".to_string());
    };

    let mut schema_fields = IndexMap::new();
    let mut config = None;

    for (field_name, type_value) in fields {
        let field_name = field_name.trim();
        if field_name.is_empty() {
            return Err("Schema field names cannot be empty".to_string());
        }

        let Value::String(type_spec) = type_value else {
            return Err(format!(
                "Schema field '{field_name}' must use a string type spec"
            ));
        };

        let parsed = parse_type_spec(field_name, type_spec)?;
        if let Some(id_config) = parsed.config {
            if config.is_some() {
                return Err("Only one ID marker is allowed per collection schema".to_string());
            }
            config = Some(id_config);
        }
        schema_fields.insert(field_name.to_string(), parsed.field_info);
    }

    Ok(ParsedCompactSchema {
        schema: SchemaDict {
            fields: schema_fields,
        },
        config,
    })
}

/// Validate that a schema can be used with a collection configuration.
pub(crate) fn validate_schema_config(schema: &SchemaDict, config: &DbConfig) -> Result<(), String> {
    let Some(id_field) = schema.fields.get(&config.id_key) else {
        return Err(format!(
            "Schema must contain configured id field '{}'",
            config.id_key
        ));
    };

    if id_field.nullable {
        return Err(format!(
            "Configured id field '{}' cannot be nullable",
            config.id_key
        ));
    }

    match config.id_type {
        IdType::Int => {
            if id_field.ty != JsonPrimitive::Int {
                return Err(format!(
                    "Id field '{}' must be Int for auto-increment collections",
                    config.id_key
                ));
            }
        }
        IdType::Uuid => {
            if id_field.ty != JsonPrimitive::String {
                return Err(format!(
                    "Id field '{}' must be String for UUID collections",
                    config.id_key
                ));
            }
        }
        IdType::None => {
            if !matches!(
                id_field.ty,
                JsonPrimitive::Int | JsonPrimitive::Float | JsonPrimitive::String
            ) {
                return Err(format!(
                    "Id field '{}' for IdType::None must be Int, Float, or String",
                    config.id_key
                ));
            }
        }
    }

    Ok(())
}

struct ParsedTypeSpec {
    field_info: FieldInfo,
    config: Option<DbConfig>,
}

fn parse_type_spec(field_name: &str, type_spec: &str) -> Result<ParsedTypeSpec, String> {
    let type_spec = type_spec.trim();
    if type_spec.is_empty() {
        return Err(format!(
            "Schema field '{field_name}' has an empty type spec"
        ));
    }

    match type_spec {
        "Id" => {
            return Ok(ParsedTypeSpec {
                field_info: FieldInfo {
                    ty: JsonPrimitive::Int,
                    nullable: false,
                },
                config: Some(DbConfig::int(field_name)),
            });
        }
        "Uuid" => {
            return Ok(ParsedTypeSpec {
                field_info: FieldInfo {
                    ty: JsonPrimitive::String,
                    nullable: false,
                },
                config: Some(DbConfig::uuid(field_name)),
            });
        }
        _ => {}
    }

    if let Some(type_name) = type_spec.strip_prefix("None:") {
        if type_name.is_empty() {
            return Err(format!(
                "Schema field '{field_name}' must specify a type after None:"
            ));
        }
        if type_name.ends_with('!') {
            return Err(format!(
                "Schema field '{field_name}' cannot use ! with None: ID markers"
            ));
        }

        let ty = parse_json_primitive(type_name)?;
        if !matches!(
            ty,
            JsonPrimitive::Int | JsonPrimitive::Float | JsonPrimitive::String
        ) {
            return Err(format!(
                "Schema field '{field_name}' uses invalid None ID type '{type_name}'"
            ));
        }

        return Ok(ParsedTypeSpec {
            field_info: FieldInfo {
                ty,
                nullable: false,
            },
            config: Some(DbConfig::none(field_name)),
        });
    }

    if type_spec.contains('!') && !type_spec.ends_with('!') {
        return Err(format!(
            "Schema field '{field_name}' has malformed nullability marker"
        ));
    }

    let bang_count = type_spec.chars().filter(|ch| *ch == '!').count();
    if bang_count > 1 {
        return Err(format!(
            "Schema field '{field_name}' has multiple nullability markers"
        ));
    }

    let nullable = bang_count == 0;
    let type_name = type_spec.strip_suffix('!').unwrap_or(type_spec);
    let ty = parse_json_primitive(type_name)?;
    if ty == JsonPrimitive::Null && !nullable {
        return Err(format!("Schema field '{field_name}' cannot use Null!"));
    }

    Ok(ParsedTypeSpec {
        field_info: FieldInfo { ty, nullable },
        config: None,
    })
}

fn parse_json_primitive(type_name: &str) -> Result<JsonPrimitive, String> {
    match type_name {
        "Null" => Ok(JsonPrimitive::Null),
        "Bool" => Ok(JsonPrimitive::Bool),
        "Int" => Ok(JsonPrimitive::Int),
        "Float" => Ok(JsonPrimitive::Float),
        "String" => Ok(JsonPrimitive::String),
        "Object" => Ok(JsonPrimitive::Object),
        "Array" => Ok(JsonPrimitive::Array),
        _ => Err(format!("Unknown schema type '{type_name}'")),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn parses_regular_nullable_and_non_nullable_fields() {
        let parsed =
            parse_compact_schema(&json!({ "name": "String!", "email": "String" })).unwrap();

        assert_eq!(parsed.config, None);
        assert_eq!(parsed.schema.fields["name"].ty, JsonPrimitive::String);
        assert!(!parsed.schema.fields["name"].nullable);
        assert_eq!(parsed.schema.fields["email"].ty, JsonPrimitive::String);
        assert!(parsed.schema.fields["email"].nullable);
    }

    #[test]
    fn parses_id_markers_with_custom_field_names() {
        let cases = [
            ("id", "Id", DbConfig::int("id"), JsonPrimitive::Int),
            (
                "user_id",
                "Id",
                DbConfig::int("user_id"),
                JsonPrimitive::Int,
            ),
            (
                "uuid",
                "Uuid",
                DbConfig::uuid("uuid"),
                JsonPrimitive::String,
            ),
            (
                "session_uuid",
                "Uuid",
                DbConfig::uuid("session_uuid"),
                JsonPrimitive::String,
            ),
            (
                "external_key",
                "None:String",
                DbConfig::none("external_key"),
                JsonPrimitive::String,
            ),
            (
                "legacy_id",
                "None:Int",
                DbConfig::none("legacy_id"),
                JsonPrimitive::Int,
            ),
        ];

        for (field_name, marker, expected_config, expected_type) in cases {
            let parsed = parse_compact_schema(&json!({ field_name: marker })).unwrap();
            assert_eq!(parsed.config, Some(expected_config));
            assert_eq!(parsed.schema.fields[field_name].ty, expected_type);
            assert!(!parsed.schema.fields[field_name].nullable);
        }
    }

    #[test]
    fn rejects_invalid_none_id_specs() {
        for marker in ["None:Int!", "None:String!", "None:Null", "None:"] {
            let err = parse_compact_schema(&json!({ "id": marker })).unwrap_err();
            assert!(
                err.contains("None") || err.contains("type"),
                "unexpected error: {err}"
            );
        }
    }

    #[test]
    fn rejects_duplicate_id_markers() {
        let err = parse_compact_schema(&json!({ "id": "Id", "uuid": "Uuid" })).unwrap_err();
        assert!(err.contains("Only one ID marker"));
    }

    #[test]
    fn rejects_malformed_types() {
        for schema in [
            json!("String"),
            json!({ "name": 1 }),
            json!({ "name": "Unknown" }),
            json!({ "name": "String!!" }),
            json!({ "name": "!String" }),
        ] {
            assert!(parse_compact_schema(&schema).is_err(), "{schema:?}");
        }
    }

    #[test]
    fn validates_schema_against_collection_config() {
        let parsed = parse_compact_schema(&json!({ "user_id": "Id", "name": "String" })).unwrap();

        assert!(validate_schema_config(&parsed.schema, &DbConfig::int("user_id")).is_ok());
        assert!(validate_schema_config(&parsed.schema, &DbConfig::int("id")).is_err());
        assert!(validate_schema_config(&parsed.schema, &DbConfig::uuid("user_id")).is_err());
    }
}
