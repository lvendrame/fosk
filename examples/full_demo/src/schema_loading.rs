use std::error::Error;

use fosk::{Db, DbConfig, JsonPrimitive};
use serde_json::json;

use crate::helpers::{app_file, schema_summary, schema_summary_fields};

pub fn run() -> Result<(), Box<dyn Error>> {
    println!("== Schema loading and inspection ==");

    // A single collection schema loaded from a JSON value needs the collection
    // name because a serde_json::Value has no filename to infer it from.
    let db = Db::new();
    db.load_collection_schema_from_json(
        "users",
        json!({
            "user_id": "Id",
            "name": "String!",
            "age": "Int"
        }),
    )
    .unwrap();
    let users = db.get("users").unwrap();
    println!(
        "Users config inferred from schema: {:?}",
        users.get_config()
    );
    println!("Users schema: {}", schema_summary(&users.schema().unwrap()));

    // Whole-DB schemas are keyed by collection name. DB-level schema loads infer
    // references after loading; here orders.user_id links to users.user_id.
    let loaded = db
        .load_schemas_from_json(json!({
            "sessions": {
                "session_uuid": "Uuid",
                "user_id": "Int!",
                "token": "String!"
            },
            "legacy_accounts": {
                "external_key": "None:String",
                "status": "String!"
            },
            "numeric_legacy": {
                "legacy_id": "None:Int",
                "label": "String"
            },
            "orders": {
                "order_id": "Id",
                "user_id": "Int!",
                "total": "Float!"
            }
        }))
        .unwrap();
    println!("Loaded {loaded} additional collection schemas.");
    println!(
        "Reference inferred from orders.user_id: {:?}",
        db.get_collection_column_ref("orders", "user_id")
    );

    let with_refs = db.schema_with_refs_of("orders").unwrap();
    println!(
        "Schema with refs for orders: fields={}, inbound={:?}, outbound={:?}",
        schema_summary_fields(&with_refs.fields),
        with_refs.inbound_refs.keys(),
        with_refs.outbound_refs.keys()
    );

    // File-based schema loaders infer collection names from file stems. These
    // fixture files live under mocks/schemas so users can inspect both shapes.
    let profile_schema_os_path = app_file("mocks/schemas/profiles.json").into_os_string();
    let status = db
        .load_collection_schema_from_file(&profile_schema_os_path)
        .unwrap();
    println!("{status}");

    let db_schema_os_path = app_file("mocks/schemas/database_schema.json").into_os_string();
    let status = db.load_schemas_from_file(&db_schema_os_path).unwrap();
    println!("{status}");

    // Alternate single-collection schema files show the compact format across
    // different ID strategies and field shapes. They load into a separate DB so
    // examples with different ID keys do not collide with the users schema above.
    let catalog_db = Db::new();
    let alternate_schema_files = [
        "mocks/schemas/users.json",
        "mocks/schemas/auth_tokens.json",
        "mocks/schemas/blog_posts.json",
        "mocks/schemas/comments.json",
        "mocks/schemas/inventory_movements.json",
        "mocks/schemas/warehouses.json",
        "mocks/schemas/feature_flags.json",
        "mocks/schemas/audit_events.json",
        "mocks/schemas/payments.json",
        "mocks/schemas/shipments.json",
        "mocks/schemas/support_tickets.json",
        "mocks/schemas/geo_regions.json",
    ];
    println!(
        "Loading {} alternate single-collection schema files:",
        alternate_schema_files.len()
    );
    for relative_path in alternate_schema_files {
        let path = app_file(relative_path).into_os_string();
        let status = catalog_db.load_collection_schema_from_file(&path).unwrap();
        println!("  {status}");
    }

    // Whole-DB schema files can describe alternate catalogs. This one reuses
    // names like orders and products with different ID conventions, so it loads
    // into its own DB instead of conflicting with the schemas above.
    let rest_schema_db = Db::new();
    let rest_schema_os_path = app_file("mocks/schemas/rest_resources.json").into_os_string();
    let status = rest_schema_db
        .load_schemas_from_file(&rest_schema_os_path)
        .unwrap();
    println!("{status}");
    println!(
        "REST resource schema collections: {:?}",
        rest_schema_db.list_collections()
    );

    assert_eq!(users.get_config(), DbConfig::int("user_id"));
    assert_eq!(
        users.schema().unwrap().fields["user_id"].ty,
        JsonPrimitive::Int
    );
    assert!(db.get_collection_column_ref("orders", "user_id").is_some());
    assert!(alternate_schema_files.len() > 10);
    println!();

    Ok(())
}
