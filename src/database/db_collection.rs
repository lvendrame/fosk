use serde_json::{Map, Number, Value};
use std::{
    collections::HashMap,
    error::Error,
    ffi::OsString,
    fmt::{self, Display},
    fs,
    io::{BufWriter, Write},
    sync::RwLock,
};

use crate::{
    Db, FieldInfo, JsonPrimitive,
    database::{
        ColumnValue, DbConfig, ExpansionChain, IdManager, IdType, IdValue, SchemaDict,
        apply_schema_to_collection, parse_schema_for_load, read_schema_json_file,
    },
};

/// Thread-safe handle to an in-memory collection protected by a RwLock.
pub(crate) type MemoryCollection = RwLock<InternalMemoryCollection>;

/// Error returned when a collection read lock cannot be acquired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionReadError {
    /// The collection lock was poisoned by a panic while held for writing.
    LockPoisoned,
}

impl Display for CollectionReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned => f.write_str("collection read lock is poisoned"),
        }
    }
}

impl Error for CollectionReadError {}

/// Error returned when a collection write lock cannot be acquired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionWriteError {
    /// The collection lock was poisoned by a panic while held.
    LockPoisoned,
}

impl Display for CollectionWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned => f.write_str("collection write lock is poisoned"),
        }
    }
}

impl Error for CollectionWriteError {}

/// Error returned when inserting one item into a collection fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddError {
    /// The collection write lock could not be acquired.
    LockPoisoned,
    /// The item is not a JSON object.
    NonObjectItem,
    /// The collection requires callers to provide an id, but none was present.
    MissingId {
        /// Configured id field that was required but absent.
        id_key: String,
    },
    /// A provided id already exists in a collection with caller-managed ids.
    DuplicateId {
        /// Duplicate id value.
        id: String,
    },
}

impl Display for AddError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned => f.write_str("collection write lock is poisoned"),
            Self::NonObjectItem => f.write_str("collection items must be JSON objects"),
            Self::MissingId { id_key } => write!(f, "missing required id field '{id_key}'"),
            Self::DuplicateId { id } => write!(f, "duplicate collection id '{id}'"),
        }
    }
}

impl Error for AddError {}

/// Error returned when inserting a JSON batch fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddBatchError {
    /// The collection write lock could not be acquired.
    LockPoisoned,
    /// The batch root value is not a JSON array.
    NonArrayInput,
    /// One item in the batch is not a JSON object.
    NonObjectItem {
        /// Zero-based index of the invalid batch item.
        index: usize,
    },
    /// One item in a caller-managed-id collection has no usable id.
    MissingId {
        /// Zero-based index of the invalid batch item.
        index: usize,
        /// Configured id field that was required but absent.
        id_key: String,
    },
    /// One item in a caller-managed-id collection duplicates an existing id.
    DuplicateId {
        /// Zero-based index of the invalid batch item.
        index: usize,
        /// Duplicate id value.
        id: String,
    },
    /// One item has an integer id that is not representable as `u64`.
    InvalidIntId {
        /// Zero-based index of the invalid batch item.
        index: usize,
    },
}

impl Display for AddBatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned => f.write_str("collection write lock is poisoned"),
            Self::NonArrayInput => f.write_str("batch input must be a JSON array"),
            Self::NonObjectItem { index } => {
                write!(f, "batch item at index {index} must be a JSON object")
            }
            Self::MissingId { index, id_key } => {
                write!(
                    f,
                    "batch item at index {index} is missing required id field '{id_key}'"
                )
            }
            Self::DuplicateId { index, id } => {
                write!(
                    f,
                    "batch item at index {index} duplicates collection id '{id}'"
                )
            }
            Self::InvalidIntId { index } => {
                write!(f, "batch item at index {index} has an invalid integer id")
            }
        }
    }
}

impl Error for AddBatchError {}

/// Error returned when loading collection data from JSON or a file fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadCollectionError {
    /// The collection write lock could not be acquired.
    LockPoisoned,
    /// The JSON root was not an array.
    NonArrayInput,
    /// A file could not be read.
    FileRead {
        /// File path that could not be read.
        path: String,
    },
    /// A file did not contain valid JSON.
    InvalidJson {
        /// File path that did not parse as JSON.
        path: String,
    },
    /// A row in the loaded batch was invalid.
    Batch(AddBatchError),
}

impl Display for LoadCollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned => f.write_str("collection write lock is poisoned"),
            Self::NonArrayInput => f.write_str("loaded JSON must contain an array at the root"),
            Self::FileRead { path } => write!(f, "could not read file {path}"),
            Self::InvalidJson { path } => write!(f, "file {path} does not contain valid JSON"),
            Self::Batch(error) => Display::fmt(error, f),
        }
    }
}

impl Error for LoadCollectionError {}

impl From<AddBatchError> for LoadCollectionError {
    fn from(value: AddBatchError) -> Self {
        match value {
            AddBatchError::LockPoisoned => Self::LockPoisoned,
            AddBatchError::NonArrayInput => Self::NonArrayInput,
            other => Self::Batch(other),
        }
    }
}

/// Error returned when writing collection data to a file fails.
#[derive(Debug)]
pub enum WriteCollectionError {
    /// The collection read lock could not be acquired.
    LockPoisoned,
    /// The output file could not be created.
    FileCreate {
        /// Destination path.
        path: String,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// The collection could not be serialized as JSON.
    Serialize {
        /// Underlying serialization error.
        source: serde_json::Error,
    },
}

impl Display for WriteCollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LockPoisoned => f.write_str("collection read lock is poisoned"),
            Self::FileCreate { path, source } => {
                write!(f, "failed to create json file {path}: {source}")
            }
            Self::Serialize { source } => write!(f, "failed to write json file: {source}"),
        }
    }
}

impl Error for WriteCollectionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::FileCreate { source, .. } => Some(source),
            Self::Serialize { source } => Some(source),
            Self::LockPoisoned => None,
        }
    }
}

