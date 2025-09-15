use std::{collections::HashMap, ffi::OsString, fs, io::BufWriter, sync::RwLock};
use serde_json::{Map, Value};

use crate::{database::{ColumnValue, DbConfig, ExpansionType, IdManager, IdType, IdValue, SchemaDict}, Db};

/// Thread-safe handle to an in-memory collection protected by a RwLock.
pub(crate) type MemoryCollection = RwLock<InternalMemoryCollection>;

/// Internal in-memory collection representation.
///
/// Stores items keyed by string ids and maintains an `IdManager` plus an
/// optional inferred `SchemaDict` for the collection.
pub(crate) struct InternalMemoryCollection {
    collection: HashMap<String, Value>,
    id_manager: IdManager,
    config: DbConfig,
    /// collection name
    pub name: String,
    /// optional inferred schema for the collection
    pub schema: Option<SchemaDict>,
}

impl InternalMemoryCollection {
    pub fn new(name: &str, config: DbConfig) -> Self {
        let collection: HashMap<String, Value> = HashMap::new();
        let id_manager = IdManager::new(config.id_type);
        Self {
            collection,
            id_manager,
            config,
            name: name.to_ascii_lowercase(),
            schema: None,
        }
    }

    pub fn into_protected(self) -> MemoryCollection {
        RwLock::new(self)
    }

    pub fn schema(&self) -> Option<SchemaDict> {
        self.schema.as_ref().cloned()
    }

    pub fn get_reference_column_name(&self) -> String {
        let name = if self.name.ends_with("s") {
            self.name[..self.name.len()-1].to_string()
        } else {
            self.name.to_string()
        };

        format!("{}_{}", name, self.config.id_key)
    }

    pub fn ensure_update_schema_for_item(&mut self, item: &Value) {
        if let Value::Object(map) = item {
            if self.schema.is_none() {
                self.schema = Some(SchemaDict::infer_schema_from_object(map));
            } else if let Some(schema) = &mut self.schema {
                schema.merge_schema(map);
            }
        }
    }

    pub fn merge_json_values(mut base: Value, update: Value) -> Value {
        match (&mut base, update) {
            (Value::Object(base_map), Value::Object(update_map)) => {
                // Merge object fields
                for (key, value) in update_map {
                    if base_map.contains_key(&key) {
                        // Recursively merge nested objects
                        let existing_value = base_map.get(&key).unwrap().clone();
                        base_map.insert(key, Self::merge_json_values(existing_value, value));
                    } else {
                        // Add new field
                        base_map.insert(key, value);
                    }
                }
                base
            }
            // For non-object values, replace entirely
            (_, update_value) => update_value,
        }
    }

    pub fn new_coll(name: &str, config: DbConfig) -> Self {
        Self::new(name, config)
    }

    pub fn get_all(&self) -> Vec<Value> {
        self.collection.values().cloned().collect::<Vec<Value>>()
    }

    pub fn get_paginated(&self, offset: usize, limit: usize) -> Vec<Value> {
        self.collection.values()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect::<Vec<Value>>()
    }

    pub fn get(&self, id: &str) -> Option<Value> {
        self.collection.get(id).cloned()
    }

    pub fn get_filtered_by_columns_values(&self, columns_values: Vec<ColumnValue>, expansion_type: ExpansionType, db: &Db) -> Vec<Value> {
        self.collection.values().filter_map(|row| {
            match row {
                Value::Object(map) => {
                    for column_value in &columns_values {
                        match map.get(&column_value.column) {
                            Some(value) => if *value != column_value.value  {
                                return None;
                            },
                            None => return None,
                        }
                    }

                    let expanded = self.expand_row(row, expansion_type.clone(), db);
                    Some(expanded)
                },
                _ => None,
            }
        })
        .collect::<Vec<Value>>()
    }

    fn expand_object(&self, object: Map<String, Value>, collection_name: String, next_expansion_type: ExpansionType, db: &Db) -> Value {
        let refs = db.get_collection_refs(&self.name);
        let mut object = object.clone();

        match refs {
            Some(refs) => {
                for entry in refs.values() {
                    if entry.ref_collection.eq_ignore_ascii_case(&collection_name) {
                        if let Some(cell) = object.get(&entry.column) {
                            if let Some(collection) = db.get(&entry.ref_collection) {
                                let cvs = vec![ColumnValue::new(entry.ref_column.clone(), cell.clone())];
                                let expanded = collection.get_filtered_by_columns_values(cvs, next_expansion_type.clone(), db);
                                let mut  key = collection.get_name();
                                if !key.ends_with("s") {
                                    key.push('s');
                                }
                                object.insert(key, Value::Array(expanded));
                            }
                        }
                    }
                }
                Value::Object(object)
            },
            None => Value::Object(object),
        }
    }

    pub fn expand_row(&self, row: &Value, expansion_type: ExpansionType, db: &Db) -> Value {
        match (row.clone(), expansion_type) {
            (Value::Object(map), ExpansionType::Single(collection_name)) =>
                self.expand_object(map, collection_name, ExpansionType::None, db),
            (Value::Object(map), ExpansionType::Child(collection_name, expansion_type)) =>
                self.expand_object(map, collection_name, expansion_type.as_ref().clone(), db),
            _ => row.clone(),
        }
    }

