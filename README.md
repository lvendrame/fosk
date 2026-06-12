<p align="center">
  <img src="images/fosk_logo.png" height="200" alt="FOSK logo">
</p>

# FOSK

`fosk` is a lightweight embedded SQL engine for Rust applications.
It allows you to define in-memory collections, seed them with JSON objects, and query using a SQL-like syntax.

---

## ✨ Features

- In-memory database with collections (tables)
- Configurable ID strategies: integer, UUID, or none
- Simple JSON storage (serde_json::Value)
- SQL parser with support for:
  - SELECT, WHERE, GROUP BY, HAVING
  - JOIN (inner, left, right, full)
  - Non-correlated FROM/JOIN subqueries with required aliases
  - ORDER BY, LIMIT, OFFSET
  - Parameterized queries (? placeholders, including arrays)
- Test-friendly: create databases on the fly and seed them

---

## Installation

In your `Cargo.toml`:

```toml
[dependencies]
fosk = "0.1.15"
serde_json = "1"
```

---

## Quick example

```rust
use fosk::{Db, DbConfig};
use serde_json::json;

fn main() {
    let db = Db::new_with_config(DbConfig::int("id"));
    let people = db.create("people");

    let alice = people.add(json!({ "name": "Alice", "age": 30 })).unwrap();
    people.add_batch(json!([
        { "name": "Bob", "age": 25 },
        { "name": "Carol", "age": 28 }
    ]));

    let rows = db.query_with_args(
        "SELECT id, name, age FROM people WHERE id = ?",
        alice["id"].clone(),
    ).unwrap();

    println!("Query result: {rows:?}");
}
```

For a larger executable walkthrough of the public API, run:

```bash
cargo run --manifest-path examples/full_demo/Cargo.toml
```

The app is an independent Cargo project under `examples/full_demo`. It includes
mock collection and schema files under `examples/full_demo/mocks`, including
UUID IDs, auto-increment IDs, caller-provided `None:*` IDs, custom ID field
names, nested objects, arrays, nullable fields, and relationship-shaped data.
It also runs SQL examples over the loaded files: boolean filters, `LIKE`,
`IS NULL`, parameterized `IN (?)`, `NOT IN`, joins, `COUNT(DISTINCT ...)`,
`SUM`, `AVG`, `GROUP BY`, `HAVING`, `ORDER BY`, `OFFSET`, and `LIMIT`.

Example app map:

- Database handles and ID strategies: [`examples/full_demo/src/database_and_ids.rs`](examples/full_demo/src/database_and_ids.rs)
- Collection CRUD: [`examples/full_demo/src/collection_crud.rs`](examples/full_demo/src/collection_crud.rs)
- JSON file loading/saving and fixture catalog: [`examples/full_demo/src/load_save.rs`](examples/full_demo/src/load_save.rs)
- Queries over complex loaded fixtures: [`examples/full_demo/src/complex_queries.rs`](examples/full_demo/src/complex_queries.rs)
- Sales-style join/aggregate queries: [`examples/full_demo/src/queries.rs`](examples/full_demo/src/queries.rs)
- Reference creation, inference, and expansion: [`examples/full_demo/src/references.rs`](examples/full_demo/src/references.rs)
- Schema loading from JSON values and files: [`examples/full_demo/src/schema_loading.rs`](examples/full_demo/src/schema_loading.rs)
- Public schema metadata helpers: [`examples/full_demo/src/metadata.rs`](examples/full_demo/src/metadata.rs)
- Collection fixtures: [`examples/full_demo/mocks/collections`](examples/full_demo/mocks/collections)
- Schema fixtures: [`examples/full_demo/mocks/schemas`](examples/full_demo/mocks/schemas)

---

## 📚 Public API Guide

### Create a database

`Db` is the user-facing database handle. It owns named collections, stores the default collection configuration, runs SQL queries, and manages schema/reference metadata.

