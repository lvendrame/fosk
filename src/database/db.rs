use std::{
    collections::HashMap,
    ffi::OsString,
    fs,
    io::BufWriter,
    sync::{Arc, RwLock},
};

use serde_json::{Map, Value};

use crate::{
    database::{
        DbCollection, DbConfig, DbReferences, ReferenceColumn, ReferenceFieldMap, SchemaProvider,
        SchemaWithRefs, apply_schema_to_collection, collection_name_from_file_stem,
        config_for_missing_collection, parse_schema_for_load, read_schema_json_file,
    },
    executor::plan_executor::{Executor, PlanExecutor},
    parser::{
        aggregators_helper::AggregateRegistry,
        analyzer::{AnalysisContext, AnalyzerError},
        ast::Query,
    },
    planner::plan_builder::PlanBuilder,
};

/// Thread-safe pointer to the internal database state.
pub(crate) type ProtectedDb = Arc<RwLock<InternalDb>>;

/// Internal database holding configuration and named collections.
#[derive(Default)]
pub(crate) struct InternalDb {
    config: DbConfig,
    collections: HashMap<String, Arc<DbCollection>>,
    pub(crate) reference_manager: Arc<RwLock<DbReferences>>,
}

impl InternalDb {
    /// Convert the internal DB into a thread-safe `ProtectedDb`.
    pub fn into_protected(self) -> ProtectedDb {
        Arc::new(RwLock::new(self))
    }

    fn new_db() -> Self {
        Self::new_db_with_config(DbConfig::default())
    }

    fn new_db_with_config(config: DbConfig) -> Self {
        Self {
            config,
            collections: HashMap::new(),
            reference_manager: Arc::new(RwLock::new(DbReferences::default())),
        }
    }

    /// Create or register a new collection using the DB's default `Config`.
    pub fn create(&mut self, coll_name: &str) -> Arc<DbCollection> {
        self.create_with_config(coll_name, self.config.clone())
    }

    /// Create or register a new collection with a specific `Config`.
    pub fn create_with_config(&mut self, coll_name: &str, config: DbConfig) -> Arc<DbCollection> {
        let collection = Arc::new(DbCollection::new_coll(coll_name, config));
        self.collections
            .insert(coll_name.to_ascii_lowercase(), Arc::clone(&collection));

        collection
    }

    /// Get a shared handle to a collection by name.
    pub fn get(&self, col_name: &str) -> Option<Arc<DbCollection>> {
        self.collections
            .get(&col_name.to_ascii_lowercase())
            .map(Arc::clone)
    }

    /// List collection names registered in this database instance.
    pub fn list_collections(&self) -> Vec<String> {
        self.collections.keys().cloned().collect::<Vec<_>>()
    }

    /// Remove a collection from the database.
    pub fn drop_collection(&mut self, col_name: &str) -> bool {
        self.collections
            .remove(&col_name.to_ascii_lowercase())
            .is_some()
    }

    /// Remove all collections.
    pub fn clear(&mut self) {
        self.collections.clear()
    }

    pub fn load_from_json(&mut self, json_value: Value, keep: bool) -> Result<usize, String> {
        // Guard: Check if it's a JSON Object
        let Value::Object(object) = json_value else {
            return Err("Informed JSON does not contain a JSON object in the root".to_string());
        };

        let mut total = 0;
        for (name, items) in object {
            let collection = self.create(&name);
            collection.load_from_json(items, keep)?;
            total += 1;
        }

        Ok(total)
    }

    pub fn load_from_file(&mut self, file_path: &OsString) -> Result<String, String> {
        let file_path_lossy = file_path.to_string_lossy();

        // Guard: Try to read the file content
        let file_content = fs::read_to_string(file_path)
            .map_err(|_| format!("Could not read file {}", file_path_lossy))?;

        // Guard: Try to parse the content as JSON
        let json_value = serde_json::from_str::<Value>(&file_content)
            .map_err(|_| format!("File {} does not contain valid JSON", file_path_lossy))?;

        match self.load_from_json(json_value, false) {
            Ok(loaded_collections) => Ok(format!(
                "✔️ Loaded {} initial collections from {}",
                loaded_collections, file_path_lossy
            )),
            Err(error) => Err(format!(
                "Error to process the file {}. Details: {}",
                file_path_lossy, error
            )),
        }
    }

    pub fn write_to_json(&self) -> Value {
        let mut collections: Map<String, Value> = Map::new();

        for (name, collection) in &self.collections {
            let values = collection.get_all();
            collections.insert(name.clone(), Value::Array(values));
        }

        Value::Object(collections)
    }

    pub fn write_to_file(&self, file_path: &OsString) -> Result<(), String> {
        let file = std::fs::File::create(file_path).expect("Failed to create json file");
        let mut w = BufWriter::new(file);

        let data = self.write_to_json();
        serde_json::to_writer_pretty(&mut w, &data).expect("Failed to write to a json file");
        Ok(())
    }

    pub fn load_collection_schema_from_json(
        &mut self,
        collection_name: &str,
        json_value: Value,
    ) -> Result<(), String> {
        let parsed = parse_schema_for_load(&json_value)?;
        let collection = match self.get(collection_name) {
            Some(collection) => collection,
            None => {
                let config = config_for_missing_collection(&parsed, &self.config);
                self.create_with_config(collection_name, config)
            }
        };

        apply_schema_to_collection(&collection, parsed)
    }

    pub fn load_schemas_from_json(&mut self, json_value: Value) -> Result<usize, String> {
        let Value::Object(object) = json_value else {
            return Err(
                "Schema JSON must contain an object of collection names to schemas".to_string(),
            );
        };

        let mut total = 0;
        for (collection_name, schema_value) in object {
            self.load_collection_schema_from_json(&collection_name, schema_value)?;
            total += 1;
        }

        Ok(total)
    }
}

/// Public database handle exposing higher-level APIs.
pub struct Db {
    /// Internal protected database state
    pub(crate) internal_db: ProtectedDb,
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}

impl Db {
    /// Create a new in-memory database with the default configuration.
    ///
    /// The default configuration uses UUID ids stored under the `"id"` field.
    /// Collections created with [`Db::create`] inherit this configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, IdType};
    ///
    /// let db = Db::new();
    ///
    /// assert_eq!(db.get_config().id_type, IdType::Uuid);
    /// assert_eq!(db.get_config().id_key, "id");
    /// ```
    pub fn new() -> Self {
        Self {
            internal_db: InternalDb::new_db().into_protected(),
        }
    }