    pub fn expand_list(&self, list: Vec<Value>, expansion_type: ExpansionType, db: &Db) -> Vec<Value> {
        list.iter().map(|row| self.expand_row(row, expansion_type.clone(), db)).collect()
    }

    pub fn exists(&self, id: &str) -> bool {
        self.collection.contains_key(id)
    }

    pub fn count(&self) -> usize {
        self.collection.len()
    }

    pub fn add(&mut self, item: Value) -> Option<Value> {
        let next_id = {
            self.id_manager.next()
        };

        let mut item = item;
        let id_string = if let Some(id_value) = next_id {
            // Convert IdValue to string and add it to the item
            let id_string = id_value.to_string();

            // Add the ID to the item using the configured id_key
            if let Value::Object(ref mut map) = item {
                map.insert(self.config.id_key.clone(), Value::String(id_string.clone()));
            }
            Some(id_string)
        } else if let Some(Value::String(id_string)) = item.get(self.config.id_key.clone()){
            Some(id_string.clone())
        } else if let Some(Value::Number(id_number)) = item.get(self.config.id_key.clone()){
            Some(id_number.to_string())
        }else {
            None
        };

        if let Some(id_string) = id_string {
            self.ensure_update_schema_for_item(&item);

            self.collection.insert(id_string, item.clone());

            return Some(item);
        }

        None
    }

    pub fn add_batch(&mut self, items: Value) -> Vec<Value> {
        let mut added_items = Vec::new();

        if let Value::Array(items_array) = items {
            let mut max_id = None;
            for item in items_array {
                if let Value::Object(ref item_map) = item {
                    self.ensure_update_schema_for_item(&item);

                    let id = item_map.get(&self.config.id_key);
                    let id = match self.id_manager.id_type {
                        IdType::Uuid => match id {
                            Some(Value::String(id)) => Some(id.clone()),
                            _ => None
                        },
                        IdType::Int => match id {
                            Some(Value::Number(id)) => {
                                if let Some(current) = max_id {
                                    let id = id.as_u64().unwrap();
                                    if current < id {
                                        max_id = Some(id);
                                    }
                                } else {
                                    max_id = id.as_u64();
                                }
                                Some(id.to_string())
                            },
                            _ => None
                        },
                        IdType::None => match item.get(self.config.id_key.clone()) {
                            Some(Value::String(id_string)) => Some(id_string.clone()),
                            Some(Value::Number(id_number)) => Some(id_number.to_string()),
                            _ => None
                        }
                    };

                    // Extract the ID from the item using the configured id_key
                    if let Some(id) = id {
                        // Insert the item with its existing ID
                        self.collection.insert(id.clone(), item.clone());
                        added_items.push(item);
                    }
                    // Skip items that don't have the required ID field
                }
                // Skip non-object items
            }

            // update the id_manager with the max id for an integer id
            if let Some(value) = max_id {
                if self.id_manager.set_current(IdValue::Int(value)).is_err() {
                    println!("Error to set the value {} to {} collection Id", value, self.name.clone());
                }
            }
        }

        added_items
    }

    pub fn update(&mut self, id: &str, item: Value) -> Option<Value> {
        let mut item = item;

        // Add the ID to the item using the configured id_key
        if let Value::Object(ref mut map) = item {
            map.insert(self.config.id_key.clone(), Value::String(id.to_string()));
        }

        if self.collection.contains_key(id) {
            self.ensure_update_schema_for_item(&item);
            self.collection.insert(id.to_string(), item.clone());
            Some(item)
        } else {
            None
        }
    }

    pub fn update_partial(&mut self, id: &str, partial_item: Value) -> Option<Value> {
        if let Some(existing_item) = self.collection.get(id).cloned() {
            // Merge the partial update with the existing item
            let updated_item = Self::merge_json_values(existing_item, partial_item);

            // Ensure the ID is still present in the updated item
            let mut final_item = updated_item;
            if let Value::Object(ref mut map) = final_item {
                map.insert(self.config.id_key.clone(), Value::String(id.to_string()));
            }

            self.ensure_update_schema_for_item(&final_item);

            // Update the item in the database
            self.collection.insert(id.to_string(), final_item.clone());
            Some(final_item)
        } else {
            None
        }
    }

    pub fn delete(&mut self, id: &str) -> Option<Value> {
        self.collection.remove(id)
    }

    pub fn clear(&mut self) -> usize {
        let count = self.collection.len();
        self.collection.clear();
        count
    }

    pub fn load_from_json(&mut self, json_value: Value, keep: bool) -> Result<Vec<Value>, String> {
        // Guard: Check if it's a JSON Array
        let Value::Array(_) = json_value else {
            return Err("⚠️ Informed JSON does not contain a JSON array in the root, skipping initial data load".to_string());
        };

        if !keep {
            self.clear();
        }

        // Load the array into the collection using add_batch
        let added_items = self.add_batch(json_value);
        Ok(added_items)
    }

    pub fn load_from_file(&mut self, file_path: &OsString) -> Result<String, String> {
        let file_path_lossy = file_path.to_string_lossy();

        // Guard: Try to read the file content
        let file_content = fs::read_to_string(file_path)
            .map_err(|_| format!("⚠️ Could not read file {}, skipping initial data load", file_path_lossy))?;

        // Guard: Try to parse the content as JSON
        let json_value = serde_json::from_str::<Value>(&file_content)
            .map_err(|_| format!("⚠️ File {} does not contain valid JSON, skipping initial data load", file_path_lossy))?;

        match self.load_from_json(json_value, false) {
            Ok(added_items) => Ok(format!("✔️ Loaded {} initial items from {}", added_items.len(), file_path_lossy)),
            Err(error) => Err(format!("Error to process the file {}. Details: {}", file_path_lossy, error)),
        }
    }
}

