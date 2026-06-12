use std::{
    fs,
    io::{Error as IoError, ErrorKind},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

pub fn boxed_debug<E: std::fmt::Debug>(error: E) -> Box<dyn std::error::Error> {
    IoError::other(format!("{error:?}")).into()
}

pub fn pretty<T: Serialize>(value: &T) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(error) => format!("<<failed to serialize value: {error}>>"),
    }
}

pub fn required<T>(value: Option<T>, context: &str) -> Result<T, Box<dyn std::error::Error>> {
    value.ok_or_else(|| IoError::new(ErrorKind::NotFound, context.to_string()).into())
}

pub fn schema_summary(schema: &fosk::SchemaDict) -> String {
    schema_summary_fields(&schema.fields)
}

pub fn schema_summary_fields(fields: &indexmap::IndexMap<String, fosk::FieldInfo>) -> String {
    let mut entries = fields
        .iter()
        .map(|(name, info)| {
            let suffix = if info.nullable { "" } else { "!" };
            format!("{name}: {:?}{suffix}", info.ty)
        })
        .collect::<Vec<_>>();
    entries.sort();
    entries.join(", ")
}

pub fn temp_file(prefix: &str, extension: &str) -> PathBuf {
    temp_path(prefix, extension)
}

fn temp_path(prefix: &str, extension: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    std::env::temp_dir().join(format!("{prefix}-{nanos}.{extension}"))
}

pub fn remove_temp_file(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

pub fn app_file(relative_path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path)
}
