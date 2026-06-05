//! Database, collection, id, and schema APIs.
//!
//! This module contains the user-facing handles for storing JSON documents
//! and querying them with SQL-like statements. The most common entry points
//! are [`Db`], [`DbCollection`], [`DbConfig`], and [`IdType`].

/// ID generation strategies used by collections.
pub mod id_type;
pub use id_type::*;

#[doc(hidden)]
#[allow(missing_docs)]
pub mod id_manager;
pub use id_manager::*;

/// Database and collection configuration.
pub mod db_config;
pub use db_config::*;

/// Thread-safe collection handle and document operations.
pub mod db_collection;
pub use db_collection::*;

/// Thread-safe database handle and query operations.
pub mod db;
pub use db::*;

/// Inferred schema and reference metadata.
pub mod schema;
pub use schema::*;

#[doc(hidden)]
#[allow(missing_docs)]
pub mod expansion_chain;
pub use expansion_chain::*;

#[doc(hidden)]
#[allow(missing_docs)]
pub mod column_value;
pub use column_value::*;
