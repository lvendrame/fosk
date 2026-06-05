use fosk::{DbCollection, DbConfig};
use serde_json::json;

use crate::helpers::pretty;

pub fn run() {
    println!("== Collection CRUD operations ==");

    let people = DbCollection::new_coll("people", DbConfig::none("id"));

    // add and add_batch store JSON objects. With DbConfig::none, the ID field
    // must already be present in each document.
    people.add(json!({
        "id": "ada",
        "name": "Ada",
        "profile": { "city": "London" }
    }));
    people.add_batch(json!([
        { "id": "grace", "name": "Grace" },
        { "id": "katherine", "name": "Katherine" }
    ]));

    println!("All people: {}", pretty(&people.get_all()));
    println!(
        "Page offset=1 limit=1: {}",
        pretty(&people.get_paginated(1, 1))
    );
    println!("Does 'ada' exist? {}", people.exists("ada"));

    // update_partial recursively merges object fields.
    let partial = people
        .update_partial("ada", json!({ "profile": { "role": "engineer" } }))
        .unwrap();
    println!("After partial update: {}", pretty(&partial));

    // update is a full replacement.
    let replaced = people
        .update("grace", json!({ "id": "grace", "name": "Grace Hopper" }))
        .unwrap();
    println!("After full replacement: {}", pretty(&replaced));

    let removed = people.delete("katherine").unwrap();
    println!("Deleted row: {}", pretty(&removed));
    println!("Remaining count before clear: {}", people.count());
    println!("Clear removed {} rows\n", people.clear());

    assert_eq!(partial["profile"]["city"], "London");
    assert_eq!(partial["profile"]["role"], "engineer");
    assert_eq!(replaced["name"], "Grace Hopper");
}