    /// Create a new reference-counted database handle.
    ///
    /// Use this helper when multiple owners need to share the same database
    /// handle without wrapping it in [`Arc`] manually.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::Db;
    /// use std::sync::Arc;
    ///
    /// let db = Db::new_arc();
    /// let cloned = Arc::clone(&db);
    ///
    /// cloned.create("people");
    /// assert!(db.get("people").is_some());
    /// ```
    pub fn new_arc() -> Arc<Self> {
        Arc::new(Self {
            internal_db: InternalDb::new_db().into_protected(),
        })
    }

    /// Create a new in-memory database with an explicit [`DbConfig`].
    ///
    /// The database-level configuration is copied into collections created
    /// with [`Db::create`]. Use [`Db::create_with_config`] when a single
    /// collection needs different id behavior.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig, IdType};
    ///
    /// let db = Db::new_with_config(DbConfig::int("id"));
    ///
    /// assert_eq!(db.get_config().id_type, IdType::Int);
    /// ```
    pub fn new_with_config(config: DbConfig) -> Self {
        Self {
            internal_db: InternalDb::new_db_with_config(config).into_protected(),
        }
    }

    /// Create or replace a collection using the database default configuration.
    ///
    /// Collection names are stored case-insensitively, so later calls to
    /// [`Db::get`] can use any casing. If a collection with the same name
    /// already exists, the new empty collection replaces it.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::int("id"));
    /// let people = db.create("People");
    ///
    /// let inserted = people.add(json!({ "name": "Ada" })).unwrap();
    ///
    /// assert_eq!(inserted["id"], 1);
    /// assert!(db.get("people").is_some());
    /// ```
    pub fn create(&self, coll_name: &str) -> Arc<DbCollection> {
        self.internal_db.write().unwrap().create(coll_name)
    }

    /// Create or replace a collection with its own [`DbConfig`].
    ///
    /// This is useful when most collections use one id strategy, but a
    /// specific collection should generate, store, or require ids differently.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new();
    /// let logs = db.create_with_config("logs", DbConfig::none("key"));
    ///
    /// assert!(logs.add(json!({ "key": "startup", "ok": true })).is_some());
    /// assert!(logs.add(json!({ "ok": false })).is_none());
    /// ```
    pub fn create_with_config(&self, coll_name: &str, config: DbConfig) -> Arc<DbCollection> {
        self.internal_db
            .write()
            .unwrap()
            .create_with_config(coll_name, config)
    }

    /// Get a collection by name.
    ///
    /// Name lookup is case-insensitive. The returned handle points to the
    /// existing collection and can be used for reads, writes, loads, and
    /// schema inspection.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::Db;
    ///
    /// let db = Db::new();
    /// db.create("People");
    ///
    /// assert!(db.get("people").is_some());
    /// assert!(db.get("missing").is_none());
    /// ```
    pub fn get(&self, col_name: &str) -> Option<Arc<DbCollection>> {
        self.internal_db.read().unwrap().get(col_name)
    }

    /// List registered collection names.
    ///
    /// Names are returned in their internal lowercase form.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::Db;
    ///
    /// let db = Db::new();
    /// db.create("People");
    /// db.create("Orders");
    ///
    /// let mut names = db.list_collections();
    /// names.sort();
    ///
    /// assert_eq!(names, vec!["orders", "people"]);
    /// ```
    pub fn list_collections(&self) -> Vec<String> {
        self.internal_db.read().unwrap().list_collections()
    }

    /// Remove a collection from the database.
    ///
    /// Returns `true` when a collection existed and was removed, or `false`
    /// when no collection with that name was registered.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::Db;
    ///
    /// let db = Db::new();
    /// db.create("People");
    ///
    /// assert!(db.drop_collection("people"));
    /// assert!(!db.drop_collection("people"));
    /// ```
    pub fn drop_collection(&self, col_name: &str) -> bool {
        self.internal_db.write().unwrap().drop_collection(col_name)
    }

    /// Remove all collections from the database.
    ///
    /// This clears collection registration but does not change the database
    /// default configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::Db;
    ///
    /// let db = Db::new();
    /// db.create("people");
    /// db.create("orders");
    ///
    /// db.clear();
    ///
    /// assert!(db.list_collections().is_empty());
    /// ```
    pub fn clear(&self) {
        self.internal_db.write().unwrap().clear();
    }

    /// Return the database-level default configuration used for new collections.
    ///
    /// The returned value is a clone. Mutating it will not affect the database;
    /// create a new database or use [`Db::create_with_config`] for a different
    /// collection configuration.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    ///
    /// let db = Db::new_with_config(DbConfig::int("id"));
    ///
    /// assert_eq!(db.get_config(), DbConfig::int("id"));
    /// ```
    pub fn get_config(&self) -> DbConfig {
        self.internal_db.read().unwrap().config.clone()
    }

    /// Load multiple collections from a JSON object.
    ///
    /// The root value must be an object whose keys are collection names and
    /// whose values are arrays of documents. Returns the number of collections
    /// processed. If `keep` is `true`, each loaded document keeps its existing
    /// id where possible; otherwise ids may be regenerated according to each
    /// collection's configuration.
    ///
    /// # Errors
    ///
    /// Returns an error string when the root value is not an object or when a
    /// collection value cannot be loaded as an array.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::none("id"));
    ///
    /// let loaded = db
    ///     .load_from_json(json!({
    ///         "people": [
    ///             { "id": 1, "name": "Ada" },
    ///             { "id": 2, "name": "Grace" }
    ///         ]
    ///     }), true)
    ///     .unwrap();
    ///
    /// assert_eq!(loaded, 1);
    /// assert_eq!(db.get("people").unwrap().count(), 2);
    /// ```
    pub fn load_from_json(&self, json_value: Value, keep: bool) -> Result<usize, String> {
        self.internal_db
            .write()
            .unwrap()
            .load_from_json(json_value, keep)
    }

    /// Load multiple collections from a JSON file.
    ///
    /// The file must contain the same object shape accepted by
    /// [`Db::load_from_json`]. Returns a human-readable success message.
    ///
    /// # Errors
    ///
    /// Returns an error string when the file cannot be read, cannot be parsed
    /// as JSON, or does not contain loadable collections.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use fosk::Db;
    /// use std::ffi::OsString;
    ///
    /// let db = Db::new();
    /// let status = db.load_from_file(&OsString::from("seed.json"))?;
    ///
    /// println!("{status}");
    /// # Ok::<(), String>(())
    /// ```
    pub fn load_from_file(&self, file_path: &OsString) -> Result<String, String> {
        self.internal_db.write().unwrap().load_from_file(file_path)
    }