fn write_collection_data_to_writer<W: Write>(
    writer: &mut W,
    data: &[Value],
) -> Result<(), WriteCollectionError> {
    serde_json::to_writer_pretty(writer, data)
        .map_err(|source| WriteCollectionError::Serialize { source })
}

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

        let mut schema = SchemaDict::default();
        schema.fields.insert(
            config.id_key.clone(),
            FieldInfo {
                nullable: false,
                ty: match config.id_type {
                    IdType::Int => JsonPrimitive::Int,
                    _ => JsonPrimitive::String,
                },
            },
        );
        let schema = Some(schema);

        Self {
            collection,
            id_manager,
            config,
            name: name.to_ascii_lowercase(),
            schema,
        }
    }

    pub fn into_protected(self) -> MemoryCollection {
        RwLock::new(self)
    }

    pub fn schema(&self) -> Option<SchemaDict> {
        self.schema.as_ref().cloned()
    }

    pub(crate) fn set_schema(&mut self, schema: SchemaDict) {
        self.schema = Some(schema);
    }

    pub fn get_reference_column_name(&self) -> String {
        let name = if self.name.ends_with("s") {
            self.name[..self.name.len() - 1].to_string()
        } else {
            self.name.to_string()
        };

        let id_key = if self.config.id_key.starts_with("_") {
            let mut id_key = self.config.id_key.clone();
            id_key.remove(0);
            id_key
        } else {
            self.config.id_key.clone()
        };

        if id_key == format!("{}_id", name) || id_key.starts_with(&format!("{name}_")) {
            return id_key;
        }

        format!("{}_{}", name, id_key)
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
        self.collection
            .values()
            .skip(offset)
            .take(limit)
            .cloned()
            .collect::<Vec<Value>>()
    }

    pub fn get(&self, id: &str) -> Option<Value> {
        self.collection.get(id).cloned()
    }

    pub fn get_filtered_by_columns_values(
        &self,
        columns_values: Vec<ColumnValue>,
        expansion_type: ExpansionChain,
        db: &Db,
    ) -> Result<Vec<Value>, CollectionReadError> {
        let mut rows = Vec::new();
        for row in self.collection.values() {
            let Value::Object(map) = row else {
                continue;
            };

            let matches =
                columns_values
                    .iter()
                    .all(|column_value| match map.get(&column_value.column) {
                        Some(value) => *value == column_value.value,
                        None => false,
                    });

            if matches {
                rows.push(self.expand_row(row, expansion_type.clone(), db)?);
            }
        }

        Ok(rows)
    }

    fn expand_object(
        &self,
        object: Map<String, Value>,
        collection_name: String,
        next_expansion_type: ExpansionChain,
        db: &Db,
    ) -> Result<Value, CollectionReadError> {
        let refs = db.get_collection_refs(&self.name);
        let mut object = object.clone();

        match refs {
            Some(refs) => {
                for entry in refs.values() {
                    // n-1
                    if entry.ref_collection.eq_ignore_ascii_case(&collection_name)
                        && let Some(collection) = db.get(&entry.ref_collection)
                        && let Some(cell) = object.get(&entry.column)
                    {
                        let cvs = vec![ColumnValue::new(entry.ref_column.clone(), cell.clone())];
                        let expanded = collection.get_filtered_by_columns_values(
                            cvs,
                            next_expansion_type.clone(),
                            db,
                        )?;
                        let key = collection.get_name()?;
                        object.insert(key, Value::Array(expanded));
                    }

                    // 1-n
                    if entry.collection.eq_ignore_ascii_case(&collection_name)
                        && let Some(collection) = db.get(&entry.collection)
                        && let Some(cell) = object.get(&entry.ref_column)
                    {
                        let cvs = vec![ColumnValue::new(entry.column.clone(), cell.clone())];
                        let expanded = collection.get_filtered_by_columns_values(
                            cvs,
                            next_expansion_type.clone(),
                            db,
                        )?;
                        let key = collection.get_name()?;
                        object.insert(key, Value::Array(expanded));
                    }
                }
                Ok(Value::Object(object))
            }
            None => Ok(Value::Object(object)),
        }
    }

    pub fn expand_row(
        &self,
        row: &Value,
        expansion_type: ExpansionChain,
        db: &Db,
    ) -> Result<Value, CollectionReadError> {
        match (row.clone(), expansion_type) {
            (Value::Object(map), ExpansionChain::Single(collection_name)) => {
                self.expand_object(map, collection_name, ExpansionChain::None, db)
            }
            (Value::Object(map), ExpansionChain::Child(collection_name, expansion_type)) => {
                self.expand_object(map, collection_name, expansion_type.as_ref().clone(), db)
            }
            _ => Ok(row.clone()),
        }
    }

    pub fn expand_list(
        &self,
        list: Vec<Value>,
        expansion_type: ExpansionChain,
        db: &Db,
    ) -> Result<Vec<Value>, CollectionReadError> {
        list.iter()
            .map(|row| self.expand_row(row, expansion_type.clone(), db))
            .collect()
    }

    pub fn exists(&self, id: &str) -> bool {
        self.collection.contains_key(id)
    }

    pub fn count(&self) -> usize {
        self.collection.len()
    }

    pub fn add(&mut self, item: Value) -> Result<Value, AddError> {
        if !item.is_object() {
            return Err(AddError::NonObjectItem);
        }

        let next_id = { self.id_manager.next() };

        let mut item = item;
        let id_string = if let Some(id_value) = next_id {
            // Convert IdValue to string and add it to the item
            let id_string = id_value.to_string();

            // Add the ID to the item using the configured id_key
            if let Value::Object(ref mut map) = item {
                match id_value {
                    IdValue::Uuid(id) => {
                        map.insert(self.config.id_key.clone(), Value::String(id.clone()));
                    }
                    IdValue::Int(id) => {
                        map.insert(self.config.id_key.clone(), Value::Number(id.into()));
                    }
                }
            }
            id_string
        } else if let Some(Value::String(id_string)) = item.get(self.config.id_key.clone()) {
            id_string.clone()
        } else if let Some(Value::Number(id_number)) = item.get(self.config.id_key.clone()) {
            id_number.to_string()
        } else {
            return Err(AddError::MissingId {
                id_key: self.config.id_key.clone(),
            });
        };

        if self.config.id_type == IdType::None && self.collection.contains_key(&id_string) {
            return Err(AddError::DuplicateId { id: id_string });
        }

        self.ensure_update_schema_for_item(&item);

        self.collection.insert(id_string, item.clone());

        Ok(item)
    }

    pub fn add_batch(&mut self, items: Value) -> Result<Vec<Value>, AddBatchError> {
        let Value::Array(items_array) = items else {
            return Err(AddBatchError::NonArrayInput);
        };

        let mut added_items = Vec::new();
        let mut max_id = None;
        for (index, item) in items_array.into_iter().enumerate() {
            if !item.is_object() {
                return Err(AddBatchError::NonObjectItem { index });
            }

            if let Value::Object(ref item_map) = item {
                self.ensure_update_schema_for_item(&item);
                let id_key = self.config.id_key.clone();

                let id = item_map.get(&id_key);
                let id = match self.id_manager.id_type {
                    IdType::Uuid => match id {
                        Some(Value::String(id)) => Some(id.clone()),
                        _ => None,
                    },
                    IdType::Int => match id {
                        Some(Value::Number(id)) => {
                            let id = id.as_u64().ok_or(AddBatchError::InvalidIntId { index })?;
                            if let Some(current) = max_id {
                                if current < id {
                                    max_id = Some(id);
                                    let _ = self.id_manager.set_current(IdValue::Int(id));
                                }
                            } else {
                                max_id = Some(id);
                                let _ = self.id_manager.set_current(IdValue::Int(id));
                            }
                            Some(id.to_string())
                        }
                        _ => None,
                    },
                    IdType::None => match item.get(&id_key) {
                        Some(Value::String(id_string)) => Some(id_string.clone()),
                        Some(Value::Number(id_number)) => Some(id_number.to_string()),
                        _ => None,
                    },
                };

                // Extract the ID from the item using the configured id_key
                if let Some(id) = id {
                    let duplicate_none_id =
                        self.config.id_type == IdType::None && self.collection.contains_key(&id);
                    if duplicate_none_id {
                        return Err(AddBatchError::DuplicateId { index, id });
                    }

                    // Insert the item with its existing ID
                    self.collection.insert(id.clone(), item.clone());
                    added_items.push(item);
                } else if let Some(id) = self.id_manager.next() {
                    // Take ownership of the map so we can mutate it
                    if let Value::Object(mut owned_map) = item {
                        let id_value = match id {
                            IdValue::Uuid(ref s) => Value::String(s.clone()),
                            IdValue::Int(i) => {
                                max_id = Some(i);
                                Value::Number(i.into())
                            }
                        };
                        owned_map.insert(id_key, id_value);
                        let new_item = Value::Object(owned_map);
                        self.collection.insert(id.to_string(), new_item.clone());
                        added_items.push(new_item);
                    }
                } else {
                    return Err(AddBatchError::MissingId { index, id_key });
                }
            }
        }

        Ok(added_items)
    }

    pub fn update(&mut self, id: &str, item: Value) -> Option<Value> {
        let mut item = item;

        // Add the ID to the item using the configured id_key
        if let Value::Object(ref mut map) = item {
            let id_key = self.config.id_key.clone();
            if !map.contains_key(&id_key) {
                let id = match self.config.id_type {
                    IdType::Int => match id.parse::<u64>() {
                        Ok(num) => Value::Number(Number::from(num)),
                        Err(_) => Value::String(id.to_string()),
                    },
                    _ => Value::String(id.to_string()),
                };
                map.insert(self.config.id_key.clone(), id);
            }
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
            let mut updated_item = Self::merge_json_values(existing_item, partial_item);

            // Ensure the ID is still present in the updated item
            if let Value::Object(ref mut map) = updated_item {
                let id_key = self.config.id_key.clone();
                if !map.contains_key(&id_key) {
                    let id = match self.config.id_type {
                        IdType::Int => match id.parse::<u64>() {
                            Ok(num) => Value::Number(Number::from(num)),
                            Err(_) => Value::String(id.to_string()),
                        },
                        _ => Value::String(id.to_string()),
                    };
                    map.insert(self.config.id_key.clone(), id);
                }
            }

            self.ensure_update_schema_for_item(&updated_item);

            // Update the item in the database
            self.collection.insert(id.to_string(), updated_item.clone());
            Some(updated_item)
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

    pub fn load_from_json(
        &mut self,
        json_value: Value,
        keep: bool,
    ) -> Result<Vec<Value>, LoadCollectionError> {
        // Guard: Check if it's a JSON Array
        let Value::Array(_) = json_value else {
            return Err(LoadCollectionError::NonArrayInput);
        };

        if !keep {
            self.clear();
        }

        // Load the array into the collection using add_batch
        let added_items = self.add_batch(json_value)?;
        Ok(added_items)
    }

    pub fn load_from_file(&mut self, file_path: &OsString) -> Result<String, LoadCollectionError> {
        let file_path_lossy = file_path.to_string_lossy();

        // Guard: Try to read the file content
        let file_content =
            fs::read_to_string(file_path).map_err(|_| LoadCollectionError::FileRead {
                path: file_path_lossy.to_string(),
            })?;

        // Guard: Try to parse the content as JSON
        let json_value = serde_json::from_str::<Value>(&file_content).map_err(|_| {
            LoadCollectionError::InvalidJson {
                path: file_path_lossy.to_string(),
            }
        })?;

        match self.load_from_json(json_value, false) {
            Ok(added_items) => Ok(format!(
                "✔️ Loaded {} initial items from {}",
                added_items.len(),
                file_path_lossy
            )),
            Err(error) => Err(error),
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
    /// directly (for example with `dbcoll.collection.read()`) but prefer the
    /// high-level methods on `DbCollection` when possible.
    pub(crate) collection: MemoryCollection,
}

impl DbCollection {
    /// Create a new `DbCollection` backed by an internal in-memory collection.
    ///
    /// `name` is the collection name and `config` controls id strategy and key.
    ///
    /// Most users create collections through [`Db::create`](crate::Db::create)
    /// or [`Db::create_with_config`](crate::Db::create_with_config), which
    /// register the collection in a database.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    ///
    /// let name = people.get_name().map_err(|error| error.to_string())?;
    /// assert_eq!(name, "people");
    /// # Ok(())
    /// # }
    /// ```
    pub fn new_coll(name: &str, config: DbConfig) -> Self {
        Self {
            collection: InternalMemoryCollection::new_coll(name, config).into_protected(),
        }
    }

    /// Get the default reference field name for this collection based on its name and id key.
    ///
    /// For a collection named `users` with id key `id`, this returns `user_id`.
    ///
    /// This value is used by [`Db::infer_reference`](crate::Db::infer_reference)
    /// when it searches for conventional foreign-key-like fields.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    ///
    /// # fn main() -> Result<(), String> {
    /// let users = DbCollection::new_coll("users", DbConfig::int("id"));
    ///
    /// let reference_column = users
    ///     .get_reference_column_name()
    ///     .map_err(|error| error.to_string())?;
    /// assert_eq!(reference_column, "user_id");
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_reference_column_name(&self) -> Result<String, CollectionReadError> {
        Ok(self
            .collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .get_reference_column_name())
    }

    /// Return all documents in the collection as a `Vec<Value>`.
    ///
    /// This clones stored JSON values and is intended for small collections or
    /// tests; prefer paginated access for large datasets.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    /// let _inserted = people
    ///     .add(json!({ "id": 1, "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let all = people.get_all().map_err(|error| error.to_string())?;
    ///
    /// assert_eq!(all[0]["name"], "Ada");
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_all(&self) -> Result<Vec<Value>, CollectionReadError> {
        Ok(self
            .collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .get_all())
    }

    /// Return a page of documents starting at `offset` with at most `limit`
    /// items.
    ///
    /// `offset` is zero-based. If `offset` is past the end of the collection,
    /// an empty vector is returned.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    /// let _inserted = people
    ///     .add_batch(json!([{ "name": "Ada" }, { "name": "Grace" }]))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let page = people
    ///     .get_paginated(1, 1)
    ///     .map_err(|error| error.to_string())?;
    ///
    /// assert_eq!(page.len(), 1);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_paginated(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<Value>, CollectionReadError> {
        Ok(self
            .collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .get_paginated(offset, limit))
    }

    pub(crate) fn get_filtered_by_columns_values(
        &self,
        columns_values: Vec<ColumnValue>,
        expansion_type: ExpansionChain,
        db: &Db,
    ) -> Result<Vec<Value>, CollectionReadError> {
        self.collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .get_filtered_by_columns_values(columns_values, expansion_type, db)
    }

    /// Retrieve a single document by id.
    ///
    /// Ids are addressed as strings even when the stored id value is numeric.
    /// The returned value is a clone of the stored JSON document.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    /// let _inserted = people
    ///     .add(json!({ "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let ada = people
    ///     .get("1")
    ///     .map_err(|error| error.to_string())?
    ///     .ok_or_else(|| "missing person with id 1".to_string())?;
    ///
    /// assert_eq!(ada["name"], "Ada");
    /// # Ok(())
    /// # }
    /// ```
    pub fn get(&self, id: &str) -> Result<Option<Value>, CollectionReadError> {
        Ok(self
            .collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .get(id))
    }

    /// Check whether a document with `id` exists in the collection.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    /// let _inserted = people
    ///     .add(json!({ "id": "ada", "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let ada_exists = people.exists("ada").map_err(|error| error.to_string())?;
    /// let grace_exists = people.exists("grace").map_err(|error| error.to_string())?;
    /// assert!(ada_exists);
    /// assert!(!grace_exists);
    /// # Ok(())
    /// # }
    /// ```
    pub fn exists(&self, id: &str) -> Result<bool, CollectionReadError> {
        Ok(self
            .collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .exists(id))
    }

    /// Return the number of documents currently stored in the collection.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    /// let _inserted = people
    ///     .add_batch(json!([{ "name": "Ada" }, { "name": "Grace" }]))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let count = people.count().map_err(|error| error.to_string())?;
    /// assert_eq!(count, 2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn count(&self) -> Result<usize, CollectionReadError> {
        Ok(self
            .collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .count())
    }

    /// Add a document to the collection.
    ///
    /// Depending on the configured `id_type`, the collection may generate an
    /// id and insert it into the document. Returns the stored document on
    /// success with the id populated.
    ///
    /// The input must be a JSON object. For `IdType::Int` and `IdType::Uuid`,
    /// an id is generated when the configured id key is absent. For
    /// `IdType::None`, the document must contain the configured id key.
    ///
    /// # Errors
    ///
    /// Returns [`AddError::LockPoisoned`] when the collection lock cannot be
    /// acquired, [`AddError::NonObjectItem`] for non-object JSON values,
    /// [`AddError::MissingId`] when caller-managed ids are required but absent,
    /// or [`AddError::DuplicateId`] for duplicate caller-managed ids.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    ///
    /// let inserted = people
    ///     .add(json!({ "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// assert_eq!(inserted["id"], 1);
    /// assert_eq!(inserted["name"], "Ada");
    /// # Ok(())
    /// # }
    /// ```
    pub fn add(&self, item: Value) -> Result<Value, AddError> {
        self.collection
            .write()
            .map_err(|_| AddError::LockPoisoned)?
            .add(item)
    }

    /// Add multiple items from a JSON array value and return the items that
    /// were added.
    ///
    /// # Errors
    ///
    /// Returns an error when the input is not an array, an array item is not an
    /// object, the configured id is missing, or an id conflicts with another
    /// inserted item. It also returns an error when the collection lock cannot
    /// be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    ///
    /// let inserted = people.add_batch(json!([
    ///     { "id": "ada", "name": "Ada" },
    ///     { "id": "grace", "name": "Grace" }
    /// ])).map_err(|error| error.to_string())?;
    ///
    /// assert_eq!(inserted.len(), 2);
    /// let count = people.count().map_err(|error| error.to_string())?;
    /// assert_eq!(count, 2);
    /// # Ok(())
    /// # }
    /// ```
    pub fn add_batch(&self, items: Value) -> Result<Vec<Value>, AddBatchError> {
        self.collection
            .write()
            .map_err(|_| AddBatchError::LockPoisoned)?
            .add_batch(items)
    }

    /// Replace the document with id `id` with `item`. Returns the stored
    /// document on success or `None` if the id was not present.
    ///
    /// This is a full replacement. Fields not present in `item` are removed
    /// from the stored document, except for id handling performed by the
    /// collection.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionWriteError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    /// let _inserted = people
    ///     .add(json!({ "id": "ada", "name": "Ada", "age": 37 }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let updated = people
    ///     .update("ada", json!({ "id": "ada", "name": "Ada Lovelace" }))
    ///     .map_err(|error| error.to_string())?
    ///     .ok_or_else(|| "missing person with id ada".to_string())?;
    ///
    /// assert_eq!(updated["name"], "Ada Lovelace");
    /// assert!(updated.get("age").is_none());
    /// # Ok(())
    /// # }
    /// ```
    pub fn update(&self, id: &str, item: Value) -> Result<Option<Value>, CollectionWriteError> {
        Ok(self
            .collection
            .write()
            .map_err(|_| CollectionWriteError::LockPoisoned)?
            .update(id, item))
    }

    /// Apply a partial update to the document with `id` by merging JSON
    /// values; returns the updated document or `None` if the id is not found.
    ///
    /// Object fields are merged recursively. Non-object values replace the
    /// existing value at that field.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionWriteError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    /// let _inserted = people
    ///     .add(json!({
    ///     "id": "ada",
    ///     "name": "Ada",
    ///     "profile": { "city": "London" }
    /// }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let updated = people
    ///     .update_partial("ada", json!({ "profile": { "role": "engineer" } }))
    ///     .map_err(|error| error.to_string())?
    ///     .ok_or_else(|| "missing person with id ada".to_string())?;
    ///
    /// assert_eq!(updated["profile"]["city"], "London");
    /// assert_eq!(updated["profile"]["role"], "engineer");
    /// # Ok(())
    /// # }
    /// ```
    pub fn update_partial(
        &self,
        id: &str,
        partial_item: Value,
    ) -> Result<Option<Value>, CollectionWriteError> {
        Ok(self
            .collection
            .write()
            .map_err(|_| CollectionWriteError::LockPoisoned)?
            .update_partial(id, partial_item))
    }

    /// Remove and return the document with `id` if it exists.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionWriteError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    /// let _inserted = people
    ///     .add(json!({ "id": "ada", "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let removed = people
    ///     .delete("ada")
    ///     .map_err(|error| error.to_string())?
    ///     .ok_or_else(|| "missing person with id ada".to_string())?;
    ///
    /// assert_eq!(removed["name"], "Ada");
    /// let exists = people.exists("ada").map_err(|error| error.to_string())?;
    /// assert!(!exists);
    /// # Ok(())
    /// # }
    /// ```
    pub fn delete(&self, id: &str) -> Result<Option<Value>, CollectionWriteError> {
        Ok(self
            .collection
            .write()
            .map_err(|_| CollectionWriteError::LockPoisoned)?
            .delete(id))
    }

    /// Remove all documents and return the number of removed items.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionWriteError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    /// let _inserted = people
    ///     .add_batch(json!([{ "name": "Ada" }, { "name": "Grace" }]))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let removed = people.clear().map_err(|error| error.to_string())?;
    /// let count = people.count().map_err(|error| error.to_string())?;
    /// assert_eq!(removed, 2);
    /// assert_eq!(count, 0);
    /// # Ok(())
    /// # }
    /// ```
    pub fn clear(&self) -> Result<usize, CollectionWriteError> {
        Ok(self
            .collection
            .write()
            .map_err(|_| CollectionWriteError::LockPoisoned)?
            .clear())
    }

    /// Load documents from a serde_json `Value` (must be an array) and return
    /// the list of items actually added.
    ///
    /// If `keep` is `true`, existing ids in the input are preserved where
    /// possible. If `keep` is `false`, ids may be regenerated according to
    /// the collection configuration.
    ///
    /// # Errors
    ///
    /// Returns [`LoadCollectionError::LockPoisoned`] when the collection lock
    /// cannot be acquired, [`LoadCollectionError::NonArrayInput`] when the
    /// root value is not an array, or [`LoadCollectionError::Batch`] when an
    /// item cannot be inserted.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    ///
    /// let inserted = people
    ///     .load_from_json(json!([{ "id": 1, "name": "Ada" }]), true)
    ///     .map_err(|error| error.to_string())?;
    ///
    /// assert_eq!(inserted.len(), 1);
    /// let stored = people
    ///     .get("1")
    ///     .map_err(|error| error.to_string())?
    ///     .ok_or_else(|| "missing person with id 1".to_string())?;
    /// assert_eq!(stored["name"], "Ada");
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_from_json(
        &self,
        json_value: Value,
        keep: bool,
    ) -> Result<Vec<Value>, LoadCollectionError> {
        self.collection
            .write()
            .map_err(|_| LoadCollectionError::LockPoisoned)?
            .load_from_json(json_value, keep)
    }

    /// Load documents from a file path. Returns a human-readable status on
    /// success.
    ///
    /// The file must contain a JSON array of documents.
    ///
    /// # Errors
    ///
    /// Returns [`LoadCollectionError::LockPoisoned`] when the collection lock
    /// cannot be acquired, [`LoadCollectionError::FileRead`] when the file
    /// cannot be read, [`LoadCollectionError::InvalidJson`] when parsing fails,
    /// or a load/batch validation error when file contents are invalid.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use fosk::{DbCollection, DbConfig};
    /// use std::ffi::OsString;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    /// let status = people.load_from_file(&OsString::from("people.json"))?;
    ///
    /// println!("{status}");
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_from_file(&self, file_path: &OsString) -> Result<String, LoadCollectionError> {
        self.collection
            .write()
            .map_err(|_| LoadCollectionError::LockPoisoned)?
            .load_from_file(file_path)
    }

    /// Load this collection's schema from a compact JSON object.
    ///
    /// The JSON object maps field names to compact type strings. ID markers
    /// such as `"Id"`, `"Uuid"`, and `"None:String"` must match this
    /// collection's existing [`DbConfig`]. This method replaces schema
    /// metadata only; it does not mutate stored rows or ID generator state.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig, JsonPrimitive};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("person_id"));
    ///
    /// people
    ///     .load_schema_from_json(json!({
    ///         "person_id": "Id",
    ///         "name": "String!"
    ///     }))
    ///     ?;
    ///
    /// let schema = people
    ///     .schema()
    ///     .map_err(|error| error.to_string())?
    ///     .ok_or_else(|| "missing people schema".to_string())?;
    /// assert_eq!(schema.fields["person_id"].ty, JsonPrimitive::Int);
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_schema_from_json(&self, json_value: Value) -> Result<(), String> {
        let parsed = parse_schema_for_load(&json_value)?;
        apply_schema_to_collection(self, parsed)
    }

    /// Load this collection's schema from a compact JSON file.
    ///
    /// The file must contain the same field-name-to-type object accepted by
    /// [`DbCollection::load_schema_from_json`].
    ///
    /// # Example
    ///
    /// ```no_run
    /// use fosk::{DbCollection, DbConfig};
    /// use std::ffi::OsString;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    /// let status = people.load_schema_from_file(&OsString::from("people.schema.json"))?;
    ///
    /// println!("{status}");
    /// # Ok(())
    /// # }
    /// ```
    pub fn load_schema_from_file(&self, file_path: &OsString) -> Result<String, String> {
        let json_value = read_schema_json_file(file_path)?;
        self.load_schema_from_json(json_value)?;
        Ok(format!(
            "Loaded schema for collection {} from {}",
            self.get_name().map_err(|error| error.to_string())?,
            file_path.to_string_lossy()
        ))
    }

    /// Save this collection to a pretty-printed JSON file.
    ///
    /// The file contains a JSON array of all stored documents.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be created, the collection cannot be
    /// read, or serialization fails.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use fosk::{DbCollection, DbConfig};
    /// use std::ffi::OsString;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    ///
    /// people.write_to_file(&OsString::from("people.json"))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn write_to_file(&self, file_path: &OsString) -> Result<(), WriteCollectionError> {
        let file = std::fs::File::create(file_path).map_err(|source| {
            WriteCollectionError::FileCreate {
                path: file_path.to_string_lossy().to_string(),
                source,
            }
        })?;
        let mut w = BufWriter::new(file);

        let data = self
            .get_all()
            .map_err(|_| WriteCollectionError::LockPoisoned)?;
        write_collection_data_to_writer(&mut w, &data)?;
        Ok(())
    }

    /// Expand a single JSON `Value` row by following a dot-separated expansion chain.
    ///
    /// `expansion` specifies which related collection to include. For example,
    /// calling `expand_row(row, "orders.items", &db)` will nest `items` under `orders`.
    ///
    /// References must be registered first with
    /// [`Db::create_reference`](crate::Db::create_reference) or
    /// [`Db::infer_reference`](crate::Db::infer_reference).
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when this collection or a
    /// related collection cannot be read.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let db = Db::new_with_config(DbConfig::none("id"));
    /// let people = db.create("people");
    /// let orders = db.create("orders");
    ///
    /// let _person = people
    ///     .add(json!({ "id": 1, "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    /// let _order = orders
    ///     .add(json!({ "id": 10, "person_id": 1 }))
    ///     .map_err(|error| error.to_string())?;
    /// db.create_reference("orders", "person_id", "people", "id");
    ///
    /// let order = orders
    ///     .get("10")
    ///     .map_err(|error| error.to_string())?
    ///     .ok_or_else(|| "missing order with id 10".to_string())?;
    /// let expanded = orders
    ///     .expand_row(&order, "people", &db)
    ///     .map_err(|error| error.to_string())?;
    ///
    /// assert_eq!(expanded["people"][0]["name"], "Ada");
    /// # Ok(())
    /// # }
    /// ```
    pub fn expand_row(
        &self,
        row: &Value,
        expansion: &str,
        db: &Db,
    ) -> Result<Value, CollectionReadError> {
        self.collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .expand_row(row, ExpansionChain::from(expansion), db)
    }

    /// Expand each JSON `Value` in a list by applying the same expansion chain.
    ///
    /// Returns a new `Vec<Value>` where each element has been passed through `expand_row`.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when this collection or a
    /// related collection cannot be read.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let db = Db::new_with_config(DbConfig::none("id"));
    /// let people = db.create("people");
    /// let orders = db.create("orders");
    ///
    /// let _person = people
    ///     .add(json!({ "id": 1, "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    /// let _order = orders
    ///     .add(json!({ "id": 10, "person_id": 1 }))
    ///     .map_err(|error| error.to_string())?;
    /// db.create_reference("orders", "person_id", "people", "id");
    ///
    /// let orders_list = orders.get_all().map_err(|error| error.to_string())?;
    /// let expanded = orders
    ///     .expand_list(orders_list, "people", &db)
    ///     .map_err(|error| error.to_string())?;
    ///
    /// assert_eq!(expanded[0]["people"][0]["name"], "Ada");
    /// # Ok(())
    /// # }
    /// ```
    pub fn expand_list(
        &self,
        list: Vec<Value>,
        expansion: &str,
        db: &Db,
    ) -> Result<Vec<Value>, CollectionReadError> {
        self.collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .expand_list(list, ExpansionChain::from(expansion), db)
    }

    /// Return the optionally-inferred `SchemaDict` for this collection (if
    /// any documents have been added that allowed schema inference).
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig, JsonPrimitive};
    /// use serde_json::json;
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    /// let _inserted = people
    ///     .add(json!({ "id": 1, "name": "Ada" }))
    ///     .map_err(|error| error.to_string())?;
    ///
    /// let schema = people
    ///     .schema()
    ///     .map_err(|error| error.to_string())?
    ///     .ok_or_else(|| "missing people schema".to_string())?;
    ///
    /// assert_eq!(schema.fields["name"].ty, JsonPrimitive::String);
    /// # Ok(())
    /// # }
    /// ```
    pub fn schema(&self) -> Result<Option<SchemaDict>, CollectionReadError> {
        Ok(self
            .collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .schema())
    }

    pub(crate) fn set_schema(&self, schema: SchemaDict) -> Result<(), CollectionWriteError> {
        self.collection
            .write()
            .map_err(|_| CollectionWriteError::LockPoisoned)?
            .set_schema(schema);
        Ok(())
    }

    /// Return the collection name.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::int("id"));
    ///
    /// let name = people.get_name().map_err(|error| error.to_string())?;
    /// assert_eq!(name, "people");
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_name(&self) -> Result<String, CollectionReadError> {
        Ok(self
            .collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .name
            .clone())
    }

    /// Return this collection's configuration.
    ///
    /// The returned value is a clone. Mutating it does not affect the
    /// collection.
    ///
    /// # Errors
    ///
    /// Returns [`CollectionReadError::LockPoisoned`] when the collection lock
    /// cannot be acquired.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{DbCollection, DbConfig};
    ///
    /// # fn main() -> Result<(), String> {
    /// let people = DbCollection::new_coll("people", DbConfig::none("id"));
    ///
    /// let config = people.get_config().map_err(|error| error.to_string())?;
    /// assert_eq!(config, DbConfig::none("id"));
    /// # Ok(())
    /// # }
    /// ```
    pub fn get_config(&self) -> Result<DbConfig, CollectionReadError> {
        Ok(self
            .collection
            .read()
            .map_err(|_| CollectionReadError::LockPoisoned)?
            .config
            .clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn create_test_collection() -> InternalMemoryCollection {
        InternalMemoryCollection::new("test_collection", DbConfig::int("id"))
    }

    fn create_uuid_collection() -> InternalMemoryCollection {
        InternalMemoryCollection::new("uuid_collection", DbConfig::uuid("id"))
    }

    fn create_none_collection() -> InternalMemoryCollection {
        InternalMemoryCollection::new("none_collection", DbConfig::none("id"))
    }

    fn add_item(collection: &mut InternalMemoryCollection, item: Value) -> Value {
        collection
            .add(item)
            .unwrap_or_else(|error| panic!("test row should insert: {error}"))
    }

    fn load_from_file_status(
        collection: &mut InternalMemoryCollection,
        file_path: &std::ffi::OsStr,
    ) -> String {
        collection
            .load_from_file(&file_path.to_os_string())
            .unwrap_or_else(|error| panic!("test file should load: {error}"))
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

        let first = add_item(&mut collection, json!({"name": "Item 1"}));
        let second = add_item(&mut collection, json!({"name": "Item 2"}));
        let third = add_item(&mut collection, json!({"name": "Item 3"}));
        assert_eq!(first["id"], 1);
        assert_eq!(second["id"], 2);
        assert_eq!(third["id"], 3);

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
        let id = item.get("id").unwrap().as_u64().unwrap();

        let retrieved = collection.get(&id.to_string());
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

        for i in 1..=10 {
            let inserted = add_item(&mut collection, json!({"name": format!("Item {}", i)}));
            assert_eq!(inserted["id"], i);
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
        let id = item.get("id").unwrap().as_u64().unwrap();

        assert!(collection.exists(&id.to_string()));
        assert!(!collection.exists("999"));
    }

    #[test]
    fn test_count() {
        let mut collection = create_test_collection();
        assert_eq!(collection.count(), 0);

        let first = add_item(&mut collection, json!({"name": "Item 1"}));
        assert_eq!(first["id"], 1);
        assert_eq!(collection.count(), 1);

        let second = add_item(&mut collection, json!({"name": "Item 2"}));
        assert_eq!(second["id"], 2);
        assert_eq!(collection.count(), 2);

        // Delete one
        let deleted = collection.delete("1").unwrap();
        assert_eq!(deleted["name"], "Item 1");
        assert_eq!(collection.count(), 1);
    }

    #[test]
    fn test_add_with_int_id() {
        let mut collection = create_test_collection();

        let item = add_item(&mut collection, json!({"name": "Test Item"}));
        assert_eq!(item.get("name").unwrap(), "Test Item");
        assert_eq!(item.get("id").unwrap(), 1);

        // Add another item
        let item2 = add_item(&mut collection, json!({"name": "Test Item 2"}));
        assert_eq!(item2.get("id").unwrap(), 2);
    }

    #[test]
    fn test_add_with_uuid_id() {
        let mut collection = create_uuid_collection();

        let item = add_item(&mut collection, json!({"name": "Test Item"}));
        assert_eq!(item.get("name").unwrap(), "Test Item");
        let id = item.get("id").unwrap().as_str().unwrap();
        assert!(!id.is_empty());
        assert!(id.len() > 10); // UUIDs are longer than 10 characters
    }

    #[test]
    fn test_add_with_none_id_existing() {
        let mut collection = create_none_collection();

        let item = add_item(
            &mut collection,
            json!({"id": "custom-id", "name": "Test Item"}),
        );
        assert_eq!(item.get("name").unwrap(), "Test Item");
        assert_eq!(item.get("id").unwrap(), "custom-id");
    }

    #[test]
    fn test_add_with_none_id_number_existing() {
        let mut collection = create_none_collection();

        let item = add_item(&mut collection, json!({"id": 1, "name": "Test Item"}));
        assert_eq!(item.get("name").unwrap(), "Test Item");
        assert_eq!(item.get("id").unwrap(), 1);
    }

    #[test]
    fn test_add_with_none_id_missing() {
        let mut collection = create_none_collection();

        let error = collection.add(json!({"name": "Test Item"})).unwrap_err();
        assert_eq!(
            error,
            AddError::MissingId {
                id_key: "id".to_string()
            }
        );
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_add_with_none_id_duplicate_is_rejected() {
        let mut collection = create_none_collection();

        let first = add_item(
            &mut collection,
            json!({"id": "custom-id", "name": "Original"}),
        );
        assert_eq!(first["id"], "custom-id");

        let duplicate = collection
            .add(json!({"id": "custom-id", "name": "Replacement"}))
            .unwrap_err();
        assert_eq!(
            duplicate,
            AddError::DuplicateId {
                id: "custom-id".to_string()
            }
        );
        assert_eq!(collection.count(), 1);

        let stored = collection.get("custom-id").unwrap();
        assert_eq!(stored.get("name").unwrap(), "Original");
    }

    #[test]
    fn test_add_with_none_numeric_id_duplicate_is_rejected() {
        let mut collection = create_none_collection();

        let first = add_item(&mut collection, json!({"id": 7, "name": "Original"}));
        assert_eq!(first["id"], 7);

        let duplicate = collection
            .add(json!({"id": 7, "name": "Replacement"}))
            .unwrap_err();
        assert_eq!(
            duplicate,
            AddError::DuplicateId {
                id: "7".to_string()
            }
        );
        assert_eq!(collection.count(), 1);

        let stored = collection.get("7").unwrap();
        assert_eq!(stored.get("name").unwrap(), "Original");
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

        let added_items = collection.add_batch(batch).unwrap();
        assert_eq!(added_items.len(), 4); // Only items with IDs should be added
        assert_eq!(collection.count(), 4);

        // Check that the max ID was set correctly
        let new_item = collection.add(json!({"name": "New Item"})).unwrap();
        assert_eq!(new_item.get("id").unwrap(), 11); // Should be max + 1
    }

    #[test]
    fn test_add_batch_uuid() {
        let mut collection = create_uuid_collection();

        let batch = json!([
            {"id": "uuid-1", "name": "Item 1"},
            {"id": "uuid-2", "name": "Item 2"},
            {"name": "Item 3"}
        ]);

        let added_items = collection.add_batch(batch).unwrap();
        assert_eq!(added_items.len(), 3);
        assert_eq!(collection.count(), 3);
    }

    #[test]
    fn test_add_batch_none() {
        let mut collection = create_none_collection();

        let batch = json!([
            {"id": "custom-1", "name": "Item 1"},
            {"id": "custom-2", "name": "Item 2"},
            {"name": "Item 3"},
            {"id": 3, "name": "Item 4"},
        ]);

        let error = collection.add_batch(batch).unwrap_err();
        assert_eq!(
            error,
            AddBatchError::MissingId {
                index: 2,
                id_key: "id".to_string()
            }
        );
        assert_eq!(collection.count(), 2);
    }

    #[test]
    fn test_add_batch_duplicate_id_is_rejected() {
        let mut collection = create_none_collection();

        let batch = json!([
            {"id": "custom-1", "name": "Original"},
            {"id": "custom-1", "name": "Replacement"},
        ]);

        let error = collection.add_batch(batch).unwrap_err();
        assert_eq!(
            error,
            AddBatchError::DuplicateId {
                index: 1,
                id: "custom-1".to_string()
            }
        );
        assert_eq!(collection.count(), 1);

        let stored = collection.get("custom-1").unwrap();
        assert_eq!(stored.get("name").unwrap(), "Original");
    }

    #[test]
    fn test_add_batch_duplicate_id_against_existing_record_is_rejected() {
        let mut collection = create_none_collection();
        let existing = add_item(
            &mut collection,
            json!({"id": "custom-1", "name": "Existing"}),
        );
        assert_eq!(existing["id"], "custom-1");

        let batch = json!([
            {"id": "custom-1", "name": "Replacement"},
            {"id": "custom-2", "name": "New"},
        ]);

        let error = collection.add_batch(batch).unwrap_err();
        assert_eq!(
            error,
            AddBatchError::DuplicateId {
                index: 0,
                id: "custom-1".to_string()
            }
        );
        assert_eq!(collection.count(), 1);

        let existing = collection.get("custom-1").unwrap();
        assert_eq!(existing.get("name").unwrap(), "Existing");

        assert!(collection.get("custom-2").is_none());
    }

    #[test]
    fn test_add_batch_none_rejects_duplicate_numeric_ids() {
        let mut collection = create_none_collection();

        let batch = json!([
            {"id": 42, "name": "Original"},
            {"id": 42, "name": "Replacement"},
            {"id": 43, "name": "Other"},
        ]);

        let error = collection.add_batch(batch).unwrap_err();
        assert_eq!(
            error,
            AddBatchError::DuplicateId {
                index: 1,
                id: "42".to_string()
            }
        );
        assert_eq!(collection.count(), 1);

        let original = collection.get("42").unwrap();
        assert_eq!(original.get("name").unwrap(), "Original");

        assert!(collection.get("43").is_none());
    }

    #[test]
    fn test_add_batch_none_rejects_later_duplicate_but_keeps_following_valid_items() {
        let mut collection = create_none_collection();

        let batch = json!([
            {"id": "custom-1", "name": "Original"},
            {"id": "custom-2", "name": "Second"},
            {"id": "custom-1", "name": "Replacement"},
            {"name": "Missing Id"},
            {"id": "custom-3", "name": "Third"},
        ]);

        let error = collection.add_batch(batch).unwrap_err();
        assert_eq!(
            error,
            AddBatchError::DuplicateId {
                index: 2,
                id: "custom-1".to_string()
            }
        );
        assert_eq!(collection.count(), 2);

        let original = collection.get("custom-1").unwrap();
        assert_eq!(original.get("name").unwrap(), "Original");

        assert!(collection.get("custom-2").is_some());
        assert!(collection.get("custom-3").is_none());
    }

    #[test]
    fn test_add_batch_int_keeps_existing_replacement_behavior_for_explicit_ids() {
        let mut collection = create_test_collection();

        let batch = json!([
            {"id": 5, "name": "Original"},
            {"id": 5, "name": "Replacement"},
        ]);

        let added_items = collection.add_batch(batch).unwrap();
        assert_eq!(added_items.len(), 2);
        assert_eq!(collection.count(), 1);

        let stored = collection.get("5").unwrap();
        assert_eq!(stored.get("name").unwrap(), "Replacement");
    }

    #[test]
    fn test_add_batch_uuid_keeps_existing_replacement_behavior_for_explicit_ids() {
        let mut collection = create_uuid_collection();

        let batch = json!([
            {"id": "uuid-1", "name": "Original"},
            {"id": "uuid-1", "name": "Replacement"},
        ]);

        let added_items = collection.add_batch(batch).unwrap();
        assert_eq!(added_items.len(), 2);
        assert_eq!(collection.count(), 1);

        let stored = collection.get("uuid-1").unwrap();
        assert_eq!(stored.get("name").unwrap(), "Replacement");
    }

    #[test]
    fn test_add_batch_non_array() {
        let mut collection = create_test_collection();

        let non_array = json!({"name": "Single Item"});
        assert_eq!(
            collection.add_batch(non_array).unwrap_err(),
            AddBatchError::NonArrayInput
        );
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn test_update_existing_item() {
        let mut collection = create_test_collection();
        let item = collection.add(json!({"name": "Original Name"})).unwrap();
        let id = item.get("id").unwrap().as_u64().unwrap();

        let updated = collection.update(
            &id.to_string(),
            json!({"name": "Updated Name", "description": "New field"}),
        );
        assert!(updated.is_some());

        let updated_item = updated.unwrap();
        assert_eq!(updated_item.get("name").unwrap(), "Updated Name");
        assert_eq!(updated_item.get("description").unwrap(), "New field");
        assert_eq!(updated_item.get("id").unwrap(), id);

        // Verify it's actually updated in the collection
        let retrieved = collection.get(&id.to_string()).unwrap();
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
        let item = collection
            .add(json!({
                "name": "Original Name",
                "description": "Original Description",
                "count": 42
            }))
            .unwrap();
        let id = item.get("id").unwrap().as_u64().unwrap();

        let updated = collection.update_partial(&id.to_string(), json!({"name": "Updated Name"}));
        assert!(updated.is_some());

        let updated_item = updated.unwrap();
        assert_eq!(updated_item.get("name").unwrap(), "Updated Name");
        assert_eq!(
            updated_item.get("description").unwrap(),
            "Original Description"
        ); // Should remain
        assert_eq!(updated_item.get("count").unwrap(), 42); // Should remain
        assert_eq!(updated_item.get("id").unwrap(), id);
    }

    #[test]
    fn test_update_partial_nested_objects() {
        let mut collection = create_test_collection();
        let item = collection
            .add(json!({
                "name": "Test Item",
                "config": {
                    "enabled": true,
                    "timeout": 30,
                    "nested": {
                        "value": "original"
                    }
                }
            }))
            .unwrap();
        let id = item.get("id").unwrap().as_u64().unwrap();

        let updated = collection.update_partial(
            &id.to_string(),
            json!({
                "config": {
                    "timeout": 60,
                    "nested": {
                        "value": "updated",
                        "new_field": "added"
                    }
                }
            }),
        );

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
        let id = item.get("id").unwrap().as_u64().unwrap();

        assert_eq!(collection.count(), 1);

        let deleted = collection.delete(&id.to_string());
        assert!(deleted.is_some());
        assert_eq!(deleted.unwrap().get("name").unwrap(), "Test Item");
        assert_eq!(collection.count(), 0);
        assert!(!collection.exists(&id.to_string()));
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

        let first = add_item(&mut collection, json!({"name": "Item 1"}));
        let second = add_item(&mut collection, json!({"name": "Item 2"}));
        let third = add_item(&mut collection, json!({"name": "Item 3"}));
        assert_eq!(first["id"], 1);
        assert_eq!(second["id"], 2);
        assert_eq!(third["id"], 3);

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
        assert_eq!(item1.get("id").unwrap(), 1);

        let item2 = collection.add(json!({"name": "Item 2"})).unwrap();
        assert_eq!(item2.get("id").unwrap(), 2);

        let item3 = collection.add(json!({"name": "Item 3"})).unwrap();
        assert_eq!(item3.get("id").unwrap(), 3);
    }

    #[test]
    fn test_custom_id_key() {
        let mut collection =
            InternalMemoryCollection::new("custom_collection", DbConfig::int("customId"));

        let item = collection.add(json!({"name": "Test Item"})).unwrap();
        assert_eq!(item.get("customId").unwrap(), 1);
        assert!(item.get("id").is_none()); // Should not have regular "id" field

        // Test retrieval
        let retrieved = collection.get("1").unwrap();
        assert_eq!(retrieved.get("customId").unwrap(), 1);
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
        let status = load_from_file_status(&mut collection, file_path.as_os_str());
        assert!(status.contains("Loaded 3 initial items"));
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
        let status = load_from_file_status(&mut collection, file_path.as_os_str());
        assert!(status.contains("Loaded 0 initial items"));
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
        let status = load_from_file_status(&mut collection, file_path.as_os_str());
        assert!(status.contains("Loaded 2 initial items"));
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
            {"name": "Item 3"} // This should now fail with a typed missing-id error.
        ]);

        let mut file = File::create(&file_path).unwrap();
        file.write_all(test_data.to_string().as_bytes()).unwrap();

        // Load data from file
        let result = collection.load_from_file(&file_path.as_os_str().to_os_string());

        assert_eq!(
            result.unwrap_err(),
            LoadCollectionError::Batch(AddBatchError::MissingId {
                index: 2,
                id_key: "id".to_string()
            })
        );
        assert_eq!(collection.count(), 2);

        assert!(collection.exists("string-id"));
        assert!(collection.exists("42"));
    }

    #[test]
    fn test_load_from_file_nonexistent_file() {
        let mut collection = create_test_collection();
        let nonexistent_path = std::ffi::OsString::from("/path/that/does/not/exist.json");

        let error = collection.load_from_file(&nonexistent_path).unwrap_err();
        assert_eq!(
            error,
            LoadCollectionError::FileRead {
                path: nonexistent_path.to_string_lossy().to_string()
            }
        );
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

        let error = collection
            .load_from_file(&file_path.as_os_str().to_os_string())
            .unwrap_err();
        assert_eq!(
            error,
            LoadCollectionError::InvalidJson {
                path: file_path.to_string_lossy().to_string()
            }
        );
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

        let error = collection
            .load_from_file(&file_path.as_os_str().to_os_string())
            .unwrap_err();
        assert_eq!(error, LoadCollectionError::NonArrayInput);
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

        let error = collection
            .load_from_file(&file_path.as_os_str().to_os_string())
            .unwrap_err();
        assert_eq!(error, LoadCollectionError::NonArrayInput);
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
        let status = load_from_file_status(&mut collection, file_path.as_os_str());
        assert!(status.contains("Loaded 3 initial items"));

        // Add a new item - should get ID 16 (max + 1)
        let new_item = collection.add(json!({"name": "New Item"})).unwrap();
        assert_eq!(new_item.get("id").unwrap(), 16);
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
        let status = load_from_file_status(&mut collection, file_path.as_os_str());
        assert!(status.contains("Loaded 1000 initial items"));
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

        let existing1 = add_item(&mut collection, json!({"name": "Existing Item 1"}));
        let existing2 = add_item(&mut collection, json!({"name": "Existing Item 2"}));
        assert_eq!(existing1["id"], 1);
        assert_eq!(existing2["id"], 2);
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
        let status = load_from_file_status(&mut collection, file_path.as_os_str());
        assert!(status.contains("Loaded 2 initial items"));
        assert_eq!(collection.count(), 2); // 2 loaded

        // Verify all data exists
        assert!(!collection.exists("1")); // Cleaned
        assert!(!collection.exists("2")); // Cleaned
        assert!(collection.exists("10")); // Loaded
        assert!(collection.exists("11")); // Loaded
    }

    #[test]
    fn test_load_from_file_custom_id_key() {
        let mut collection =
            InternalMemoryCollection::new("custom_collection", DbConfig::int("customId"));

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
        let status = load_from_file_status(&mut collection, file_path.as_os_str());
        assert!(status.contains("Loaded 2 initial items"));
        assert_eq!(collection.count(), 2);

        assert!(collection.exists("1"));
        assert!(collection.exists("2"));

        let item1 = collection.get("1").unwrap();
        assert_eq!(item1.get("customId").unwrap(), 1);
        assert_eq!(item1.get("name").unwrap(), "Item 1");
    }

    #[test]
    fn test_write_to_file() {
        use serde_json::json;
        use std::ffi::OsString;
        use std::fs;
        use tempfile::TempDir;

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

        db_collection
            .write_to_file(&os_file_path)
            .unwrap_or_else(|error| panic!("write_to_file should succeed: {error}"));

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
        let db = Db::new_with_config(DbConfig::int("id"));
        let coll = db.create("items");
        // Add an item
        let item = coll.add(json!({"id": 1, "value": "test"})).unwrap();
        // Expand with empty expansion
        let expanded = coll.expand_row(&item, "", &db).unwrap();
        assert_eq!(expanded, item);
        // Expand with non-existent relation
        let expanded2 = coll.expand_row(&item, "unknown", &db).unwrap();
        assert_eq!(expanded2, item);
    }

    #[test]
    fn test_expand_list_no_refs() {
        use serde_json::json;
        // Setup DB and collection with no references
        let db = Db::new_with_config(DbConfig::int("id"));
        let coll = db.create("items");
        // Add items
        let a = coll.add(json!({"id": 1, "value": 10})).unwrap();
        let b = coll.add(json!({"id": 2, "value": 20})).unwrap();
        let list = vec![a.clone(), b.clone()];
        // Expand list with empty expansion
        let expanded = coll.expand_list(list.clone(), "", &db).unwrap();
        assert_eq!(expanded, list);
        // Expand list with missing relation
        let expanded2 = coll.expand_list(list.clone(), "none", &db).unwrap();
        assert_eq!(expanded2, list);
    }

    #[test]
    fn test_expand_row_with_references() {
        use serde_json::json;
        // Build a mutable DB and two collections: authors and books
        let db = Db::new_with_config(DbConfig::int("id"));
        let authors = db.create("authors");
        // Add authors with explicit IDs
        let a1 = authors.add(json!({"name": "Alice"})).unwrap();
        let a2 = authors.add(json!({"name": "Bob"})).unwrap();

        let books = db.create("books");
        // Link book to author by author_id key
        let b1 = books
            .add(json!({"title": "Book1", "author_id": a1.get("id").unwrap()}))
            .unwrap();
        // Add second book to ensure multiple entries, unused in this test
        let _ = books
            .add(json!({"title": "Book2", "author_id": a2.get("id").unwrap()}))
            .unwrap();

        // Create reference from books.author_id to authors.id
        assert!(db.create_reference("books", "author_id", "authors", "id"));

        // Expand book1 row to include its referenced author
        let expanded1 = books.expand_row(&b1, "authors", &db).unwrap();
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
        let db = Db::new_with_config(DbConfig::int("id"));
        let authors = db.create("authors");
        let a1 = authors.add(json!({"name": "Alice"})).unwrap();
        let a2 = authors.add(json!({"name": "Bob"})).unwrap();

        let books = db.create("books");
        let b1 = books
            .add(json!({"title": "Book1", "author_id": a1.get("id").unwrap()}))
            .unwrap();
        let b2 = books
            .add(json!({"title": "Book2", "author_id": a2.get("id").unwrap()}))
            .unwrap();

        assert!(db.create_reference("books", "author_id", "authors", "id"));

        // Expand list of books
        let list = vec![b1.clone(), b2.clone()];
        let expanded_list = books.expand_list(list.clone(), "authors", &db).unwrap();
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

    #[test]
    fn test_expand_row_parent_to_children() {
        use serde_json::json;
        // Set up DB with two collections: orders and order_items
        let db = Db::new_with_config(DbConfig::int("id"));
        let orders = db.create("orders");
        let items = db.create("order_items");
        // Add an order
        let o1 = orders.add(json!({"total": 100})).unwrap();
        // Add one item referencing orders
        let _i1 = items
            .add(json!({"order_id": o1.get("id").unwrap(), "product": "A"}))
            .unwrap();
        // Register reference order_items.order_id -> orders.id
        assert!(db.create_reference("order_items", "order_id", "orders", "id"));
        // Expand parent order row to include its items
        let expanded = orders.expand_row(&o1, "order_items", &db).unwrap();
        if let Value::Object(map) = expanded {
            let arr = map.get("order_items").unwrap().as_array().unwrap();
            // Only one item should appear
            assert_eq!(arr.len(), 1);
            assert_eq!(arr[0].get("product").unwrap().as_str().unwrap(), "A");
        } else {
            panic!("Expected expanded order object");
        }
    }

    #[test]
    fn test_expand_row_multi_level() {
        use serde_json::{Value, json};
        // Set up DB with three collections: orders, order_items, products
        let db = Db::new_with_config(DbConfig::int("id"));
        let orders = db.create("orders");
        let items = db.create("order_items");
        let products = db.create("products");
        // Add one order
        let o1 = orders.add(json!({ "total": 300 })).unwrap();
        // Add two products
        let p1 = products
            .add(json!({ "name": "Widget", "price": 9.99 }))
            .unwrap();
        let p2 = products
            .add(json!({ "name": "Gadget", "price": 19.99 }))
            .unwrap();
        // Add order_items referencing the order and each product
        let _ = items
            .add(json!({
                "order_id": o1.get("id").unwrap(),
                "product_id": p1.get("id").unwrap()
            }))
            .unwrap();
        let _ = items
            .add(json!({
                "order_id": o1.get("id").unwrap(),
                "product_id": p2.get("id").unwrap()
            }))
            .unwrap();
        // Register references for both parent and product relationships
        assert!(db.create_reference("order_items", "order_id", "orders", "id"));
        assert!(db.create_reference("order_items", "product_id", "products", "id"));
        // Perform multi-level expansion: order -> order_items -> product
        let expanded = orders.expand_row(&o1, "order_items.products", &db).unwrap();

        println!("{}", serde_json::to_string_pretty(&expanded).unwrap());

        // Validate structure
        if let Value::Object(map) = expanded {
            // Top-level order_items array
            let items_arr = map.get("order_items").unwrap().as_array().unwrap();
            assert_eq!(items_arr.len(), 2);
            // Each item should include original fields and nested product object
            for item in items_arr {
                let item_map = item.as_object().unwrap();
                // Confirm original order_id and product_id are present
                assert_eq!(item_map.get("order_id").unwrap(), o1.get("id").unwrap());
                let prod_arr = item_map.get("products").unwrap().as_array().unwrap();
                let prod_map = prod_arr[0].as_object().unwrap();
                // Check nested product fields
                assert!(prod_map.contains_key("name"));
                assert!(prod_map.contains_key("price"));
            }
        } else {
            panic!("Expected expanded order object for multi-level expansion");
        }
    }

    #[test]
    fn internal_reference_column_name_handles_plural_and_prefixed_id_keys() {
        let people = InternalMemoryCollection::new("people", DbConfig::int("_id"));
        let categories = InternalMemoryCollection::new("categories", DbConfig::int("category_id"));
        let audit = InternalMemoryCollection::new("audit", DbConfig::uuid("audit_uuid"));

        assert_eq!(people.get_reference_column_name(), "people_id");
        assert_eq!(
            categories.get_reference_column_name(),
            "categorie_category_id"
        );
        assert_eq!(audit.get_reference_column_name(), "audit_uuid");
    }

    #[test]
    fn internal_schema_update_initializes_missing_schema_and_ignores_non_objects() {
        let mut collection = InternalMemoryCollection::new("items", DbConfig::int("id"));
        collection.schema = None;

        collection.ensure_update_schema_for_item(&json!(42));
        assert!(collection.schema.is_none());

        collection.ensure_update_schema_for_item(&json!({ "id": 1, "name": "Ada" }));
        let schema = collection.schema.unwrap();
        assert!(schema.fields.contains_key("name"));
    }

    #[test]
    fn internal_filtered_lookup_skips_non_objects_missing_columns_and_mismatches() {
        let db = Db::new_with_config(DbConfig::none("id"));
        let mut collection = InternalMemoryCollection::new("items", DbConfig::none("id"));
        let book = add_item(
            &mut collection,
            json!({ "id": "a", "kind": "book", "title": "SQL" }),
        );
        let game = add_item(
            &mut collection,
            json!({ "id": "b", "kind": "game", "title": "Rust" }),
        );
        assert_eq!(book["id"], "a");
        assert_eq!(game["id"], "b");
        collection
            .collection
            .insert("raw".to_string(), json!("not an object"));

        let matches = collection
            .get_filtered_by_columns_values(
                vec![ColumnValue::new("kind".to_string(), json!("book"))],
                ExpansionChain::None,
                &db,
            )
            .unwrap();
        let missing = collection
            .get_filtered_by_columns_values(
                vec![ColumnValue::new("missing".to_string(), json!("book"))],
                ExpansionChain::None,
                &db,
            )
            .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0]["title"], "SQL");
        assert!(missing.is_empty());
    }

    #[test]
    fn public_collection_wrappers_cover_basic_read_write_paths() {
        let collection = DbCollection::new_coll("people", DbConfig::int("id"));
        let inserted = collection
            .add(json!({ "name": "Ada" }))
            .unwrap_or_else(|error| panic!("initial public row should insert: {error}"));
        assert_eq!(inserted["id"], 1);

        assert_eq!(collection.get("1").unwrap().unwrap()["name"], "Ada");
        assert!(collection.exists("1").unwrap());
        assert_eq!(collection.count().unwrap(), 1);
        assert_eq!(collection.get_all().unwrap().len(), 1);
        assert_eq!(collection.get_paginated(0, 1).unwrap().len(), 1);

        let updated = collection
            .update("1", json!({ "name": "Ada Lovelace" }))
            .unwrap()
            .unwrap();
        assert_eq!(updated["id"], 1);
        assert_eq!(updated["name"], "Ada Lovelace");

        let partial = collection
            .update_partial("1", json!({ "profile": { "city": "London" } }))
            .unwrap()
            .unwrap();
        assert_eq!(partial["id"], 1);
        assert_eq!(partial["profile"]["city"], "London");

        assert_eq!(
            collection.delete("1").unwrap().unwrap()["name"],
            "Ada Lovelace"
        );
        assert_eq!(collection.clear().unwrap(), 0);
    }

    #[test]
    fn public_read_methods_return_error_when_lock_is_poisoned() {
        let collection = DbCollection::new_coll("people", DbConfig::int("id"));
        let db = Db::new_with_config(DbConfig::int("id"));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = collection.collection.write().unwrap();
            panic!("poison collection lock");
        }));

        assert_eq!(
            collection.get_reference_column_name().unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection.get_all().unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection.get_paginated(0, 1).unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection.get("1").unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection.exists("1").unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection.count().unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection
                .expand_row(&json!({ "id": 1 }), "", &db)
                .unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection
                .expand_list(vec![json!({ "id": 1 })], "", &db)
                .unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection.schema().unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection.get_name().unwrap_err(),
            CollectionReadError::LockPoisoned
        );
        assert_eq!(
            collection.get_config().unwrap_err(),
            CollectionReadError::LockPoisoned
        );
    }

    #[test]
    fn public_write_methods_return_error_when_lock_is_poisoned() {
        let collection = DbCollection::new_coll("people", DbConfig::int("id"));
        let temp_dir = TempDir::new().unwrap();
        let output = temp_dir
            .path()
            .join("output.json")
            .as_os_str()
            .to_os_string();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = collection.collection.write().unwrap();
            panic!("poison collection lock");
        }));

        assert_eq!(
            collection.add(json!({ "name": "Ada" })).unwrap_err(),
            AddError::LockPoisoned
        );
        assert_eq!(
            collection.add_batch(json!([])).unwrap_err(),
            AddBatchError::LockPoisoned
        );
        assert_eq!(
            collection.load_from_json(json!([]), true).unwrap_err(),
            LoadCollectionError::LockPoisoned
        );
        assert_eq!(
            collection
                .load_from_file(&OsString::from("missing.json"))
                .unwrap_err(),
            LoadCollectionError::LockPoisoned
        );
        assert_eq!(
            collection.update("1", json!({ "id": 1 })).unwrap_err(),
            CollectionWriteError::LockPoisoned
        );
        assert_eq!(
            collection
                .update_partial("1", json!({ "name": "Ada" }))
                .unwrap_err(),
            CollectionWriteError::LockPoisoned
        );
        assert_eq!(
            collection.delete("1").unwrap_err(),
            CollectionWriteError::LockPoisoned
        );
        assert_eq!(
            collection.clear().unwrap_err(),
            CollectionWriteError::LockPoisoned
        );
        assert_eq!(
            collection.set_schema(SchemaDict::default()).unwrap_err(),
            CollectionWriteError::LockPoisoned
        );
        match collection.write_to_file(&output).unwrap_err() {
            WriteCollectionError::LockPoisoned => {}
            other => panic!("expected lock poisoned write error, got {other}"),
        }
    }

    #[test]
    fn add_rejects_non_object_items() {
        let collection = DbCollection::new_coll("people", DbConfig::int("id"));

        assert_eq!(
            collection.add(json!("bad")).unwrap_err(),
            AddError::NonObjectItem
        );
    }

    #[test]
    fn add_reports_missing_and_duplicate_caller_managed_ids() {
        let collection = DbCollection::new_coll("people", DbConfig::none("id"));

        assert_eq!(
            collection.add(json!({ "name": "Ada" })).unwrap_err(),
            AddError::MissingId {
                id_key: "id".to_string()
            }
        );

        collection
            .add(json!({ "id": "ada", "name": "Ada" }))
            .unwrap_or_else(|error| panic!("initial caller-managed row should insert: {error}"));
        assert_eq!(
            collection
                .add(json!({ "id": "ada", "name": "Replacement" }))
                .unwrap_err(),
            AddError::DuplicateId {
                id: "ada".to_string()
            }
        );
    }

    #[test]
    fn add_batch_reports_non_object_and_invalid_integer_ids() {
        let mut collection = create_test_collection();

        assert_eq!(
            collection
                .add_batch(json!([{ "name": "Ada" }, "bad"]))
                .unwrap_err(),
            AddBatchError::NonObjectItem { index: 1 }
        );
        assert_eq!(collection.count(), 1);

        let mut collection = create_test_collection();
        assert_eq!(
            collection
                .add_batch(json!([{ "id": -1, "name": "Ada" }]))
                .unwrap_err(),
            AddBatchError::InvalidIntId { index: 0 }
        );
        assert_eq!(collection.count(), 0);
    }

    #[test]
    fn load_from_json_reports_batch_errors() {
        let collection = DbCollection::new_coll("people", DbConfig::none("id"));

        assert_eq!(
            collection
                .load_from_json(json!([{ "name": "Ada" }]), true)
                .unwrap_err(),
            LoadCollectionError::Batch(AddBatchError::MissingId {
                index: 0,
                id_key: "id".to_string()
            })
        );
    }

    #[test]
    fn load_from_json_reports_non_object_batch_errors() {
        let collection = DbCollection::new_coll("people", DbConfig::int("id"));

        assert_eq!(
            collection
                .load_from_json(json!([{ "name": "Ada" }, "bad"]), true)
                .unwrap_err(),
            LoadCollectionError::Batch(AddBatchError::NonObjectItem { index: 1 })
        );
    }

    #[test]
    fn load_from_json_reports_invalid_integer_batch_errors() {
        let collection = DbCollection::new_coll("people", DbConfig::int("id"));

        assert_eq!(
            collection
                .load_from_json(json!([{ "id": -1, "name": "Ada" }]), true)
                .unwrap_err(),
            LoadCollectionError::Batch(AddBatchError::InvalidIntId { index: 0 })
        );
    }

    #[test]
    fn write_to_file_reports_file_creation_errors() {
        let collection = DbCollection::new_coll("people", DbConfig::int("id"));
        let temp_dir = TempDir::new().unwrap();
        let error = collection
            .write_to_file(&temp_dir.path().as_os_str().to_os_string())
            .unwrap_err();

        match error {
            WriteCollectionError::FileCreate { path, source } => {
                assert_eq!(path, temp_dir.path().to_string_lossy());
                assert_eq!(source.kind(), std::io::ErrorKind::IsADirectory);
            }
            other => panic!("expected file creation error, got {other}"),
        }
    }

    #[test]
    fn write_collection_data_to_writer_reports_serialization_errors() {
        struct FailingWriter;

        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("write failed"))
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let mut writer = FailingWriter;
        let error =
            write_collection_data_to_writer(&mut writer, &[json!({ "id": 1 })]).unwrap_err();

        assert!(matches!(error, WriteCollectionError::Serialize { .. }));
        assert!(error.source().is_some());
    }

    #[test]
    fn public_collection_schema_file_wrapper_reports_success() {
        use std::{ffi::OsString, fs::File, io::Write};
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("people.schema.json");
        File::create(&path)
            .unwrap()
            .write_all(
                json!({
                    "person_id": "Id",
                    "name": "String!"
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap();

        let collection = DbCollection::new_coll("people", DbConfig::int("person_id"));
        let status = collection
            .load_schema_from_file(&OsString::from(path.to_string_lossy().into_owned()))
            .unwrap();

        assert!(status.contains("Loaded schema for collection people"));
        assert!(
            collection
                .schema()
                .unwrap()
                .unwrap()
                .fields
                .contains_key("name")
        );
    }
}
