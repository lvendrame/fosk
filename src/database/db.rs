use std::{collections::HashMap, sync::{Arc, RwLock}};

use crate::{database::{Config, DbCollection, SchemaProvider}, executor::plan_executor::{Executor, PlanExecutor}, parser::{aggregators_helper::AggregateRegistry, analyzer::{AnalysisContext, AnalyzerError}, ast::Query}, planner::plan_builder::PlanBuilder};

/// Thread-safe pointer to the internal database state.
pub type ProtectedDb = Arc<RwLock<InternalDb>>;

/// Internal database holding configuration and named collections.
#[derive(Default)]
pub struct InternalDb {
    config: Config,
    collections: HashMap<String, Arc<DbCollection>>,
}

impl InternalDb {

    /// Convert the internal DB into a thread-safe `ProtectedDb`.
    pub fn into_protected(self) -> ProtectedDb {
        Arc::new(RwLock::new(self))
    }

    fn new_db() -> Self {
        Self::new_db_with_config(Config::default())
    }

    fn new_db_with_config(config: Config) -> Self {
        Self {
            config,
            collections: HashMap::new(),
        }
    }

    /// Create or register a new collection using the DB's default `Config`.
    pub fn create(&mut self, coll_name: &str) -> Arc<DbCollection> {
        self.create_with_config(coll_name, self.config.clone())
    }

    /// Create or register a new collection with a specific `Config`.
    pub fn create_with_config(&mut self, coll_name: &str, config: Config) -> Arc<DbCollection> {
        let collection = Arc::new(DbCollection::new_coll(coll_name, config));
        self.collections.insert(coll_name.to_string(), Arc::clone(&collection));

        collection
    }

    /// Get a shared handle to a collection by name.
    pub fn get(&self, col_name: &str) -> Option<Arc<DbCollection>> {
        self.collections.get(col_name).map(Arc::clone)
    }

    /// List collection names registered in this database instance.
    pub fn list_collections(&self) -> Vec<String> {
        self.collections.keys().cloned().collect::<Vec<_>>()
    }

}

/// Public database handle exposing higher-level APIs.
pub struct Db {
    /// Internal protected database state
    pub internal_db: ProtectedDb,
}

impl Db {

    /// Create a new in-memory database with default configuration.
    pub fn new_db() -> Self {
        Self{
            internal_db: InternalDb::new_db().into_protected(),
        }
    }

    /// Create a new in-memory database with an explicit `Config`.
    pub fn new_db_with_config(config: Config) -> Self {
        Self{
            internal_db: InternalDb::new_db_with_config(config).into_protected(),
        }
    }

    /// Create a collection using the DB's internal lock (concurrent-safe).
    pub fn create(&self, coll_name: &str) -> Arc<DbCollection> {
        self.internal_db.write().unwrap().create(coll_name)
    }

    /// Create a collection with explicit `Config` using the DB's internal lock.
    pub fn create_with_config(&self, coll_name: &str, config: Config) -> Arc<DbCollection> {
        self.internal_db.write().unwrap().create_with_config(coll_name, config)
    }

    /// Get a shared handle to a collection by name.
    pub fn get(&self, col_name: &str) -> Option<Arc<DbCollection>> {
        self.internal_db.read().unwrap().get(col_name)
    }

    /// List registered collection names.
    pub fn list_collections(&self) -> Vec<String> {
        self.internal_db.read().unwrap().list_collections()
    }

    /// Execute a SQL query through the parser, analyzer, planner and executor.
    ///
    /// Returns a vector of JSON `Value` rows on success or an `AnalyzerError`.
    pub fn query(&self, sql: &str) -> Result<Vec<serde_json::Value>, AnalyzerError> {
        // 1) Parse
        let q = Query::try_from(sql)
            .map_err(|e| AnalyzerError::Other(format!("parse error: {e}")))?;

        // 2) Analyze (Db implements SchemaProvider)
        let aggregates = AggregateRegistry::default_aggregate_registry();
        let analyzed = AnalysisContext::analyze_query(&q, self, &aggregates)?;

        // 3) Plan
        let plan = PlanBuilder::from_analyzed(&analyzed)?;

        // 4) Execute
        let exec = PlanExecutor::new(plan);
        exec.execute(self)
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
    use serde_json::json;
    use crate::database::{Config, IdType};

    fn mk_db() -> Db {
        let db = Db::new_db_with_config(Config { id_type: IdType::None, id_key: "id".into() });
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
}
