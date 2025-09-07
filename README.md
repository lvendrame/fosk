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
- ORDER BY, LIMIT, OFFSET
- Parameterized queries (? placeholders, including arrays)
- Test-friendly: create databases on the fly and seed them

---

## Installation

In your `Cargo.toml`:

```toml
[dependencies]
fosk = "0.1.2"
serde_json = "1"
```

---

## Quick example

```rust
use fosk::{Db, DbConfig, IdType};
use serde_json::json;

fn main() {
    let db = Db::new_db_with_config(DbConfig {
        id_type: IdType::Int,       // auto-increment IDs
        id_key: "id".into(),
    });

    let people = db.create_collection("People");

    // Add JSON
    let added = people.add(json!({"name": "Alice", "age": 30})).unwrap();

    // Retrieve auto-generated ID (if configured)
    let id = added.get("id").unwrap().to_string();
    println!("Added person with ID: {}", id);

    // Add multiple documents
    let added_many = people.add_batch(json!([
        {"name": "Bob", "age": 25},
        {"name": "Carol", "age": 28}
    ]));
    println!("Added many: {:?}", added_many);

    // Query with parameters
    let rows = db.query_with_args(
        "SELECT id, name, age FROM People WHERE id = ?",
        json!(id),
    ).unwrap();

    println!("Query result: {rows:?}");
}
```

---

## 📚 Main Types & API

### Db

Represents a database.

- new_db_with_config(config: DbConfig) -> Db
- create_collection(name: &str) -> CollectionHandle
- query(sql: &str) -> Result<Vec<Value>, AnalyzerError>
- query_with_args(sql: &str, args: Value) -> Result<Vec<Value>, AnalyzerError>
- drop_collection(col_name: &str) -> bool
- clear()
- get_config() -> DbConfig

### Config

Defines collection behavior.

- DbConfig::int("id") → auto-increment integer IDs
- DbConfig::uuid("id") → UUID v4 IDs
- DbConfig::none("id") → no automatic ID field

### CollectionHandle

- add(item: Value) -> Option<Value>
- get_all() -> Vec<Value>
- get(id: &str) -> Option<Value>
- update(id: &str, item: Value) -> Option<Value>
- delete(id: &str) -> Option<Value>

#### extras

- get_paginated(offset: usize, limit: usize) -> Vec<Value>
- exists(id: &str) -> bool
- count() -> usize
- add_batch(items: Value) -> Vec<Value>
- update_partial(id: &str, partial_item: Value) -> Option<Value>
- clear() -> usize
- get_config() -> DbConfig

### Load from existing data

#### fn Db::load_from_file(path: &std::path::Path) -> Result<String, String>;

```rust
let db = Db::new();
// Load all collections from a file
db.load_from_file("./collection.json");
```

#### fn Db::load_from_json(json_value: &serde_json::Value) -> Result<usize, String>

```rust
let db = Db::new();
// Load all collections from JSON
db.load_from_json(json!({
    "people": [...],
    "companies": [...],
    "products": [...],
}));
```

#### fn DbCollection::load_from_file(path: &std::path::Path) -> Result<String, String>;

```rust
let db = Db::new();
let people = db.create_collection("People");
// Load collection from a file
people.load_from_file("./people.json");
```

#### fn DbCollection::load_from_json(json_value: &serde_json::Value) -> Result<String, String>

```rust
let db = Db::new();
let people = db.create_collection("People");
// Load collection from JSON
people.load_from_json(json!([
    { "id": 1,  "full_name": "Alice Johnson",    "age": 29, "city": "Porto",    "vip": true  },
    { "id": 2,  "full_name": "Bruno Martins",    "age": 34, "city": "Lisboa",   "vip": false },
    { "id": 3,  "full_name": "Carla Sousa",      "age": 41, "city": "Braga",    "vip": false },
    { "id": 4,  "full_name": "David Pereira",    "age": 25, "city": "Coimbra",  "vip": true  }
]));
```

### Save data

#### fn Db::fn write_to_json(&self) -> Value

```rust
let db = Db::new();
...
...
...
let json_value = db.write_to_json();
```

#### fn DbCollection::write_to_file(&self, file_path: &OsString) -> Result<(), String>

```rust
let db = Db::new();
...
...
...
db.write_to_file("./collections.json");
```

#### fn DbCollection::write_to_file(path: &std::path::Path) -> Result<(), String>

```rust
let db = Db::new();
let people = db.create_collection("People");
...
...
people.write_to_file("./people.json");
```

---

## 🧪 Testing & Seeding

Example test seed (see [fixtures::seed_db](https://github.com/lvendrame/fosk/blob/main/src/executor/_tests.rs#L252)):

```rust
pub fn seed_db() -> Db {
    let db = Db::new_db_with_config(DbConfig {
        id_type: IdType::None,
        id_key: "id".into(),
    });

    create_people(&db);
    create_products(&db);
    create_orders(&db);
    create_order_items(&db);

    db
}
```

---

## ⚠️ Notes

- Projections normally output unqualified field names (id, name), unless duplicates exist.
  In case of conflicts, names are disambiguated with their collection prefix (id, o.id).

---

## 📄 License

Licensed under the _MIT License_.
See [LICENSE](https://raw.githubusercontent.com/lvendrame/fosk/refs/heads/main/license.txt) for details.