```rust
use fosk::{Db, DbConfig, IdType};

let default_db = Db::new();
assert_eq!(default_db.get_config().id_type, IdType::Uuid);

let int_db = Db::new_with_config(DbConfig::int("id"));
assert_eq!(int_db.get_config(), DbConfig::int("id"));

let shared = Db::new_arc();
shared.create("people");
assert!(shared.get("people").is_some());
```

Runnable example: [`examples/full_demo/src/database_and_ids.rs`](examples/full_demo/src/database_and_ids.rs)

### Configure IDs

`DbConfig` controls how a collection handles IDs. The database config is copied into collections created with `Db::create`; use `Db::create_with_config` when one collection needs a different strategy.

```rust
use fosk::{Db, DbConfig, IdType};
use serde_json::json;

let db = Db::new_with_config(DbConfig::int("id"));
let people = db.create("people");
let inserted = people.add(json!({ "name": "Ada" })).unwrap();
assert_eq!(inserted["id"], 1);

let sessions = db.create_with_config("sessions", DbConfig::uuid("session_id"));
let session = sessions.add(json!({ "user_id": 1 })).unwrap();
assert!(session["session_id"].as_str().is_some());

let logs = db.create_with_config("logs", DbConfig::none("key"));
assert!(logs.add(json!({ "key": "startup", "ok": true })).is_some());
assert!(logs.add(json!({ "ok": false })).is_none());

assert_eq!(logs.get_config().id_type, IdType::None);
```

Available constructors:

- `DbConfig::new()` uses UUID IDs in the `id` field.
- `DbConfig::int("id")` uses auto-increment integer IDs.
- `DbConfig::uuid("id")` uses generated UUID strings.
- `DbConfig::none("id")` requires callers to provide the ID field.

Runnable examples:

- ID strategies in code: [`examples/full_demo/src/database_and_ids.rs`](examples/full_demo/src/database_and_ids.rs)
- Mixed ID fixture loading: [`examples/full_demo/src/load_save.rs`](examples/full_demo/src/load_save.rs)
- Fixtures with UUID, `_id`, and caller-provided IDs: [`examples/full_demo/mocks/collections`](examples/full_demo/mocks/collections)

### Manage collections

Collection names are stored case-insensitively. Creating an existing collection name replaces it with a new empty collection.

```rust
use fosk::{Db, DbConfig};
use serde_json::json;

let db = Db::new_with_config(DbConfig::int("id"));

let people = db.create("People");
people.add(json!({ "name": "Ada" }));

assert!(db.get("people").is_some());
assert_eq!(db.list_collections(), vec!["people"]);

assert!(db.drop_collection("PEOPLE"));
assert!(db.list_collections().is_empty());

db.create("orders");
db.clear();
assert!(db.list_collections().is_empty());
```

Runnable example: [`examples/full_demo/src/database_and_ids.rs`](examples/full_demo/src/database_and_ids.rs)

### Work with documents

`DbCollection` exposes read, write, pagination, replacement, partial update, and deletion helpers. IDs are looked up as strings even when stored as numbers.

```rust
use fosk::{DbCollection, DbConfig};
use serde_json::json;

let people = DbCollection::new_coll("people", DbConfig::none("id"));

people.add(json!({ "id": "ada", "name": "Ada", "profile": { "city": "London" } }));
people.add_batch(json!([
    { "id": "grace", "name": "Grace" },
    { "id": "katherine", "name": "Katherine" }
]));

assert_eq!(people.count(), 3);
assert!(people.exists("ada"));
assert_eq!(people.get("ada").unwrap()["name"], "Ada");
assert_eq!(people.get_paginated(1, 1).len(), 1);

let updated = people
    .update_partial("ada", json!({ "profile": { "role": "engineer" } }))
    .unwrap();
assert_eq!(updated["profile"]["city"], "London");
assert_eq!(updated["profile"]["role"], "engineer");

let replaced = people
    .update("grace", json!({ "id": "grace", "name": "Grace Hopper" }))
    .unwrap();
assert_eq!(replaced["name"], "Grace Hopper");

assert!(people.delete("katherine").is_some());
assert_eq!(people.clear(), 2);
```