/// Thread-safe, user-facing handle to a collection.
///
/// `DbCollection` wraps an internal in-memory collection protected by a
/// `RwLock` (`MemoryCollection`) and exposes convenient, high-level methods
/// for common operations: retrieval (`get`, `get_all`, `get_paginated`),
/// mutation (`add`, `add_batch`, `update`, `update_partial`, `delete`,
/// `clear`), and bulk loads (`load_from_json`, `load_from_file`).
///
/// Prefer using the provided methods which acquire locks internally. If you
/// need lower-level access to the underlying collection, the `collection`
/// field is public so callers can acquire a read or write lock directly with
/// `.read()` / `.write()`, but doing so bypasses the convenience methods and
/// should be done with care.
pub struct DbCollection {
    /// The internal protected memory collection. Callers may acquire a lock
    /// directly (e.g. `dbcoll.collection.read().unwrap()`) but prefer the
    /// high-level methods on `DbCollection` when possible.
    pub(crate) collection: MemoryCollection,
}

impl DbCollection {
    /// Create a new `DbCollection` backed by an internal in-memory collection.
    ///
    /// `name` is the collection name and `config` controls id strategy and key.
    pub fn new_coll(name: &str, config: DbConfig) -> Self {
        Self {
            collection: InternalMemoryCollection::new_coll(name, config).into_protected()
        }
    }

    /// Return a default name to reference this collection id key
    pub fn get_reference_column_name(&self) -> String {
        self.collection.read().unwrap().get_reference_column_name()
    }

    /// Return all documents in the collection as a `Vec<Value>`.
    ///
    /// This clones stored JSON values and is intended for small collections or
    /// tests; prefer paginated access for large datasets.
    pub fn get_all(&self) -> Vec<Value> {
        self.collection.read().unwrap().get_all()
    }

    /// Return a page of documents starting at `offset` with at most `limit`
    /// items.
    pub fn get_paginated(&self, offset: usize, limit: usize) -> Vec<Value> {
        self.collection.read().unwrap().get_paginated(offset, limit)
    }

    pub(crate) fn get_filtered_by_columns_values(&self, columns_values: Vec<ColumnValue>, expansion_type: ExpansionType, db: &Db) -> Vec<Value> {
        self.collection.read().unwrap().get_filtered_by_columns_values(columns_values, expansion_type, db)
    }

    /// Retrieve a single document by its string id.
    pub fn get(&self, id: &str) -> Option<Value> {
        self.collection.read().unwrap().get(id)
    }

    /// Check whether a document with `id` exists in the collection.
    pub fn exists(&self, id: &str) -> bool {
        self.collection.read().unwrap().exists(id)
    }

    /// Return the number of documents currently stored in the collection.
    pub fn count(&self) -> usize {
        self.collection.read().unwrap().count()
    }

    /// Add a document to the collection.
    ///
    /// Depending on the configured `id_type`, the collection may generate an
    /// id and insert it into the document. Returns the stored document on
    /// success (with id populated) or `None` if the item could not be added
    /// (for example when ids are required but missing).
    pub fn add(&self, item: Value) -> Option<Value> {
        self.collection.write().unwrap().add(item)
    }

    /// Add multiple items from a JSON array value; returns the subset of
    /// items that were actually added.
    pub fn add_batch(&self, items: Value) -> Vec<Value> {
        self.collection.write().unwrap().add_batch(items)
    }

    /// Replace the document with id `id` with `item`. Returns the stored
    /// document on success or `None` if the id was not present.
    pub fn update(&self, id: &str, item: Value) -> Option<Value> {
        self.collection.write().unwrap().update(id, item)
    }

    /// Apply a partial update to the document with `id` by merging JSON
    /// values; returns the updated document or `None` if the id is not found.
    pub fn update_partial(&self, id: &str, partial_item: Value) -> Option<Value> {
        self.collection.write().unwrap().update_partial(id, partial_item)
    }

    /// Remove and return the document with `id` if it exists.
    pub fn delete(&self, id: &str) -> Option<Value> {
        self.collection.write().unwrap().delete(id)
    }

    /// Remove all documents and return the number of removed items.
    pub fn clear(&self) -> usize {
        self.collection.write().unwrap().clear()
    }

    /// Load documents from a serde_json `Value` (must be an array) and return
    /// the list of items actually added. Errors if the value is not an array.
    pub fn load_from_json(&self, json_value: Value, keep: bool) -> Result<Vec<Value>, String> {
        self.collection.write().unwrap().load_from_json(json_value, keep)
    }

    /// Load documents from a file path. Returns a human-readable status on
    /// success or an error string on failure.
    pub fn load_from_file(&self, file_path: &OsString) -> Result<String, String> {
        self.collection.write().unwrap().load_from_file(file_path)
    }

    /// Save collection to a file path.
    pub fn write_to_file(&self, file_path: &OsString) -> Result<(), String> {
        let file = std::fs::File::create(file_path).expect("Failed to create json file");
        let mut w = BufWriter::new(file);

        let data = self.get_all();
        serde_json::to_writer_pretty(&mut w, &data).expect("Failed to write to a json file");
        Ok(())
    }

