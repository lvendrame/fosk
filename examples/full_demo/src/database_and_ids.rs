use std::error::Error;

use fosk::{AddError, Db, DbConfig};
use serde_json::json;

use crate::helpers::required;

pub fn run() -> Result<(), Box<dyn Error>> {
    println!("== Database handles and ID strategies ==");

    // Db::new uses the crate default: generated UUID values stored in "id".
    let default_db = Db::new();
    println!("Default DB config: {:?}", default_db.get_config());

    // Db::new_arc is convenient when several owners need to share one DB.
    let shared = Db::new_arc();
    shared.create("shared_people");
    println!(
        "Shared DB contains collections: {:?}",
        shared.list_collections()
    );

    // DbConfig controls how collection IDs are handled.
    let int_db = Db::new_with_config(DbConfig::int("id"));
    let people = int_db.create("people");
    let ada = people.add(json!({ "name": "Ada Lovelace" }))?;
    println!("Auto-increment insert generated id: {}", ada["id"]);

    let sessions = int_db.create_with_config("sessions", DbConfig::uuid("session_id"));
    let session = sessions.add(json!({ "person_id": ada["id"] }))?;
    println!(
        "UUID insert generated session_id: {}",
        session["session_id"]
    );

    let audit = int_db.create_with_config("audit_log", DbConfig::none("event_id"));
    let accepted = audit.add(json!({ "event_id": "startup", "ok": true }))?;
    let rejected = audit.add(json!({ "ok": false }));
    let rejected_missing_id = match rejected {
        Err(AddError::MissingId { id_key }) => id_key == "event_id",
        other => panic!("expected missing event_id error, got {other:?}"),
    };
    println!(
        "Caller-provided IDs: accepted={}, rejected_missing_id={}",
        accepted["event_id"] == "startup",
        rejected_missing_id
    );

    assert_eq!(ada["id"], 1);
    let session_id = required(
        session["session_id"].as_str(),
        "generated session_id should be a string",
    )?;
    assert!(!session_id.is_empty());
    assert_eq!(accepted["event_id"], "startup");
    assert!(rejected_missing_id);

    // Collection names are case-insensitive and stored internally lowercase.
    println!("Collections before drop: {:?}", int_db.list_collections());
    let people_collection = required(int_db.get("PEOPLE"), "PEOPLE collection should exist")?;
    assert_eq!(people_collection.get_name()?, "people");
    assert!(int_db.drop_collection("PEOPLE"));
    println!(
        "Collections after dropping people: {:?}\n",
        int_db.list_collections()
    );
    Ok(())
}