Runnable example: [`examples/full_demo/src/collection_crud.rs`](examples/full_demo/src/collection_crud.rs)

### Load existing data

```rust
use fosk::{Db, DbCollection, DbConfig};
use serde_json::json;
use std::ffi::OsString;

let db = Db::new();

let loaded = db.load_from_json(json!({
    "people": [
        { "id": 1, "name": "Ada" },
        { "id": 2, "name": "Grace" }
    ],
    "companies": [
        { "id": 1, "name": "ACME" }
    ]
}), true).unwrap();
assert_eq!(loaded, 2);

let people = DbCollection::new_coll("people", DbConfig::none("id"));
let inserted = people
    .load_from_json(json!([{ "id": 1, "name": "Ada" }]), true)
    .unwrap();
assert_eq!(inserted.len(), 1);

// File APIs accept OsString paths and return human-readable status strings.
// db.load_from_file(&OsString::from("collections.json"))?;
// people.load_from_file(&OsString::from("people.json"))?;
```

The `keep` flag controls incoming IDs:

- `true` preserves IDs from loaded documents where possible.
- `false` allows IDs to be regenerated according to the collection config.

Runnable examples:

- DB and collection file loading: [`examples/full_demo/src/load_save.rs`](examples/full_demo/src/load_save.rs)
- Coherent whole-DB fixture: [`examples/full_demo/mocks/collections/database.json`](examples/full_demo/mocks/collections/database.json)
- Sales DB fixture used by query/reference examples: [`examples/full_demo/mocks/collections/sales_database.json`](examples/full_demo/mocks/collections/sales_database.json)
- Standalone collection fixtures with different ID conventions: [`examples/full_demo/mocks/collections`](examples/full_demo/mocks/collections)

### Save data

```rust
use fosk::{Db, DbConfig};
use serde_json::json;
use std::ffi::OsString;

let db = Db::new_with_config(DbConfig::none("id"));
let people = db.create("people");
people.add(json!({ "id": 1, "name": "Ada" }));

let dump = db.write_to_json();
assert_eq!(dump["people"][0]["name"], "Ada");

// db.write_to_file(&OsString::from("collections.json"))?;
// people.write_to_file(&OsString::from("people.json"))?;
```

Runnable example: [`examples/full_demo/src/load_save.rs`](examples/full_demo/src/load_save.rs)

### Query data

Use `query` for SQL without placeholders and `query_with_args` for positional `?` parameters. Pass one JSON value for one placeholder, or a JSON array for multiple placeholders. Arrays can also be used inside `IN (?)`.

```rust
use fosk::{Db, DbConfig};
use serde_json::json;

let db = Db::new_with_config(DbConfig::none("id"));
db.create("people").add_batch(json!([
    { "id": 1, "name": "Ada", "age": 37, "city": "London" },
    { "id": 2, "name": "Grace", "age": 29, "city": "Arlington" }
]));

let older = db
    .query("SELECT name FROM people WHERE age > 30")
    .unwrap();
assert_eq!(older[0]["name"], "Ada");

let selected = db
    .query_with_args(
        "SELECT name FROM people WHERE id IN (?) ORDER BY id",
        json!([[1, 2]])
    )
    .unwrap();
assert_eq!(selected.len(), 2);
```

Supported SQL includes `SELECT`, `WHERE`, `GROUP BY`, `HAVING`, joins, non-correlated `FROM`/`JOIN` subqueries with aliases, `ORDER BY`, `LIMIT`, `OFFSET`, aggregate functions, aliases, and positional parameters.

Runnable examples:

- Queries over file-loaded complex fixtures: [`examples/full_demo/src/complex_queries.rs`](examples/full_demo/src/complex_queries.rs)
- Sales-style joins and aggregate reports: [`examples/full_demo/src/queries.rs`](examples/full_demo/src/queries.rs)
- Query fixture catalog: [`examples/full_demo/mocks/collections`](examples/full_demo/mocks/collections)

### References and expansion