    pub fn expand_row(&self, row: &Value, expansion: &str, db: &Db) -> Value {
        self.collection.read().unwrap().expand_row(row, ExpansionType::from(expansion), db)
    }

    pub fn expand_list(&self, list: Vec<Value>, expansion: &str, db: &Db) -> Vec<Value> {
        self.collection.read().unwrap().expand_list(list, ExpansionType::from(expansion), db)
    }

    /// Return the optionally-inferred `SchemaDict` for this collection (if
    /// any documents have been added that allowed schema inference).
    pub fn schema(&self) -> Option<SchemaDict> {
        self.collection.read().ok().and_then(|g| g.schema())
    }

    // Get the collection name
    pub fn get_name(&self) -> String {
        self.collection.read().unwrap().name.clone()
    }

    // Get the collection DBConfig
    pub fn get_config(&self) -> DbConfig {
        self.collection.read().unwrap().config.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_collection() -> InternalMemoryCollection {
        InternalMemoryCollection::new("test_collection", DbConfig::int("id"))
    }

    fn create_uuid_collection() -> InternalMemoryCollection {
        InternalMemoryCollection::new("uuid_collection", DbConfig::uuid("id"))
    }

    fn create_none_collection() -> InternalMemoryCollection {
        InternalMemoryCollection::new("none_collection", DbConfig::none("id"))
    }

    #[test]
    fn test_new_collection() {
        let collection = create_test_collection();
        assert_eq!(collection.count(), 0);
        assert_eq!(collection.config.id_key, "id");
        assert_eq!(collection.name, "test_collection");
    }

    #[test]
    fn test_into_protected() {
        let collection = create_test_collection();
        let protected = collection.into_protected();

        let guard = protected.read().unwrap();
        assert_eq!(guard.count(), 0);
        assert_eq!(guard.name, "test_collection");
    }

    #[test]
    fn test_get_all_empty() {
        let collection = create_test_collection();
        let all_items = collection.get_all();
        assert!(all_items.is_empty());
    }

    #[test]
    fn test_get_all_with_items() {
        let mut collection = create_test_collection();

        // Add some items
        collection.add(json!({"name": "Item 1"}));
        collection.add(json!({"name": "Item 2"}));
        collection.add(json!({"name": "Item 3"}));

        let all_items = collection.get_all();
        assert_eq!(all_items.len(), 3);

        // Check that all items have IDs assigned
        for item in &all_items {
            assert!(item.get("id").is_some());
            assert!(item.get("name").is_some());
        }
    }

    #[test]
    fn test_get_existing_item() {
        let mut collection = create_test_collection();
        let item = collection.add(json!({"name": "Test Item"})).unwrap();
        let id = item.get("id").unwrap().as_str().unwrap();

        let retrieved = collection.get(id);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().get("name").unwrap(), "Test Item");
    }