    /// Load one collection schema from a compact JSON object.
    ///
    /// `collection_name` is required because a raw [`serde_json::Value`] has no
    /// filename or root key from which a single collection name can be inferred.
    /// Missing collections are created with an ID marker-derived config when
    /// present, otherwise with the DB default config. After a successful load,
    /// references are inferred across registered collections.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, JsonPrimitive};
    /// use serde_json::json;
    ///
    /// let db = Db::new();
    /// db.load_collection_schema_from_json(
    ///     "people",
    ///     json!({ "person_id": "Id", "name": "String!" })
    /// )
    /// .unwrap();
    ///
    /// let schema = db.get("people").unwrap().schema().unwrap();
    /// assert_eq!(schema.fields["person_id"].ty, JsonPrimitive::Int);
    /// ```
    pub fn load_collection_schema_from_json(
        &self,
        collection_name: &str,
        json_value: Value,
    ) -> Result<(), String> {
        self.internal_db
            .write()
            .unwrap()
            .load_collection_schema_from_json(collection_name, json_value)?;
        self.infer_all_references();
        Ok(())
    }

    /// Load one collection schema from a JSON file.
    ///
    /// The collection name is inferred from the file stem. For example,
    /// `people.json` loads the schema into the `people` collection. After a
    /// successful load, references are inferred across registered collections.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use fosk::Db;
    /// use std::ffi::OsString;
    ///
    /// let db = Db::new();
    /// let status = db.load_collection_schema_from_file(&OsString::from("people.json"))?;
    ///
    /// println!("{status}");
    /// # Ok::<(), String>(())
    /// ```
    pub fn load_collection_schema_from_file(&self, file_path: &OsString) -> Result<String, String> {
        let collection_name = collection_name_from_file_stem(file_path)?;
        let json_value = read_schema_json_file(file_path)?;
        self.load_collection_schema_from_json(&collection_name, json_value)?;
        Ok(format!(
            "Loaded schema for collection {} from {}",
            collection_name,
            file_path.to_string_lossy()
        ))
    }

    /// Load multiple collection schemas from a compact DB schema JSON object.
    ///
    /// The root object keys are collection names and values are compact
    /// collection schemas. After a successful load, references are inferred
    /// across registered collections.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::Db;
    /// use serde_json::json;
    ///
    /// let db = Db::new();
    /// let loaded = db.load_schemas_from_json(json!({
    ///     "users": { "user_id": "Id", "name": "String!" },
    ///     "orders": { "id": "Id", "user_id": "Int!" }
    /// })).unwrap();
    ///
    /// assert_eq!(loaded, 2);
    /// assert!(db.get_collection_column_ref("orders", "user_id").is_some());
    /// ```
    pub fn load_schemas_from_json(&self, json_value: Value) -> Result<usize, String> {
        let loaded = self
            .internal_db
            .write()
            .unwrap()
            .load_schemas_from_json(json_value)?;
        self.infer_all_references();
        Ok(loaded)
    }

    /// Load multiple collection schemas from a JSON file.
    ///
    /// The file must contain the same object shape accepted by
    /// [`Db::load_schemas_from_json`]. After a successful load, references are
    /// inferred across registered collections.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use fosk::Db;
    /// use std::ffi::OsString;
    ///
    /// let db = Db::new();
    /// let status = db.load_schemas_from_file(&OsString::from("schemas.json"))?;
    ///
    /// println!("{status}");
    /// # Ok::<(), String>(())
    /// ```
    pub fn load_schemas_from_file(&self, file_path: &OsString) -> Result<String, String> {
        let json_value = read_schema_json_file(file_path)?;
        let loaded = self.load_schemas_from_json(json_value)?;
        Ok(format!(
            "Loaded {loaded} collection schemas from {}",
            file_path.to_string_lossy()
        ))
    }

    /// Serialize all collections to a JSON object.
    ///
    /// The returned value uses collection names as object keys and arrays of
    /// stored documents as values.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::none("id"));
    /// db.create("people").add(json!({ "id": 1, "name": "Ada" }));
    ///
    /// let dump = db.write_to_json();
    ///
    /// assert_eq!(dump["people"][0]["name"], "Ada");
    /// ```
    pub fn write_to_json(&self) -> Value {
        self.internal_db.read().unwrap().write_to_json()
    }

    /// Serialize all collections to a pretty-printed JSON file.
    ///
    /// The output format matches [`Db::write_to_json`].
    ///
    /// # Errors
    ///
    /// Returns an error only if serialization fails after the file is created.
    /// File creation currently panics if the path cannot be opened.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use fosk::Db;
    /// use std::ffi::OsString;
    ///
    /// let db = Db::new();
    ///
    /// db.write_to_file(&OsString::from("collections.json"))?;
    /// # Ok::<(), String>(())
    /// ```
    pub fn write_to_file(&self, file_path: &OsString) -> Result<(), String> {
        self.internal_db.read().unwrap().write_to_file(file_path)
    }

    /// Execute a SQL query through the parser, analyzer, planner and executor.
    ///
    /// Use this when the SQL has no positional parameters. The result is a
    /// vector of JSON object rows containing the selected fields.
    ///
    /// # Errors
    ///
    /// Returns an [`AnalyzerError`] when parsing, name resolution, planning,
    /// or execution fails.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::int("id"));
    /// let people = db.create("people");
    ///
    /// people.add(json!({ "name": "Ada", "age": 37 }));
    /// people.add(json!({ "name": "Grace", "age": 29 }));
    ///
    /// let rows = db
    ///     .query("SELECT name FROM people WHERE age > 30")
    ///     .unwrap();
    ///
    /// assert_eq!(rows[0]["name"], "Ada");
    /// ```
    pub fn query(&self, sql: &str) -> Result<Vec<serde_json::Value>, AnalyzerError> {
        // 1) Parse
        let q =
            Query::try_from(sql).map_err(|e| AnalyzerError::Other(format!("parse error: {e}")))?;

        // 2) Analyze (Db implements SchemaProvider)
        let aggregates = AggregateRegistry::default_aggregate_registry();
        let analyzed = AnalysisContext::analyze_query(&q, self, &aggregates, Value::Null)?;

        // 3) Plan
        let plan = PlanBuilder::from_analyzed(&analyzed)?;

        // 4) Execute
        let exec = PlanExecutor::new(plan);
        exec.execute(self)
    }

