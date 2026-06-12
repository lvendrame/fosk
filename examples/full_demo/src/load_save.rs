use std::error::Error;

use fosk::{Db, DbCollection, DbConfig};

use crate::helpers::{app_file, pretty, remove_temp_file, required, schema_summary, temp_file};

pub struct CollectionFixture {
    pub name: &'static str,
    pub file: &'static str,
    pub config: DbConfig,
}

pub fn run() -> Result<(), Box<dyn Error>> {
    println!("== Loading and saving JSON data ==");

    let db = Db::new_with_config(DbConfig::none("id"));

    // DB-level file loading expects an object whose keys are collection names.
    // The fixture lives in this independent app so users can inspect the shape.
    let db_fixture = app_file("mocks/collections/database.json").into_os_string();
    let status = db.load_from_file(&db_fixture)?;
    println!("{status}");
    println!("Serialized DB snapshot: {}", pretty(&db.write_to_json()?));

    // File APIs use OsString paths. The example writes to temp files so it can
    // run without any project-local fixtures.
    let db_path = temp_file("fosk-demo-db", "json");
    let db_os_path = db_path.clone().into_os_string();
    db.write_to_file(&db_os_path)?;
    println!("Wrote DB snapshot to {}", db_path.display());

    let people = DbCollection::new_coll("people_from_file", DbConfig::none("id"));
    let people_os_path = app_file("mocks/collections/people.json").into_os_string();
    let status = people.load_from_file(&people_os_path)?;
    println!("{status}; loaded rows: {}", people.count()?);

    // Standalone collection files cover common REST/mock-server fixture shapes:
    // auto int IDs, UUID IDs, explicit "None" IDs, custom ID field names,
    // nested objects, arrays, booleans, nulls, and relationship columns.
    let fixtures = collection_fixtures();
    println!(
        "Loading {} standalone collection fixture files:",
        fixtures.len()
    );
    for fixture in &fixtures {
        let collection = DbCollection::new_coll(fixture.name, fixture.config.clone());
        let path = app_file(fixture.file).into_os_string();
        let status = collection.load_from_file(&path)?;
        let schema = required(
            collection.schema()?,
            "fixture load should infer a collection schema",
        )?;
        println!(
            "  {name}: {rows} rows, config={config:?}, schema={schema}",
            name = fixture.name,
            rows = collection.count()?,
            config = collection.get_config()?,
            schema = schema_summary(&schema)
        );
        println!("    {status}");
    }

    remove_temp_file(&db_path);

    assert_eq!(db.list_collections().len(), 3);
    assert_eq!(people.count()?, 2);
    assert!(fixtures.len() > 10);
    println!();

    Ok(())
}

pub fn collection_fixtures() -> Vec<CollectionFixture> {
    vec![
        CollectionFixture {
            name: "users",
            file: "mocks/collections/users.json",
            config: DbConfig::int("id"),
        },
        CollectionFixture {
            name: "companies",
            file: "mocks/collections/companies.json",
            config: DbConfig::uuid("id"),
        },
        CollectionFixture {
            name: "products",
            file: "mocks/collections/products.json",
            config: DbConfig::int("_id"),
        },
        CollectionFixture {
            name: "cities",
            file: "mocks/collections/cities.json",
            config: DbConfig::none("id"),
        },
        CollectionFixture {
            name: "orders",
            file: "mocks/collections/orders.json",
            config: DbConfig::int("id"),
        },
        CollectionFixture {
            name: "order_items",
            file: "mocks/collections/order_items.json",
            config: DbConfig::int("id"),
        },
        CollectionFixture {
            name: "auth_tokens",
            file: "mocks/collections/auth_tokens.json",
            config: DbConfig::none("token"),
        },
        CollectionFixture {
            name: "blog_posts",
            file: "mocks/collections/blog_posts.json",
            config: DbConfig::none("slug"),
        },
        CollectionFixture {
            name: "comments",
            file: "mocks/collections/comments.json",
            config: DbConfig::int("id"),
        },
        CollectionFixture {
            name: "inventory_movements",
            file: "mocks/collections/inventory_movements.json",
            config: DbConfig::int("movement_id"),
        },
        CollectionFixture {
            name: "warehouses",
            file: "mocks/collections/warehouses.json",
            config: DbConfig::none("code"),
        },
        CollectionFixture {
            name: "feature_flags",
            file: "mocks/collections/feature_flags.json",
            config: DbConfig::none("key"),
        },
        CollectionFixture {
            name: "audit_events",
            file: "mocks/collections/audit_events.json",
            config: DbConfig::uuid("event_uuid"),
        },
        CollectionFixture {
            name: "payments",
            file: "mocks/collections/payments.json",
            config: DbConfig::int("payment_id"),
        },
        CollectionFixture {
            name: "shipments",
            file: "mocks/collections/shipments.json",
            config: DbConfig::none("tracking_number"),
        },
        CollectionFixture {
            name: "support_tickets",
            file: "mocks/collections/support_tickets.json",
            config: DbConfig::int("ticket_id"),
        },
        CollectionFixture {
            name: "geo_regions",
            file: "mocks/collections/geo_regions.json",
            config: DbConfig::none("region_code"),
        },
    ]
}
