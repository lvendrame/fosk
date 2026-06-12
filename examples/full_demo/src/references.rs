use std::error::Error;

use fosk::{Db, DbConfig};
use serde_json::json;

use crate::helpers::{pretty, required};

pub fn run(db: &Db) -> Result<(), Box<dyn Error>> {
    println!("== References and expansion ==");

    // References are used by expand_row and expand_list. They can be created
    // explicitly when names do not follow the default inference convention.
    assert!(db.create_reference("orders", "person_id", "people", "id"));
    assert!(db.create_reference("orderitems", "order_id", "orders", "id"));
    assert!(db.create_reference("orderitems", "product_id", "products", "id"));

    let orders = required(db.get("orders"), "orders collection should exist")?;
    let order = required(orders.get("1")?, "order id 1 should exist")?;
    let order_with_person = orders.expand_row(&order, "people", db)?;
    println!("Order expanded with person: {}", pretty(&order_with_person));

    let orderitems = required(db.get("orderitems"), "orderitems collection should exist")?;
    let first_items = orderitems.get_paginated(0, 2)?;
    let items_with_products = orderitems.expand_list(first_items, "products", db)?;
    println!(
        "First two order items expanded with products: {}",
        pretty(&items_with_products)
    );

    let refs = required(
        db.get_collection_refs("orderitems"),
        "orderitems references should exist",
    )?;
    println!("Registered references for orderitems: {:?}", refs.keys());

    // Inference works when a local field matches the target collection's
    // conventional reference column name.
    let inferred_db = Db::new_with_config(DbConfig::none("id"));
    inferred_db
        .create("people")
        .add(json!({ "id": 1, "name": "Ada" }))?;
    inferred_db
        .create("tickets")
        .add(json!({ "id": 10, "people_id": 1, "title": "Support" }))?;
    assert!(inferred_db.infer_reference("tickets", "people"));
    println!(
        "Inferred tickets.people_id -> people.id: {:?}",
        inferred_db.get_collection_column_ref("tickets", "people_id")
    );

    assert_eq!(order_with_person["people"][0]["full_name"], "Alice Johnson");
    assert!(!items_with_products.is_empty());
    println!();
    Ok(())
}
