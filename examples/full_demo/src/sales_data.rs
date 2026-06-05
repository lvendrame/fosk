use fosk::{Db, DbConfig};

use crate::helpers::app_file;

pub fn seed() -> Db {
    println!("== Seeding a sales database ==");

    let db = Db::new_with_config(DbConfig::none("id"));

    // This fixture mirrors the shape of the executor tests and adds nested
    // objects, arrays, booleans, and nulls so schema inference has more to show.
    let fixture = app_file("mocks/collections/sales_database.json").into_os_string();
    let status = db.load_from_file(&fixture).unwrap();
    println!("{status}");

    println!("Seeded collections: {:?}", db.list_collections());
    for name in ["people", "products", "orders", "orderitems"] {
        println!("  {name}: {} rows", db.get(name).unwrap().count());
    }
    println!();

    db
}
