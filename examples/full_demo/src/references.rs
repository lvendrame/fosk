use fosk::{Db, DbConfig};
use serde_json::json;

use crate::helpers::pretty;

pub fn run(db: &Db) {
    println!("== References and expansion ==");

    // References are used by expand_row and expand_list. They can be created
    // explicitly when names do not follow the default inference convention.
    assert!(db.create_reference("orders", "person_id", "people", "id"));
    assert!(db.create_reference("orderitems", "order_id", "orders", "id"));
    assert!(db.create_reference("orderitems", "product_id", "products", "id"));

    let orders = db.get("orders").unwrap();
    let order = orders.get("1").unwrap();
    let order_with_person = orders.expand_row(&order, "people", db);
    println!("Order expanded with person: {}", pretty(&order_with_person));

    let orderitems = db.get("orderitems").unwrap();
    let items_with_products =
        orderitems.expand_list(orderitems.get_paginated(0, 2), "products", db);
    println!(
        "First two order items expanded with products: {}",
        pretty(&items_with_products)
    );

    let refs = db.get_collection_refs("orderitems").unwrap();
    println!("Registered references for orderitems: {:?}", refs.keys());

    // Inference works when a local field matches the target collection's
    // conventional reference column name.
    let inferred_db = Db::new_with_config(DbConfig::none("id"));
    inferred_db
        .create("people")
        .add(json!({ "id": 1, "name": "Ada" }));
    inferred_db
        .create("tickets")
        .add(json!({ "id": 10, "people_id": 1, "title": "Support" }));
    assert!(inferred_db.infer_reference("tickets", "people"));
    println!(
        "Inferred tickets.people_id -> people.id: {:?}",
        inferred_db.get_collection_column_ref("tickets", "people_id")
    );

    assert_eq!(order_with_person["people"][0]["full_name"], "Alice Johnson");
    assert!(!items_with_products.is_empty());
    println!();
}