    #[test]
    fn test_get_nonexistent_item() {
        let collection = create_test_collection();
        let retrieved = collection.get("999");
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_get_paginated_empty() {
        let collection = create_test_collection();
        let paginated = collection.get_paginated(0, 10);
        assert!(paginated.is_empty());
    }

    #[test]
    fn test_get_paginated_with_items() {
        let mut collection = create_test_collection();

        // Add 10 items
        for i in 1..=10 {
            collection.add(json!({"name": format!("Item {}", i)}));
        }

        // Test first page
        let first_page = collection.get_paginated(0, 3);
        assert_eq!(first_page.len(), 3);

        // Test second page
        let second_page = collection.get_paginated(3, 3);
        assert_eq!(second_page.len(), 3);

        // Test last page (partial)
        let last_page = collection.get_paginated(9, 5);
        assert_eq!(last_page.len(), 1);

        // Test beyond range
        let empty_page = collection.get_paginated(15, 5);
        assert!(empty_page.is_empty());
    }

    #[test]
    fn test_exists() {
        let mut collection = create_test_collection();
        let item = collection.add(json!({"name": "Test Item"})).unwrap();
        let id = item.get("id").unwrap().as_str().unwrap();

        assert!(collection.exists(id));
        assert!(!collection.exists("999"));
    }

    #[test]
    fn test_count() {
        let mut collection = create_test_collection();
        assert_eq!(collection.count(), 0);

        collection.add(json!({"name": "Item 1"}));
        assert_eq!(collection.count(), 1);

        collection.add(json!({"name": "Item 2"}));
        assert_eq!(collection.count(), 2);

        // Delete one
        let all_items = collection.get_all();
        let id = all_items[0].get("id").unwrap().as_str().unwrap();
        collection.delete(id);
        assert_eq!(collection.count(), 1);
    }

    #[test]
    fn test_add_with_int_id() {
        let mut collection = create_test_collection();

        let item = collection.add(json!({"name": "Test Item"}));
        assert!(item.is_some());

        let item = item.unwrap();
        assert_eq!(item.get("name").unwrap(), "Test Item");
        assert_eq!(item.get("id").unwrap(), "1");

        // Add another item
        let item2 = collection.add(json!({"name": "Test Item 2"})).unwrap();
        assert_eq!(item2.get("id").unwrap(), "2");
    }

    #[test]
    fn test_add_with_uuid_id() {
        let mut collection = create_uuid_collection();

        let item = collection.add(json!({"name": "Test Item"}));
        assert!(item.is_some());

        let item = item.unwrap();
        assert_eq!(item.get("name").unwrap(), "Test Item");
        let id = item.get("id").unwrap().as_str().unwrap();
        assert!(!id.is_empty());
        assert!(id.len() > 10); // UUIDs are longer than 10 characters
    }

    #[test]
    fn test_add_with_none_id_existing() {
        let mut collection = create_none_collection();

        let item = collection.add(json!({"id": "custom-id", "name": "Test Item"}));
        assert!(item.is_some());

        let item = item.unwrap();
        assert_eq!(item.get("name").unwrap(), "Test Item");
        assert_eq!(item.get("id").unwrap(), "custom-id");
    }

    #[test]
    fn test_add_with_none_id_number_existing() {
        let mut collection = create_none_collection();

        let item = collection.add(json!({"id": 1, "name": "Test Item"}));
        assert!(item.is_some());

        let item = item.unwrap();
        assert_eq!(item.get("name").unwrap(), "Test Item");
        assert_eq!(item.get("id").unwrap(), 1);
    }

    #[test]
    fn test_add_with_none_id_missing() {
        let mut collection = create_none_collection();

        let item = collection.add(json!({"name": "Test Item"}));
        assert!(item.is_none());
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_add_batch_int() {
        let mut collection = create_test_collection();

        let batch = json!([
            {"name": "Item 1"},
            {"id": 5, "name": "Item 2"},
            {"id": 3, "name": "Item 3"},
            {"id": 10, "name": "Item 4"}
        ]);

        let added_items = collection.add_batch(batch);
        assert_eq!(added_items.len(), 3); // Only items with IDs should be added
        assert_eq!(collection.count(), 3);

        // Check that the max ID was set correctly
        let new_item = collection.add(json!({"name": "New Item"})).unwrap();
        assert_eq!(new_item.get("id").unwrap(), "11"); // Should be max + 1
    }

    #[test]
    fn test_add_batch_uuid() {
        let mut collection = create_uuid_collection();

        let batch = json!([
            {"id": "uuid-1", "name": "Item 1"},
            {"id": "uuid-2", "name": "Item 2"},
            {"name": "Item 3"} // This should be skipped
        ]);

        let added_items = collection.add_batch(batch);
        assert_eq!(added_items.len(), 2);
        assert_eq!(collection.count(), 2);
    }

    #[test]
    fn test_add_batch_none() {
        let mut collection = create_none_collection();

        let batch = json!([
            {"id": "custom-1", "name": "Item 1"},
            {"id": "custom-2", "name": "Item 2"},
            {"name": "Item 3"}, // This should be skipped
            {"id": 3, "name": "Item 4"},
        ]);

        let added_items = collection.add_batch(batch);
        assert_eq!(added_items.len(), 3);
        assert_eq!(collection.count(), 3);
    }

    #[test]
    fn test_add_batch_non_array() {
        let mut collection = create_test_collection();

        let non_array = json!({"name": "Single Item"});
        let added_items = collection.add_batch(non_array);
        assert!(added_items.is_empty());
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_update_existing_item() {
        let mut collection = create_test_collection();
        let item = collection.add(json!({"name": "Original Name"})).unwrap();
        let id = item.get("id").unwrap().as_str().unwrap();

        let updated = collection.update(id, json!({"name": "Updated Name", "description": "New field"}));
        assert!(updated.is_some());

        let updated_item = updated.unwrap();
        assert_eq!(updated_item.get("name").unwrap(), "Updated Name");
        assert_eq!(updated_item.get("description").unwrap(), "New field");
        assert_eq!(updated_item.get("id").unwrap(), id);

        // Verify it's actually updated in the collection
        let retrieved = collection.get(id).unwrap();
        assert_eq!(retrieved.get("name").unwrap(), "Updated Name");
    }

    #[test]
    fn test_update_nonexistent_item() {
        let mut collection = create_test_collection();

        let updated = collection.update("999", json!({"name": "Updated Name"}));
        assert!(updated.is_none());
    }

    #[test]
    fn test_update_partial_existing_item() {
        let mut collection = create_test_collection();
        let item = collection.add(json!({
            "name": "Original Name",
            "description": "Original Description",
            "count": 42
        })).unwrap();
        let id = item.get("id").unwrap().as_str().unwrap();

        let updated = collection.update_partial(id, json!({"name": "Updated Name"}));
        assert!(updated.is_some());

        let updated_item = updated.unwrap();
        assert_eq!(updated_item.get("name").unwrap(), "Updated Name");
        assert_eq!(updated_item.get("description").unwrap(), "Original Description"); // Should remain
        assert_eq!(updated_item.get("count").unwrap(), 42); // Should remain
        assert_eq!(updated_item.get("id").unwrap(), id);
    }

    #[test]
    fn test_update_partial_nested_objects() {
        let mut collection = create_test_collection();
        let item = collection.add(json!({
            "name": "Test Item",
            "config": {
                "enabled": true,
                "timeout": 30,
                "nested": {
                    "value": "original"
                }
            }
        })).unwrap();
        let id = item.get("id").unwrap().as_str().unwrap();

        let updated = collection.update_partial(id, json!({
            "config": {
                "timeout": 60,
                "nested": {
                    "value": "updated",
                    "new_field": "added"
                }
            }
        }));

        assert!(updated.is_some());
        let updated_item = updated.unwrap();

        let config = updated_item.get("config").unwrap();
        assert_eq!(config.get("enabled").unwrap(), true); // Should remain
        assert_eq!(config.get("timeout").unwrap(), 60); // Should be updated

        let nested = config.get("nested").unwrap();
        assert_eq!(nested.get("value").unwrap(), "updated");
        assert_eq!(nested.get("new_field").unwrap(), "added");
    }

    #[test]
    fn test_update_partial_nonexistent_item() {
        let mut collection = create_test_collection();

        let updated = collection.update_partial("999", json!({"name": "Updated Name"}));
        assert!(updated.is_none());
    }

    #[test]
    fn test_delete_existing_item() {
        let mut collection = create_test_collection();
        let item = collection.add(json!({"name": "Test Item"})).unwrap();
        let id = item.get("id").unwrap().as_str().unwrap();

        assert_eq!(collection.count(), 1);

        let deleted = collection.delete(id);
        assert!(deleted.is_some());
        assert_eq!(deleted.unwrap().get("name").unwrap(), "Test Item");
        assert_eq!(collection.count(), 0);
        assert!(!collection.exists(id));
    }

    #[test]
    fn test_delete_nonexistent_item() {
        let mut collection = create_test_collection();

        let deleted = collection.delete("999");
        assert!(deleted.is_none());
    }

    #[test]
    fn test_clear_empty_collection() {
        let mut collection = create_test_collection();

        let count = collection.clear();
        assert_eq!(count, 0);
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_clear_with_items() {
        let mut collection = create_test_collection();

        // Add some items
        collection.add(json!({"name": "Item 1"}));
        collection.add(json!({"name": "Item 2"}));
        collection.add(json!({"name": "Item 3"}));

        assert_eq!(collection.count(), 3);

        let count = collection.clear();
        assert_eq!(count, 3);
        assert_eq!(collection.count(), 0);
        assert!(collection.get_all().is_empty());
    }

    #[test]
    fn test_merge_json_values_objects() {
        let base = json!({
            "name": "Original",
            "config": {
                "enabled": true,
                "timeout": 30
            },
            "tags": ["tag1", "tag2"]
        });

        let update = json!({
            "name": "Updated",
            "config": {
                "timeout": 60,
                "new_setting": "value"
            },
            "description": "New field"
        });

        let merged = InternalMemoryCollection::merge_json_values(base, update);

        assert_eq!(merged.get("name").unwrap(), "Updated");
        assert_eq!(merged.get("description").unwrap(), "New field");
        assert_eq!(merged.get("tags").unwrap(), &json!(["tag1", "tag2"])); // Should remain

        let config = merged.get("config").unwrap();
        assert_eq!(config.get("enabled").unwrap(), true); // Should remain
        assert_eq!(config.get("timeout").unwrap(), 60); // Should be updated
        assert_eq!(config.get("new_setting").unwrap(), "value"); // Should be added
    }

    #[test]
    fn test_merge_json_values_non_objects() {
        let base = json!("original");
        let update = json!("updated");

        let merged = InternalMemoryCollection::merge_json_values(base, update);
        assert_eq!(merged, json!("updated"));

        let base = json!(42);
        let update = json!(100);

        let merged = InternalMemoryCollection::merge_json_values(base, update);
        assert_eq!(merged, json!(100));
    }

    #[test]
    fn test_id_manager_integration() {
        let mut collection = create_test_collection();

        // Add items and verify sequential IDs
        let item1 = collection.add(json!({"name": "Item 1"})).unwrap();
        assert_eq!(item1.get("id").unwrap(), "1");

        let item2 = collection.add(json!({"name": "Item 2"})).unwrap();
        assert_eq!(item2.get("id").unwrap(), "2");

        let item3 = collection.add(json!({"name": "Item 3"})).unwrap();
        assert_eq!(item3.get("id").unwrap(), "3");
    }

    #[test]
    fn test_custom_id_key() {
        let mut collection = InternalMemoryCollection::new(
            "custom_collection",
            DbConfig::int("customId")
        );

        let item = collection.add(json!({"name": "Test Item"})).unwrap();
        assert_eq!(item.get("customId").unwrap(), "1");
        assert!(item.get("id").is_none()); // Should not have regular "id" field

        // Test retrieval
        let retrieved = collection.get("1").unwrap();
        assert_eq!(retrieved.get("customId").unwrap(), "1");
    }

    // Tests for load_from_file method
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_load_from_file_valid_json_array() {
        let mut collection = create_test_collection();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test_data.json");

        // Create a test JSON file with valid array data
        let test_data = json!([
            {"id": 1, "name": "Item 1", "description": "First item"},
            {"id": 2, "name": "Item 2", "description": "Second item"},
            {"id": 3, "name": "Item 3", "description": "Third item"}
        ]);

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        // Load data from file
        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Loaded 3 initial items"));
        assert_eq!(collection.count(), 3);

        // Verify the data was loaded correctly
        assert!(collection.exists("1"));
        assert!(collection.exists("2"));
        assert!(collection.exists("3"));

        let item1 = collection.get("1").unwrap();
        assert_eq!(item1.get("name").unwrap(), "Item 1");
        assert_eq!(item1.get("description").unwrap(), "First item");
    }

    #[test]
    fn test_load_from_file_empty_array() {
        let mut collection = create_test_collection();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty_array.json");

        // Create a test JSON file with empty array
        let test_data = json!([]);

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        // Load data from file
        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Loaded 0 initial items"));
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_load_from_file_with_uuid_collection() {
        let mut collection = create_uuid_collection();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("uuid_data.json");

        // Create a test JSON file with UUID data
        let test_data = json!([
            {"id": "uuid-1", "name": "Item 1"},
            {"id": "uuid-2", "name": "Item 2"}
        ]);

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        // Load data from file
        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Loaded 2 initial items"));
        assert_eq!(collection.count(), 2);

        assert!(collection.exists("uuid-1"));
        assert!(collection.exists("uuid-2"));
    }

    #[test]
    fn test_load_from_file_with_mixed_id_types() {
        let mut collection = create_none_collection();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("mixed_data.json");

        // Create a test JSON file with mixed ID types
        let test_data = json!([
            {"id": "string-id", "name": "Item 1"},
            {"id": 42, "name": "Item 2"},
            {"name": "Item 3"} // This should be skipped (no ID)
        ]);

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        // Load data from file
        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Loaded 2 initial items"));
        assert_eq!(collection.count(), 2);

        assert!(collection.exists("string-id"));
        assert!(collection.exists("42"));
    }

    #[test]
    fn test_load_from_file_nonexistent_file() {
        let mut collection = create_test_collection();
        let nonexistent_path = std::ffi::OsString::from("/path/that/does/not/exist.json");

        let result = collection.load_from_file(&nonexistent_path);

        assert!(result.is_err());
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("Could not read file"));
        assert!(error_msg.contains("skipping initial data load"));
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_load_from_file_invalid_json() {
        let mut collection = create_test_collection();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("invalid.json");

        // Create a file with invalid JSON
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"{ invalid json content }").unwrap();

        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_err());
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("does not contain valid JSON"));
        assert!(error_msg.contains("skipping initial data load"));
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_load_from_file_json_object_not_array() {
        let mut collection = create_test_collection();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("object.json");

        // Create a JSON file with an object instead of array
        let test_data = json!({"id": 1, "name": "Single Item"});

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_err());
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("does not contain a JSON array"));
        assert!(error_msg.contains("skipping initial data load"));
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_load_from_file_json_primitive_not_array() {
        let mut collection = create_test_collection();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("primitive.json");

        // Create a JSON file with a primitive value
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"\"just a string\"").unwrap();

        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not contain a JSON array"));
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_load_from_file_updates_id_manager() {
        let mut collection = create_test_collection();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("id_update_test.json");

        // Create data with high ID values
        let test_data = json!([
            {"id": 10, "name": "Item 1"},
            {"id": 15, "name": "Item 2"},
            {"id": 5, "name": "Item 3"}
        ]);

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        // Load data from file
        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());
        assert!(result.is_ok());

        // Add a new item - should get ID 16 (max + 1)
        let new_item = collection.add(json!({"name": "New Item"})).unwrap();
        assert_eq!(new_item.get("id").unwrap(), "16");
    }

    #[test]
    fn test_load_from_file_large_dataset() {
        let mut collection = create_test_collection();
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large_dataset.json");

        // Create a large dataset
        let mut items = Vec::new();
        for i in 1..=1000 {
            items.push(json!({
                "id": i,
                "name": format!("Item {}", i),
                "value": i * 10
            }));
        }
        let test_data = json!(items);

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        // Load data from file
        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Loaded 1000 initial items"));
        assert_eq!(collection.count(), 1000);

        // Verify some random items
        assert!(collection.exists("1"));
        assert!(collection.exists("500"));
        assert!(collection.exists("1000"));

        let item_500 = collection.get("500").unwrap();
        assert_eq!(item_500.get("name").unwrap(), "Item 500");
        assert_eq!(item_500.get("value").unwrap(), 5000);
    }

    #[test]
    fn test_load_from_file_with_existing_data() {
        let mut collection = create_test_collection();

        // Add some existing data
        collection.add(json!({"name": "Existing Item 1"}));
        collection.add(json!({"name": "Existing Item 2"}));
        assert_eq!(collection.count(), 2);

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("additional_data.json");

        // Create additional data
        let test_data = json!([
            {"id": 10, "name": "Loaded Item 1"},
            {"id": 11, "name": "Loaded Item 2"}
        ]);

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        // Load additional data from file
        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Loaded 2 initial items"));
        assert_eq!(collection.count(), 2); // 2 loaded

        // Verify all data exists
        assert!(!collection.exists("1")); // Cleaned
        assert!(!collection.exists("2")); // Cleaned
        assert!(collection.exists("10")); // Loaded
        assert!(collection.exists("11")); // Loaded
    }

    #[test]
    fn test_load_from_file_custom_id_key() {
        let mut collection = InternalMemoryCollection::new(
            "custom_collection",
            DbConfig::int("customId"),
        );

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("custom_id_data.json");

        // Create data with custom ID key
        let test_data = json!([
            {"customId": 1, "name": "Item 1"},
            {"customId": 2, "name": "Item 2"}
        ]);

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        // Load data from file
        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert!(result.is_ok());
        assert!(result.unwrap().contains("Loaded 2 initial items"));
        assert_eq!(collection.count(), 2);

        assert!(collection.exists("1"));
        assert!(collection.exists("2"));

        let item1 = collection.get("1").unwrap();
        assert_eq!(item1.get("customId").unwrap(), 1);
        assert_eq!(item1.get("name").unwrap(), "Item 1");
    }

    #[test]
    fn test_write_to_file() {
        use tempfile::TempDir;
        use std::ffi::OsString;
        use std::fs;
        use serde_json::json;

        // Create a temporary directory and file path
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("output.json");
        let os_file_path = OsString::from(file_path.to_string_lossy().into_owned());

        // Create a DbCollection and add items
        let config = DbConfig::int("id");
        let db_collection = DbCollection::new_coll("test", config);

        let item1 = json!({"name": "Alice"});
        let item2 = json!({"name": "Bob"});
        let stored1 = db_collection.add(item1).unwrap();
        let stored2 = db_collection.add(item2).unwrap();

        // Write to file and assert success
        assert!(db_collection.write_to_file(&os_file_path).is_ok());

        // Read and parse file content
        let content = fs::read_to_string(file_path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();

        // Verify both items are present
        assert_eq!(parsed.len(), 2);
        let ids: Vec<_> = parsed.iter().filter_map(|v| v.get("id")).collect();
        assert!(ids.contains(&stored1.get("id").unwrap()));
        assert!(ids.contains(&stored2.get("id").unwrap()));
    }

    #[test]
    fn test_expand_row_no_refs() {
        use serde_json::json;
        // Setup DB and collection with no references
        let db = Db::new_db_with_config(DbConfig::int("id"));
        let coll = db.create("items");
        // Add an item
        let item = coll.add(json!({"id": 1, "value": "test"})).unwrap();
        // Expand with empty expansion
        let expanded = coll.expand_row(&item, "", &db);
        assert_eq!(expanded, item);
        // Expand with non-existent relation
        let expanded2 = coll.expand_row(&item, "unknown", &db);
        assert_eq!(expanded2, item);
    }

    #[test]
    fn test_expand_list_no_refs() {
        use serde_json::json;
        // Setup DB and collection with no references
        let db = Db::new_db_with_config(DbConfig::int("id"));
        let coll = db.create("items");
        // Add items
        let a = coll.add(json!({"id": 1, "value": 10})).unwrap();
        let b = coll.add(json!({"id": 2, "value": 20})).unwrap();
        let list = vec![a.clone(), b.clone()];
        // Expand list with empty expansion
        let expanded = coll.expand_list(list.clone(), "", &db);
        assert_eq!(expanded, list);
        // Expand list with missing relation
        let expanded2 = coll.expand_list(list.clone(), "none", &db);
        assert_eq!(expanded2, list);
    }

    #[test]
    fn test_expand_row_with_references() {
        use serde_json::json;
        // Build a mutable DB and two collections: authors and books
        let mut db = Db::new_db_with_config(DbConfig::int("id"));
        let authors = db.create("authors");
        // Add authors with explicit IDs
        let a1 = authors.add(json!({"name": "Alice"})).unwrap();
        let a2 = authors.add(json!({"name": "Bob"})).unwrap();

        let books = db.create("books");
        // Link book to author by author_id key
        let b1 = books.add(json!({"title": "Book1", "author_id": a1.get("id").unwrap()})).unwrap();
        // Add second book to ensure multiple entries, unused in this test
        let _ = books.add(json!({"title": "Book2", "author_id": a2.get("id").unwrap()})).unwrap();

        // Create reference from books.author_id to authors.id
        assert!(db.create_reference("books", "author_id", "authors", "id"));

        // Expand book1 row to include its referenced author
        let expanded1 = books.expand_row(&b1, "authors", &db);
        if let Value::Object(map) = expanded1 {
            let arr = map.get("authors").unwrap().as_array().unwrap();
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0].get("name").unwrap(), a1.get("name").unwrap());
        } else {
            panic!("Expected expanded object for book1");
        }
    }