References are foreign-key-like mappings between collection fields. They can be declared manually or inferred from collection naming conventions, then used to expand rows with related records.

```rust
use fosk::{Db, DbConfig};
use serde_json::json;

let db = Db::new_with_config(DbConfig::none("id"));
let people = db.create("people");
let orders = db.create("orders");

people.add(json!({ "id": 1, "name": "Ada" }));
orders.add(json!({ "id": 10, "person_id": 1, "total": 42.0 }));

assert!(db.create_reference("orders", "person_id", "people", "id"));
assert!(db.get_collection_column_ref("orders", "person_id").is_some());

let expanded = orders.expand_row(&orders.get("10").unwrap(), "people", &db);
assert_eq!(expanded["people"][0]["name"], "Ada");
```

`infer_reference("orders", "people")` looks for the referenced collection's conventional reference field name. For example, a `people` collection with ID key `id` expects `people_id`; a `users` collection with ID key `user_id` expects `user_id`.

Runnable examples:

- Manual references, inferred references, and row/list expansion: [`examples/full_demo/src/references.rs`](examples/full_demo/src/references.rs)
- Relationship-shaped sales fixture: [`examples/full_demo/mocks/collections/sales_database.json`](examples/full_demo/mocks/collections/sales_database.json)

### Load collection schemas

Collection schemas can be loaded before inserting data. They define field names, field types, nullability, and optionally the collection ID behavior. References are not written in schema files; when schemas are loaded through `Db`, references are inferred after the load using the same inference rules already used by the database.

#### Compact collection schema format

A single collection schema is a JSON object where each key is a field name and each value is a compact type string:

```json
{
  "user_id": "Id",
  "name": "String!",
  "age": "Int",
  "active": "Bool!"
}
```

Supported regular field types are:

- `Null`
- `Bool`
- `Int`
- `Float`
- `String`
- `Object`
- `Array`

Nullability is declared with `!`:

- `"age": "Int"` means `age` is nullable.
- `"age": "Int!"` means `age` is non-nullable.

#### ID markers

One field can be marked as the collection ID field:

- `"id": "Id"` uses auto-increment integer IDs and stores the field as `Int!`.
- `"uuid": "Uuid"` uses generated UUID IDs and stores the field as `String!`.
- `"external_id": "None:String"` uses caller-provided IDs and stores the field as `String!`.
- `"legacy_id": "None:Int"` uses caller-provided IDs and stores the field as `Int!`.

`None:Type` ID markers are always non-nullable because the field is the collection ID. Nullable forms such as `None:Int!`, `None:String!`, and `None:Null` are rejected.

#### Load one collection schema

```rust
use fosk::Db;
use serde_json::json;

let db = Db::new();

db.load_collection_schema_from_json("users", json!({
    "user_id": "Id",
    "name": "String!",
    "age": "Int"
}));
```

For direct JSON values, the collection name must be provided because there is no filename or parent object key to infer it from.

Runnable example: [`examples/full_demo/src/schema_loading.rs`](examples/full_demo/src/schema_loading.rs)

Schema files for one collection contain only the compact field map:

```json
{
  "user_id": "Id",
  "name": "String!",
  "age": "Int"
}
```

The collection name is inferred from the file stem:

```rust
use fosk::Db;

let db = Db::new();

// Loads into the `users` collection.
// db.load_collection_schema_from_file(&"users.json".into())?;
```

Runnable examples:

- Single schema file loading: [`examples/full_demo/src/schema_loading.rs`](examples/full_demo/src/schema_loading.rs)
- Single-collection schema fixtures: [`examples/full_demo/mocks/schemas`](examples/full_demo/mocks/schemas)

#### Load all collection schemas

To load the whole database schema, use a JSON object keyed by collection name:

```rust
use fosk::Db;
use serde_json::json;

let db = Db::new();

db.load_schemas_from_json(json!({
    "users": {
        "user_id": "Id",
        "name": "String!"
    },
    "orders": {
        "order_id": "Uuid",
        "user_id": "Int!",
        "total": "Float!"
    }
}));
```