    /// Execute a SQL query through the parser, analyzer, planner and executor.
    ///
    /// Positional `?` placeholders are filled from `args`. Pass a single JSON
    /// value for one placeholder, or a JSON array for multiple placeholders.
    /// Arrays can also be used inside `IN (?)` predicates.
    ///
    /// # Errors
    ///
    /// Returns an [`AnalyzerError`] when parsing, parameter binding, name
    /// resolution, planning, or execution fails.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::none("id"));
    /// db.create("people").add_batch(json!([
    ///     { "id": 1, "name": "Ada" },
    ///     { "id": 2, "name": "Grace" }
    /// ]));
    ///
    /// let rows = db
    ///     .query_with_args(
    ///         "SELECT name FROM people WHERE id IN (?) ORDER BY id",
    ///         json!([[1, 2]])
    ///     )
    ///     .unwrap();
    ///
    /// assert_eq!(rows.len(), 2);
    /// ```
    pub fn query_with_args(&self, sql: &str, args: Value) -> Result<Vec<Value>, AnalyzerError> {
        // 1) Parse
        let q =
            Query::try_from(sql).map_err(|e| AnalyzerError::Other(format!("parse error: {e}")))?;

        // 2) Analyze (Db implements SchemaProvider)
        let aggregates = AggregateRegistry::default_aggregate_registry();
        let analyzed = AnalysisContext::analyze_query(&q, self, &aggregates, args)?;

        // 3) Plan
        let plan = PlanBuilder::from_analyzed(&analyzed)?;

        // 4) Execute
        let exec = PlanExecutor::new(plan);
        exec.execute(self)
    }

    /// Declare a bidirectional relationship between two collections.
    ///
    /// Creates a reference from `collection_name.column` to `ref_collection_name.ref_column`
    /// and also registers the inverse (referrer) side. Returns `true` if both mappings succeed.
    ///
    /// References are used by [`DbCollection::expand_row`] and
    /// [`DbCollection::expand_list`] to include related records.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::none("id"));
    /// db.create("people").add(json!({ "id": 1, "name": "Ada" }));
    /// db.create("orders").add(json!({ "id": 10, "person_id": 1 }));
    ///
    /// assert!(db.create_reference("orders", "person_id", "people", "id"));
    /// assert!(db.get_collection_column_ref("orders", "person_id").is_some());
    /// ```
    pub fn create_reference(
        &self,
        collection_name: &str,
        column: &str,
        ref_collection_name: &str,
        ref_column: &str,
    ) -> bool {
        let rm = self.internal_db.read().unwrap().reference_manager.clone();
        let mut rm = rm.write().unwrap();
        rm.create_reference(
            self,
            collection_name,
            column,
            ref_collection_name,
            ref_column,
        )
    }

    /// Infer and register a foreign-key-like reference automatically based on default conventions.
    ///
    /// Attempts to link `collection_name` to `ref_collection_name` by matching the latter's
    /// reference column name and its primary key. Returns `true` if successful.
    ///
    /// For a referenced collection named `people` with id key `"id"`, the
    /// inferred local reference field is `"people_id"`.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::none("id"));
    /// db.create("people").add(json!({ "id": 1, "name": "Ada" }));
    /// db.create("orders").add(json!({ "id": 10, "people_id": 1 }));
    ///
    /// assert!(db.infer_reference("orders", "people"));
    /// ```
    pub fn infer_reference(&self, collection_name: &str, ref_collection_name: &str) -> bool {
        let rm = self.internal_db.read().unwrap().reference_manager.clone();
        let mut rm = rm.write().unwrap();
        rm.infer_reference(self, collection_name, ref_collection_name)
    }

    /// Retrieve all reference mappings defined for a collection.
    ///
    /// Returns a `HashMap` of field names to `ReferenceColumn` entries if any exist.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::none("id"));
    /// db.create("people").add(json!({ "id": 1 }));
    /// db.create("orders").add(json!({ "id": 10, "person_id": 1 }));
    /// db.create_reference("orders", "person_id", "people", "id");
    ///
    /// let refs = db.get_collection_refs("orders").unwrap();
    ///
    /// assert!(refs.contains_key("person_id"));
    /// ```
    pub fn get_collection_refs(&self, collection_name: &str) -> Option<ReferenceFieldMap> {
        let rm = self.internal_db.read().unwrap().reference_manager.clone();
        let rm = rm.read().unwrap();
        rm.get_collection_refs(collection_name).cloned()
    }

    /// Retrieve the reference mapping for a specific field in a collection.
    ///
    /// Returns the `ReferenceColumn` if a reference was defined on `collection_name.column`.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::none("id"));
    /// db.create("people").add(json!({ "id": 1 }));
    /// db.create("orders").add(json!({ "id": 10, "person_id": 1 }));
    /// db.create_reference("orders", "person_id", "people", "id");
    ///
    /// let reference = db
    ///     .get_collection_column_ref("orders", "person_id")
    ///     .unwrap();
    ///
    /// assert_eq!(reference.ref_collection, "people");
    /// ```
    pub fn get_collection_column_ref(
        &self,
        collection_name: &str,
        column: &str,
    ) -> Option<ReferenceColumn> {
        let rm = self.internal_db.read().unwrap().reference_manager.clone();
        let rm = rm.read().unwrap();
        rm.get_collection_column_ref(collection_name, column)
            .cloned()
    }

    /// Return inferred schema metadata plus inbound and outbound references.
    ///
    /// Returns `None` when the collection does not exist or no schema has been
    /// inferred yet. A schema is inferred as documents are added or loaded.
    ///
    /// # Example
    ///
    /// ```
    /// use fosk::{Db, DbConfig, JsonPrimitive};
    /// use serde_json::json;
    ///
    /// let db = Db::new_with_config(DbConfig::none("id"));
    /// db.create("people").add(json!({ "id": 1, "name": "Ada" }));
    ///
    /// let schema = db.schema_with_refs_of("people").unwrap();
    ///
    /// assert_eq!(schema.name, "people");
    /// assert_eq!(schema.fields["name"].ty, JsonPrimitive::String);
    /// ```
    pub fn schema_with_refs_of(&self, collection_name: &str) -> Option<SchemaWithRefs> {
        let guard = self.internal_db.read().ok()?;
        let coll = guard.get(collection_name)?;
        let schema = coll.schema()?;

        Some(SchemaWithRefs::new(collection_name, &schema, self))
    }