    #[test]
    fn test_expand_list_with_references() {
        use serde_json::json;
        // Build DB, authors and books as before
        let mut db = Db::new_db_with_config(DbConfig::int("id"));
        let authors = db.create("authors");
        let a1 = authors.add(json!({"name": "Alice"})).unwrap();
        let a2 = authors.add(json!({"name": "Bob"})).unwrap();

        let books = db.create("books");
        let b1 = books.add(json!({"title": "Book1", "author_id": a1.get("id").unwrap()})).unwrap();
        let b2 = books.add(json!({"title": "Book2", "author_id": a2.get("id").unwrap()})).unwrap();

        assert!(db.create_reference("books", "author_id", "authors", "id"));

        // Expand list of books
        let list = vec![b1.clone(), b2.clone()];
        let expanded_list = books.expand_list(list.clone(), "authors", &db);
        // Each expanded item should contain its correct author
        for (orig, exp) in list.iter().zip(expanded_list.iter()) {
            if let Value::Object(map) = exp {
                let arr = map.get("authors").unwrap().as_array().unwrap();
                assert_eq!(arr.len(), 1);
                // Check that the referenced author matches original's author_id
                let author_id = orig.get("author_id").unwrap();
                assert_eq!(arr[0].get("id").unwrap(), author_id);
            } else {
                panic!("Expected expanded object in list");
            }
        }
    }
}