The same structure can be loaded from a file:

```json
{
  "users": {
    "user_id": "Id",
    "name": "String!"
  },
  "orders": {
    "order_id": "Uuid",
    "user_id": "Int!",
    "total": "Float!"
  }
}
```

```rust
use fosk::Db;

let db = Db::new();

// db.load_schemas_from_file(&"schema.json".into())?;
```

Runnable examples:

- Whole-DB schema loading: [`examples/full_demo/src/schema_loading.rs`](examples/full_demo/src/schema_loading.rs)
- Whole-DB schema fixture: [`examples/full_demo/mocks/schemas/database_schema.json`](examples/full_demo/mocks/schemas/database_schema.json)
- Alternate REST-resource schema fixture: [`examples/full_demo/mocks/schemas/rest_resources.json`](examples/full_demo/mocks/schemas/rest_resources.json)

#### Load schema on an existing collection

Collection handles can load only their own schema:

```rust
use fosk::Db;
use serde_json::json;

let db = Db::new();
let users = db.create_with_config("users", fosk::DbConfig::int("user_id"));

users.load_schema_from_json(json!({
    "user_id": "Id",
    "name": "String!"
}));
```

Collection-level schema loading validates any ID marker against the collection's existing `DbConfig`; it does not change the collection config, stored rows, or ID generator state.

Runnable examples:

- Schema loading APIs in one place: [`examples/full_demo/src/schema_loading.rs`](examples/full_demo/src/schema_loading.rs)
- ID marker variants in schema files: [`examples/full_demo/mocks/schemas`](examples/full_demo/mocks/schemas)

### Inspect schemas

```rust
use fosk::{Db, DbConfig, JsonPrimitive};
use serde_json::json;

let db = Db::new_with_config(DbConfig::none("id"));
let people = db.create("people");
people.add(json!({ "id": 1, "name": "Ada" }));

let schema = people.schema().unwrap();
assert_eq!(schema.fields["name"].ty, JsonPrimitive::String);

let schema_with_refs = db.schema_with_refs_of("people").unwrap();
assert_eq!(schema_with_refs.name, "people");
```

Runnable examples:

- Schema inspection after loading: [`examples/full_demo/src/schema_loading.rs`](examples/full_demo/src/schema_loading.rs)
- Direct metadata helper usage: [`examples/full_demo/src/metadata.rs`](examples/full_demo/src/metadata.rs)

Useful metadata types:

- `JsonPrimitive` classifies fields as `Null`, `Bool`, `Int`, `Float`, `String`, `Object`, or `Array`.
- `FieldInfo` stores a field's primitive type and nullability.
- `SchemaDict` stores field metadata for one collection.
- `SchemaWithRefs` combines a collection schema with inbound and outbound references.
- `ReferenceColumn` describes one relationship between two collection fields.

---

## 🧪 Testing & Seeding

Example test seed (see [fixtures::seed_db](https://github.com/lvendrame/fosk/blob/main/src/executor/_tests.rs#L252)):

```rust
pub fn seed_db() -> Db {
    let db = Db::new_with_config(DbConfig::none("id"));

    create_people(&db);
    create_products(&db);
    create_orders(&db);
    create_order_items(&db);

    db
}
```

Runnable examples:

- Example app seed from a fixture file: [`examples/full_demo/src/sales_data.rs`](examples/full_demo/src/sales_data.rs)
- Sales fixture used by the seed: [`examples/full_demo/mocks/collections/sales_database.json`](examples/full_demo/mocks/collections/sales_database.json)
- Test-style query examples reproduced as logs: [`examples/full_demo/src/queries.rs`](examples/full_demo/src/queries.rs)

---

## ⚠️ Notes

- Projections normally output unqualified field names (id, name), unless duplicates exist.
  In case of conflicts, names are disambiguated with their collection prefix (id, o.id).

---

## 📄 License

Licensed under the _MIT License_.
See [LICENSE](https://raw.githubusercontent.com/lvendrame/fosk/refs/heads/main/license.txt) for details.
