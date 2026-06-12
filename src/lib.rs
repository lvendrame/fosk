//! In-memory SQL-like query engine and lightweight data store.
//!
//! `fosk` lets tests and prototypes create JSON-backed collections in memory,
//! then query them with a small SQL-like language. Most users start with
//! [`Db`], configure id behavior with [`DbConfig`] and [`IdType`], and work
//! with collection handles returned by [`Db::create`] or [`Db::get`].
//!
//! # Example
//!
//! ```
//! use fosk::{Db, DbConfig};
//! use serde_json::json;
//!
//! # fn main() -> Result<(), String> {
//! let db = Db::new_with_config(DbConfig::int("id"));
//! let people = db.create("people");
//!
//! let _inserted = people
//!     .add(json!({ "name": "Ada", "age": 37 }))
//!     .map_err(|error| error.to_string())?;
//!
//! let rows = db
//!     .query("SELECT id, name FROM people WHERE age > 30")
//!     .map_err(|error| format!("{error:?}"))?;
//!
//! assert_eq!(rows.len(), 1);
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

/// Database handles, collection handles, configuration, and schema metadata.
pub mod database;
pub use database::{
    AddBatchError, AddError, CollectionReadError, CollectionWriteError, Db, DbCollection, DbConfig,
    FieldInfo, IdType, JsonPrimitive, LoadCollectionError, ReferenceColumn, SchemaDict,
    SchemaWithRefs, WriteCollectionError,
};

#[doc(hidden)]
#[allow(missing_docs)]
pub mod executor;
#[doc(hidden)]
#[allow(missing_docs)]
pub mod parser;
#[doc(hidden)]
#[allow(missing_docs)]
pub mod planner;
