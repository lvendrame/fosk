use std::error::Error;

use fosk::{
    FieldInfo, JsonPrimitive, ReferenceColumn, SchemaDict,
    database::{ColumnValue, IdManager, IdType, IdValue},
};
use serde_json::json;

use crate::helpers::{required, schema_summary};

pub fn run() -> Result<(), Box<dyn Error>> {
    println!("== Public schema metadata helpers ==");

    // Most users get schemas from collections, but these public helper types are
    // useful when inspecting or building schema metadata directly.
    let object_value = json!({
        "id": 1,
        "name": "Ada",
        "score": 98.5
    });
    let object = required(
        object_value.as_object(),
        "metadata example object should be a JSON object",
    )?
    .clone();
    let mut schema = SchemaDict::infer_schema_from_object(&object);

    let next_object_value = json!({
        "id": 2,
        "name": "Grace"
    });
    let next_object = required(
        next_object_value.as_object(),
        "metadata merge example should be a JSON object",
    )?
    .clone();
    schema.merge_schema(&next_object);

    println!("Manually inferred schema: {}", schema_summary(&schema));
    assert_eq!(
        required(schema.get("id"), "schema should contain id field")?.ty,
        JsonPrimitive::Int
    );
    assert!(required(schema.get("score"), "schema should contain score field")?.nullable);

    let int_info = FieldInfo::infer_field_info(&json!(1));
    let float_info = FieldInfo::infer_field_info(&json!(1.5));
    let merged_info = int_info.merge_field_info(&float_info);
    println!(
        "FieldInfo can promote numeric observations: {:?} + {:?} -> {:?}",
        int_info.ty, float_info.ty, merged_info.ty
    );
    assert_eq!(merged_info.ty, JsonPrimitive::Float);

    println!(
        "JsonPrimitive classifies JSON values: {:?}, {:?}, {:?}",
        JsonPrimitive::of_value(&json!(true)),
        JsonPrimitive::of_value(&json!([1, 2])),
        JsonPrimitive::promote(JsonPrimitive::Int, JsonPrimitive::Float)
    );

    let reference = ReferenceColumn::new(
        "orders".to_string(),
        "user_id".to_string(),
        "users".to_string(),
        "user_id".to_string(),
        false,
    );
    println!("ReferenceColumn metadata: {:?}", reference);

    let column_value = ColumnValue::new("user_id".to_string(), json!(1));
    println!(
        "ColumnValue helper for filtered lookups: {:?}",
        column_value
    );

    let mut id_manager = IdManager::new(IdType::Int);
    id_manager.set_current(IdValue::Int(41))?;
    println!(
        "IdManager can track current generated IDs: {:?}",
        id_manager
    );
    println!();
    Ok(())
}