    fn infer_all_references(&self) -> usize {
        let names = self.list_collections();
        let mut inferred = 0;
        for collection_name in &names {
            for ref_collection_name in &names {
                if collection_name == ref_collection_name {
                    continue;
                }
                if self.infer_reference(collection_name, ref_collection_name) {
                    inferred += 1;
                }
            }
        }
        inferred
    }
}

impl SchemaProvider for Db {
    fn schema_of(&self, collection_ref: &str) -> Option<super::SchemaDict> {
        let guard = self.internal_db.read().ok()?;
        let coll = guard.get(collection_ref)?;
        coll.schema()
    }
}

// src/database/db_runner_tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{DbConfig, IdType};
    use serde_json::json;

    fn mk_db() -> Db {
        let db = Db::new_with_config(DbConfig {
            id_type: IdType::None,
            id_key: "id".into(),
        });
        let t = db.create("t");
        t.add_batch(json!([
            { "id": 1, "cat": "a", "amt": 10.0 },
            { "id": 2, "cat": "a", "amt": 15.0 },
            { "id": 3, "cat": "b", "amt":  7.5 },
            { "id": 4, "cat": "b", "amt": null },
            { "id": 5, "cat": "a", "amt": 22.5 }
        ]));
        db
    }

    fn mk_people_order_db() -> Db {
        let db = Db::new_with_config(DbConfig {
            id_type: IdType::None,
            id_key: "id".into(),
        });
        let people = db.create("people");
        people.add_batch(json!([
            { "id": 3, "name": "Carla", "age": 30 },
            { "id": 1, "name": "Ada", "age": 37 },
            { "id": 4, "name": "Grace", "age": 30 },
            { "id": 2, "name": "Bob", "age": 25 }
        ]));
        db
    }

    fn mk_empty_agg_db() -> Db {
        let db = Db::new_with_config(DbConfig {
            id_type: IdType::None,
            id_key: "id".into(),
        });
        let empty = db.create("empty_table");
        empty.add_batch(json!([
            { "id": 1, "grp": "x", "v": 10.0, "w": 20.0, "sum": "reserved" }
        ]));
        empty.clear();
        db
    }

    fn mk_phase2_agg_db() -> Db {
        let db = Db::new_with_config(DbConfig {
            id_type: IdType::None,
            id_key: "id".into(),
        });
        let metrics = db.create("metrics");
        metrics.add_batch(json!([
            { "id": 1, "grp": "a", "v": 10.0, "w": 1.0, "sum": "z" },
            { "id": 2, "grp": "a", "v": 3.0, "w": 2.0, "sum": "z" },
            { "id": 3, "grp": "b", "v": 5.0, "w": 7.0, "sum": "y" }
        ]));
        db
    }

    fn mk_outer_join_db() -> Db {
        let db = Db::new_with_config(DbConfig {
            id_type: IdType::None,
            id_key: "id".into(),
        });
        let people = db.create("people");
        people.add_batch(json!([
            { "id": 1, "name": "Ada" },
            { "id": 2, "name": "Bob" }
        ]));
        let pets = db.create("pets");
        pets.add_batch(json!([
            { "id": 10, "owner_id": 1, "name": "Milo" },
            { "id": 11, "owner_id": 3, "name": "Ghost" }
        ]));
        db
    }

    fn string_values(rows: &[Value], key: &str) -> Vec<String> {
        rows.iter()
            .map(|row| row[key].as_str().expect("string field").to_string())
            .collect()
    }

    #[test]
    fn db_runner_full_pipeline_group_by_having() {
        let db = mk_db();
        let sql = r#"
            SELECT t.cat AS cat, SUM(t.amt) AS total
            FROM t
            WHERE t.id > 1
            GROUP BY t.cat
            HAVING SUM(t.amt) > 20
            ORDER BY t.cat
            LIMIT 10
        "#;

        let rows = db.query(sql).expect("query should succeed");
        assert_eq!(rows.len(), 1);

        let obj = rows[0].as_object().unwrap();
        assert_eq!(obj.get("cat").unwrap(), "a");
        let total = obj.get("total").unwrap().as_f64().unwrap();
        assert!((total - 37.5).abs() < 1e-9);
    }

    #[test]
    fn db_runner_supports_from_list_cross_join() {
        let db = mk_db();
        // trivial cross join + count(*) just to exercise the path
        let sql = r#"
            SELECT COUNT(*) AS n
            FROM t a, t b
        "#;
        let rows = db.query(sql).expect("query should succeed");
        assert_eq!(rows.len(), 1);
        // t has 5 rows -> t × t has 25 rows
        assert_eq!(rows[0]["n"].as_i64().unwrap(), 25);
    }

    #[test]
    fn db_runner_order_by_aggregate_not_projected() {
        let db = mk_db();
        let rows = db
            .query(
                r#"
                SELECT t.cat AS cat
                FROM t
                GROUP BY t.cat
                ORDER BY SUM(t.amt) DESC
            "#,
            )
            .unwrap();

        assert_eq!(string_values(&rows, "cat"), vec!["a", "b"]);
        assert!(rows.iter().all(|row| row.get("sum").is_none()));
    }

    #[test]
    fn db_runner_order_by_aggregate_not_projected_preserves_projection_alias() {
        let db = mk_db();
        let rows = db
            .query(
                r#"
                SELECT t.cat AS g
                FROM t
                GROUP BY t.cat
                ORDER BY SUM(t.amt) DESC
            "#,
            )
            .unwrap();

        assert_eq!(string_values(&rows, "g"), vec!["a", "b"]);
        assert!(rows.iter().all(|row| row.get("cat").is_none()));
        assert!(rows.iter().all(|row| row.get("sum").is_none()));
    }

    #[test]
    fn db_runner_global_count_star_over_empty_table_returns_one_zero_row() {
        let db = mk_empty_agg_db();
        let rows = db.query("SELECT COUNT(*) AS n FROM empty_table").unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["n"], json!(0));
    }

    #[test]
    fn db_runner_global_sum_over_empty_table_returns_one_null_row() {
        let db = mk_empty_agg_db();
        let rows = db.query("SELECT SUM(v) AS s FROM empty_table").unwrap();

        assert_eq!(rows.len(), 1);
        assert!(rows[0]["s"].is_null());
    }

    #[test]
    fn db_runner_global_avg_min_max_over_empty_table_return_nulls() {
        let db = mk_empty_agg_db();
        let rows = db
            .query("SELECT AVG(v) AS a, MIN(v) AS mn, MAX(v) AS mx FROM empty_table")
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert!(rows[0]["a"].is_null());
        assert!(rows[0]["mn"].is_null());
        assert!(rows[0]["mx"].is_null());
    }

    #[test]
    fn db_runner_empty_grouped_aggregate_returns_zero_rows() {
        let db = mk_empty_agg_db();
        let rows = db
            .query("SELECT grp, COUNT(*) AS n FROM empty_table GROUP BY grp")
            .unwrap();

        assert!(rows.is_empty());
    }

    #[test]
    fn db_runner_multiple_sum_calls_receive_distinct_internal_names() {
        let db = mk_phase2_agg_db();
        let rows = db
            .query("SELECT SUM(v) AS sv, SUM(w) AS sw FROM metrics")
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["sv"], json!(18.0));
        assert_eq!(rows[0]["sw"], json!(10.0));
    }

    #[test]
    fn db_runner_aggregate_name_does_not_collide_with_group_key_named_sum() {
        let db = mk_phase2_agg_db();
        let rows = db
            .query(
                r#"
                SELECT sum AS bucket, SUM(v) AS total
                FROM metrics
                GROUP BY sum
                ORDER BY SUM(v) DESC
            "#,
            )
            .unwrap();

        assert_eq!(string_values(&rows, "bucket"), vec!["z", "y"]);
        assert_eq!(rows[0]["total"], json!(13.0));
        assert_eq!(rows[1]["total"], json!(5.0));
    }

    #[test]
    fn db_runner_having_can_use_hidden_aggregate() {
        let db = mk_phase2_agg_db();
        let rows = db
            .query(
                r#"
                SELECT grp
                FROM metrics
                GROUP BY grp
                HAVING SUM(v) > 6
                ORDER BY grp
            "#,
            )
            .unwrap();

        assert_eq!(string_values(&rows, "grp"), vec!["a"]);
        assert!(rows.iter().all(|row| row.get("sum").is_none()));
    }

    #[test]
    fn db_runner_right_join_emits_unmatched_right_rows_with_null_left_fields() {
        let db = mk_outer_join_db();
        let rows = db
            .query(
                r#"
                SELECT pe.name AS person, p.name AS pet
                FROM people pe
                RIGHT JOIN pets p ON pe.id = p.owner_id
                ORDER BY p.id
            "#,
            )
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["person"], json!("Ada"));
        assert_eq!(rows[0]["pet"], json!("Milo"));
        assert!(rows[1]["person"].is_null());
        assert_eq!(rows[1]["pet"], json!("Ghost"));
    }

    #[test]
    fn db_runner_full_join_emits_matched_and_unmatched_rows() {
        let db = mk_outer_join_db();
        let rows = db
            .query(
                r#"
                SELECT pe.name AS person, p.name AS pet
                FROM people pe
                FULL JOIN pets p ON pe.id = p.owner_id
                ORDER BY pe.id
            "#,
            )
            .unwrap();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["person"], json!("Ada"));
        assert_eq!(rows[0]["pet"], json!("Milo"));
        assert_eq!(rows[1]["person"], json!("Bob"));
        assert!(rows[1]["pet"].is_null());
        assert!(rows[2]["person"].is_null());
        assert_eq!(rows[2]["pet"], json!("Ghost"));
    }

    #[test]
    fn db_runner_order_by_non_projected_column() {
        let db = mk_people_order_db();
        let rows = db.query("SELECT name FROM people ORDER BY age").unwrap();
        let names = string_values(&rows, "name");
        let mut tied = names[1..3].to_vec();
        tied.sort();

        assert_eq!(names.first().unwrap(), "Bob");
        assert_eq!(names.last().unwrap(), "Ada");
        assert_eq!(tied, vec!["Carla", "Grace"]);
        assert!(
            rows.iter().all(|row| row.get("age").is_none()),
            "hidden sort key leaked"
        );
    }

    #[test]
    fn db_runner_order_by_non_projected_column_with_alias_desc() {
        let db = mk_people_order_db();
        let rows = db
            .query("SELECT name AS n FROM people ORDER BY age DESC")
            .unwrap();
        let names = string_values(&rows, "n");
        let mut tied = names[1..3].to_vec();
        tied.sort();

        assert_eq!(names.first().unwrap(), "Ada");
        assert_eq!(names.last().unwrap(), "Bob");
        assert_eq!(tied, vec!["Carla", "Grace"]);
        assert!(
            rows.iter().all(|row| row.get("age").is_none()),
            "hidden sort key leaked"
        );
    }

    #[test]
    fn db_runner_order_by_hidden_key_with_expression_projection() {
        let db = mk_people_order_db();
        let rows = db
            .query("SELECT UPPER(name) AS n FROM people ORDER BY age")
            .unwrap();
        let names = string_values(&rows, "n");
        let mut tied = names[1..3].to_vec();
        tied.sort();

        assert_eq!(names.first().unwrap(), "BOB");
        assert_eq!(names.last().unwrap(), "ADA");
        assert_eq!(tied, vec!["CARLA", "GRACE"]);
        assert!(
            rows.iter().all(|row| row.get("age").is_none()),
            "hidden sort key leaked"
        );
    }

    #[test]
    fn db_runner_order_by_hidden_multi_key() {
        let db = mk_people_order_db();
        let rows = db
            .query("SELECT name FROM people ORDER BY age ASC, id DESC")
            .unwrap();

        assert_eq!(
            string_values(&rows, "name"),
            vec!["Bob", "Grace", "Carla", "Ada"]
        );
        assert!(rows.iter().all(|row| row.get("age").is_none()));
        assert!(rows.iter().all(|row| row.get("id").is_none()));
    }

    #[test]
    fn db_runner_order_by_projected_position_still_works() {
        let db = mk_people_order_db();
        let rows = db.query("SELECT name FROM people ORDER BY 1").unwrap();

        assert_eq!(
            string_values(&rows, "name"),
            vec!["Ada", "Bob", "Carla", "Grace"]
        );
    }

    #[test]
    fn db_runner_order_by_position_out_of_range_errors() {
        let db = mk_people_order_db();
        let err = db.query("SELECT name FROM people ORDER BY 2").unwrap_err();
        let msg = format!("{err:?}").to_lowercase();

        assert!(
            msg.contains("order by position"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn db_runner_order_by_missing_column_errors() {
        let db = mk_people_order_db();
        let err = db
            .query("SELECT name FROM people ORDER BY missing_column")
            .unwrap_err();
        let msg = format!("{err:?}").to_lowercase();

        assert!(
            msg.contains("unknowncolumn") || msg.contains("missing_column"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn db_runner_with_arg() {
        let db = mk_db();
        let sql = r#"
            SELECT id, cat, amt
            FROM t
            WHERE id = ?
        "#;

        let rows = db
            .query_with_args(sql, json!(3))
            .expect("query should succeed");
        assert_eq!(rows.len(), 1);

        let obj = rows[0].as_object().unwrap();
        assert_eq!(obj.get("id").unwrap(), 3);
        assert_eq!(obj.get("cat").unwrap(), "b");
        assert_eq!(obj.get("amt").unwrap(), 7.5);
    }

    #[test]
    fn db_runner_with_args() {
        let db = mk_db();
        let sql = r#"
            SELECT id, cat, amt
            FROM t
            WHERE id IN (?)
            ORDER BY id
        "#;

        let rows = db
            .query_with_args(sql, json!([[2, 3]]))
            .expect("query should succeed");
        assert_eq!(rows.len(), 2);

        let obj = rows[0].as_object().unwrap();
        assert_eq!(obj.get("id").unwrap(), 2);
        assert_eq!(obj.get("cat").unwrap(), "a");
        assert_eq!(obj.get("amt").unwrap(), 15.0);

        let obj = rows[1].as_object().unwrap();
        assert_eq!(obj.get("id").unwrap(), 3);
        assert_eq!(obj.get("cat").unwrap(), "b");
        assert_eq!(obj.get("amt").unwrap(), 7.5);
    }

    #[test]
    fn db_runner_in_with_empty_array_param_returns_no_rows() {
        let db = mk_db();
        // WHERE id IN (?) with [] should match nothing
        let rows = db
            .query_with_args(
                r#"
                SELECT id FROM t
                WHERE id IN (?)
            "#,
                serde_json::json!([[]]),
            )
            .expect("query should succeed");
        assert!(rows.is_empty());
    }

    #[test]
    fn db_runner_multiple_positional_params() {
        let db = mk_db();
        // Two ? scalars, both must be provided in order
        let rows = db
            .query_with_args(
                r#"
                SELECT id, cat
                FROM t
                WHERE id >= ? AND cat = ?
                ORDER BY id
            "#,
                serde_json::json!([2, "a"]),
            )
            .expect("query should succeed");
        // Expect rows with id >= 2 and cat='a' -> ids 2 and 5 in mk_db fixture
        let ids: Vec<i64> = rows.iter().map(|r| r["id"].as_i64().unwrap()).collect();
        assert_eq!(ids, vec![2, 5]);
    }

    #[test]
    fn db_runner_param_in_function_and_order_by() {
        let db = mk_db();
        // Use param inside a scalar function and sort by a projected alias
        let sql = r#"
            SELECT UPPER(cat) AS c
            FROM t
            WHERE cat = ?
            ORDER BY c DESC
        "#;
        let rows = db
            .query_with_args(sql, serde_json::json!("a"))
            .expect("query should succeed");
        // All rows have cat='a' -> UPPER('a') == 'A'
        assert!(!rows.is_empty());
        for r in rows {
            assert_eq!(r["c"], serde_json::json!("A"));
        }
    }

    #[test]
    fn db_runner_in_with_mixed_literals_and_param_array() {
        let db = mk_db();
        // IN list combining literals and a param array: id IN (1, ?)
        // Param expands to Args([2,3]) -> overall set {1,2,3}
        let sql = r#"
            SELECT id
            FROM t
            WHERE id IN (1, ?)
            ORDER BY id
        "#;
        let rows = db
            .query_with_args(sql, serde_json::json!([[2, 3]]))
            .expect("query should succeed");
        let ids: Vec<i64> = rows.iter().map(|r| r["id"].as_i64().unwrap()).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn db_runner_insensitive_case() {
        let db = mk_db();
        // trivial cross join + count(*) just to exercise the path
        let sql = r#"
            SELECT COUNT(*) AS n
            FROM t a, T b
        "#;
        let rows = db.query(sql).expect("query should succeed");
        assert_eq!(rows.len(), 1);
        // t has 5 rows -> t × t has 25 rows
        assert_eq!(rows[0]["n"].as_i64().unwrap(), 25);
    }

    #[test]
    fn test_db_load_from_json() {
        use serde_json::json;

        // Single collection with one item
        let input = json!({ "a": [{ "id": 1, "x": "foo" }] });
        let db = Db::new_with_config(DbConfig::int("id"));
        let count = db.load_from_json(input.clone(), false).unwrap();
        assert_eq!(count, 1);
        // Verify write_to_json reflects same data
        let out = db.write_to_json();
        let arr = out.get("a").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].get("x").unwrap(), "foo");
    }

    #[test]
    fn test_db_load_from_file() {
        use serde_json::json;
        use std::{ffi::OsString, fs::File, io::Write};
        use tempfile::TempDir;

        // Create temp JSON file for loading
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("db.json");
        let mut f = File::create(&path).unwrap();
        let data = json!({ "b": [{ "id": 2, "y": 42 }] });
        f.write_all(data.to_string().as_bytes()).unwrap();

        let os_path = OsString::from(path.to_string_lossy().into_owned());
        let db = Db::new_with_config(DbConfig::int("id"));
        let msg = db.load_from_file(&os_path).unwrap();
        assert!(msg.contains("Loaded 1 initial collections"));
        // Confirm via write_to_json
        let out = db.write_to_json();
        let arr = out.get("b").unwrap().as_array().unwrap();
        assert_eq!(arr[0].get("y").unwrap(), 42);
    }

    #[test]
    fn load_collection_schema_from_json_creates_collection_with_custom_int_id() {
        let db = Db::new();

        db.load_collection_schema_from_json(
            "users",
            json!({
                "user_id": "Id",
                "name": "String!"
            }),
        )
        .unwrap();

        let users = db.get("users").unwrap();
        assert_eq!(users.get_config(), DbConfig::int("user_id"));

        let schema = users.schema().unwrap();
        assert_eq!(schema.fields["user_id"].ty, crate::JsonPrimitive::Int);
        assert!(!schema.fields["user_id"].nullable);
    }

    #[test]
    fn load_schemas_from_json_derives_id_configs_with_different_names() {
        let db = Db::new();

        let loaded = db
            .load_schemas_from_json(json!({
                "users": {
                    "user_id": "Id",
                    "name": "String!"
                },
                "sessions": {
                    "session_uuid": "Uuid",
                    "user_id": "Int!",
                    "token": "String!"
                },
                "legacy": {
                    "external_key": "None:String",
                    "value": "String"
                },
                "numeric_legacy": {
                    "legacy_id": "None:Int",
                    "value": "String"
                }
            }))
            .unwrap();

        assert_eq!(loaded, 4);
        assert_eq!(
            db.get("users").unwrap().get_config(),
            DbConfig::int("user_id")
        );
        assert_eq!(
            db.get("sessions").unwrap().get_config(),
            DbConfig::uuid("session_uuid")
        );
        assert_eq!(
            db.get("legacy").unwrap().get_config(),
            DbConfig::none("external_key")
        );
        assert_eq!(
            db.get("numeric_legacy").unwrap().get_config(),
            DbConfig::none("legacy_id")
        );
    }

    #[test]
    fn load_schema_rejects_conflicting_existing_collection_id_config() {
        let db = Db::new();
        db.create_with_config("users", DbConfig::uuid("user_id"));

        let err = db
            .load_collection_schema_from_json("users", json!({ "user_id": "Id" }))
            .unwrap_err();

        assert!(err.contains("conflicts"));
    }

    #[test]
    fn load_schemas_from_json_infers_references_after_loading() {
        let db = Db::new();

        db.load_schemas_from_json(json!({
            "users": {
                "user_id": "Id",
                "name": "String!"
            },
            "orders": {
                "order_id": "Id",
                "user_id": "Int!",
                "total": "Float!"
            }
        }))
        .unwrap();

        let reference = db.get_collection_column_ref("orders", "user_id").unwrap();
        assert_eq!(reference.ref_collection, "users");
        assert_eq!(reference.ref_column, "user_id");
    }

    #[test]
    fn load_collection_schema_from_file_infers_collection_name_from_file_stem() {
        use std::{ffi::OsString, fs::File, io::Write};
        use tempfile::TempDir;

        let db = Db::new();
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("people.json");
        let mut file = File::create(&path).unwrap();
        file.write_all(
            json!({ "person_id": "Id", "name": "String!" })
                .to_string()
                .as_bytes(),
        )
        .unwrap();

        let status = db
            .load_collection_schema_from_file(&OsString::from(path.to_string_lossy().into_owned()))
            .unwrap();

        assert!(status.contains("people"));
        assert_eq!(
            db.get("people").unwrap().get_config(),
            DbConfig::int("person_id")
        );
    }

    #[test]
    fn test_db_write_to_json() {
        use serde_json::json;

        let db = Db::new_with_config(DbConfig::int("id"));
        let coll = db.create("z");
        coll.add(json!({ "key": "value" }));

        let out = db.write_to_json();
        let arr = out.get("z").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].get("key").unwrap(), "value");
    }

    #[test]
    fn test_db_write_to_file() {
        use serde_json::json;
        use std::{ffi::OsString, fs};
        use tempfile::TempDir;

        // Setup and write to file
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("out.json");
        let os_path = OsString::from(path.to_string_lossy().into_owned());

        let db = Db::new_with_config(DbConfig::int("id"));
        let coll = db.create("c");
        coll.add(json!({ "n": 3 }));
        assert!(db.write_to_file(&os_path).is_ok());

        let content = fs::read_to_string(path).unwrap();
        let v: serde_json::Value = serde_json::from_str(&content).unwrap();
        let arr = v.get("c").unwrap().as_array().unwrap();
        assert_eq!(arr[0].get("n").unwrap(), 3);
    }

    #[test]
    fn public_database_wrappers_cover_lifecycle_and_config_paths() {
        let db = Db::new_with_config(DbConfig::int("row_id"));
        assert_eq!(db.get_config(), DbConfig::int("row_id"));

        db.create("People");
        db.create("Orders");
        let mut names = db.list_collections();
        names.sort();
        assert_eq!(names, vec!["orders", "people"]);

        assert!(db.drop_collection("people"));
        assert!(!db.drop_collection("people"));
        assert!(db.get("people").is_none());

        db.clear();
        assert!(db.list_collections().is_empty());
    }

    #[test]
    fn new_arc_shares_the_same_database_handle() {
        let db = Db::new_arc();
        let cloned = std::sync::Arc::clone(&db);

        cloned.create("people");

        assert!(db.get("people").is_some());
    }

    #[test]
    fn schema_with_refs_of_returns_none_for_missing_or_unschematized_collection() {
        let db = Db::new();

        assert!(db.schema_with_refs_of("missing").is_none());
        db.create("empty");
        assert!(db.schema_with_refs_of("empty").is_some());
    }

    #[test]
    fn load_from_json_and_schema_json_reject_non_object_roots() {
        let db = Db::new();

        assert!(db.load_from_json(json!([]), false).unwrap_err().contains("root"));
        assert!(db
            .load_schemas_from_json(json!([]))
            .unwrap_err()
            .contains("collection names"));
    }

    #[test]
    fn load_from_file_wraps_root_processing_errors() {
        use std::{ffi::OsString, fs::File, io::Write};
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("bad_root.json");
        File::create(&path)
            .unwrap()
            .write_all(json!([]).to_string().as_bytes())
            .unwrap();

        let db = Db::new();
        let err = db
            .load_from_file(&OsString::from(path.to_string_lossy().into_owned()))
            .unwrap_err();

        assert!(err.contains("Error to process the file"));
    }

    #[test]
    fn load_schemas_from_file_reports_success_status() {
        use std::{ffi::OsString, fs::File, io::Write};
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("schemas.json");
        File::create(&path)
            .unwrap()
            .write_all(
                json!({
                    "people": {
                        "person_id": "Id",
                        "name": "String!"
                    }
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap();

        let db = Db::new();
        let status = db
            .load_schemas_from_file(&OsString::from(path.to_string_lossy().into_owned()))
            .unwrap();

        assert!(status.contains("Loaded 1 collection schemas"));
        assert!(db.get("people").is_some());
    }
}
