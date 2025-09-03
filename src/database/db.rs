use std::{collections::HashMap, sync::{Arc, RwLock}};

use crate::{database::{Config, DbCollection, MemoryCollection, SchemaProvider}, executor::plan_executor::{Executor, PlanExecutor}, parser::{aggregators_helper::AggregateRegistry, analyzer::{AnalysisContext, AnalyzerError}, ast::Query}, planner::plan_builder::PlanBuilder};

pub type Db = Arc<RwLock<InternalDb>>;

#[derive(Default)]
pub struct InternalDb {
    config: Config,
    collections: HashMap<String, MemoryCollection>,
}

impl InternalDb {

    pub fn into_protected(self) -> Db {
        Arc::new(RwLock::new(self))
    }
}

pub trait DbCommon {
    fn new_db() -> Self;
    fn new_db_with_config(config: Config) -> Self;
    fn create(&mut self, coll_name: &str) -> MemoryCollection;
    fn create_with_config(&mut self, coll_name: &str, config: Config) -> MemoryCollection;
    fn get(&self, coll_name: &str) -> Option<MemoryCollection>;
    fn list_collections(&self) -> Vec<String>;
}

impl InternalDb {

    fn new_db() -> Self {
        Self::new_db_with_config(Config::default())
    }

    fn new_db_with_config(config: Config) -> Self {
        Self {
            config,
            collections: HashMap::new(),
        }
    }

    pub fn create(&mut self, coll_name: &str) -> MemoryCollection {
        self.create_with_config(coll_name, self.config.clone())
    }

    pub fn create_with_config(&mut self, coll_name: &str, config: Config) -> MemoryCollection {
        let collection = MemoryCollection::new_coll(coll_name, config);
        self.collections.insert(coll_name.to_string(), Arc::clone(&collection));

        collection
    }

    pub fn get(&self, col_name: &str) -> Option<MemoryCollection> {
        self.collections.get(col_name).map(Arc::clone)
    }

    pub fn list_collections(&self) -> Vec<String> {
        self.collections.keys().cloned().collect::<Vec<_>>()
    }

}

impl DbCommon for Db {

    fn new_db() -> Self {
        InternalDb::new_db().into_protected()
    }

    fn new_db_with_config(config: Config) -> Self {
        InternalDb::new_db_with_config(config).into_protected()
    }

    fn create(&mut self, coll_name: &str) -> MemoryCollection {
        self.write().unwrap().create(coll_name)
    }

    fn create_with_config(&mut self, coll_name: &str, config: Config) -> MemoryCollection {
        self.write().unwrap().create_with_config(coll_name, config)
    }

    fn get(&self, col_name: &str) -> Option<MemoryCollection> {
        self.read().unwrap().get(col_name)
    }

    fn list_collections(&self) -> Vec<String> {
        self.read().unwrap().list_collections()
    }
}

impl SchemaProvider for Db {
    fn schema_of(&self, collection_ref: &str) -> Option<super::SchemaDict> {
        let guard = self.read().ok()?;
        let coll = guard.get(collection_ref)?;
        coll.read().ok()?.schema()
    }
}

pub trait DbRunner {
    /// Parse, analyze, plan, and execute a SQL string against this Db.
    fn query(&self, sql: &str) -> Result<Vec<serde_json::Value>, AnalyzerError>;
}

impl DbRunner for Db {
    fn query(&self, sql: &str) -> Result<Vec<serde_json::Value>, AnalyzerError> {
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

// src/database/db_runner_tests.rs
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use crate::database::{DbCommon, InternalDb, Config, IdType};

    fn mk_db() -> Db {
        let mut db = InternalDb::new_db_with_config(Config { id_type: IdType::None, id_key: "id".into() }).into_protected();
        let mut t = db.create("t");
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

        let rows = DbRunner::query(&db, sql).expect("query should succeed");
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
        let rows = DbRunner::query(&db, sql).expect("query should succeed");
        assert_eq!(rows.len(), 1);
        // t has 5 rows -> t × t has 25 rows
        assert_eq!(rows[0]["n"].as_i64().unwrap(), 25);
    }
}
