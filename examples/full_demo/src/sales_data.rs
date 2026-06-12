use std::error::Error;

use fosk::{Db, DbConfig};

use crate::helpers::{app_file, required};

pub fn seed() -> Result<Db, Box<dyn Error>> {
    println!("== Seeding a sales database ==");

    let db = Db::new_with_config(DbConfig::none("id"));

    // This fixture mirrors the shape of the executor tests and adds nested
    // objects, arrays, booleans, and nulls so schema inference has more to show.
    let fixture = app_file("mocks/collections/sales_database.json").into_os_string();
    let status = db.load_from_file(&fixture)?;
    println!("{status}");

    println!("Seeded collections: {:?}", db.list_collections());
    for name in ["people", "products", "orders", "orderitems"] {
        let collection = required(db.get(name), "seeded collection should exist")?;
        println!("  {name}: {} rows", collection.count()?);
    }
    println!();

    Ok(db)
}
