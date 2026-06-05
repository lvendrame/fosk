use fosk::{Db, DbConfig};
use serde_json::json;

pub fn run() {
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
    let ada = people.add(json!({ "name": "Ada Lovelace" })).unwrap();
    println!("Auto-increment insert generated id: {}", ada["id"]);

    let sessions = int_db.create_with_config("sessions", DbConfig::uuid("session_id"));
    let session = sessions.add(json!({ "person_id": ada["id"] })).unwrap();
    println!(
        "UUID insert generated session_id: {}",
        session["session_id"]
    );

    let audit = int_db.create_with_config("audit_log", DbConfig::none("event_id"));
    let accepted = audit.add(json!({ "event_id": "startup", "ok": true }));
    let rejected = audit.add(json!({ "ok": false }));
    println!(
        "Caller-provided IDs: accepted={}, rejected_missing_id={}",
        accepted.is_some(),
        rejected.is_none()
    );

    assert_eq!(ada["id"], 1);
    assert!(session["session_id"].as_str().is_some());
    assert!(accepted.is_some());
    assert!(rejected.is_none());

    // Collection names are case-insensitive and stored internally lowercase.
    println!("Collections before drop: {:?}", int_db.list_collections());
    assert!(int_db.get("PEOPLE").is_some());
    assert!(int_db.drop_collection("PEOPLE"));
    println!(
        "Collections after dropping people: {:?}\n",
        int_db.list_collections()
    );
}
