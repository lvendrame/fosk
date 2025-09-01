use std::{collections::HashMap, sync::{Arc, RwLock}};

use crate::database::{Config, DbCollection, MemoryCollection, SchemaProvider};

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

impl DbCommon for InternalDb {

    fn new_db() -> Self {
        Self::new_db_with_config(Config::default())
    }

    fn new_db_with_config(config: Config) -> Self {
        Self {
            config,
            collections: HashMap::new(),
        }
    }

    fn create(&mut self, coll_name: &str) -> MemoryCollection {
        self.create_with_config(coll_name, self.config.clone())
    }

    fn create_with_config(&mut self, coll_name: &str, config: Config) -> MemoryCollection {
        let collection = MemoryCollection::new_coll(coll_name, config);
        self.collections.insert(coll_name.to_string(), Arc::clone(&collection));

        collection
    }

    fn get(&self, col_name: &str) -> Option<MemoryCollection> {
        self.collections.get(col_name).map(Arc::clone)
    }

    fn list_collections(&self) -> Vec<String> {
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
        coll.read().ok()?.schema().cloned()
    }
}
