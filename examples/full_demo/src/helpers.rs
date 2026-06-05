use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

pub fn pretty<T: Serialize>(value: &T) -> String {
    serde_json::to_string_pretty(value).unwrap()
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
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{nanos}.{extension}"))
}

pub fn remove_temp_file(path: &PathBuf) {
    let _ = fs::remove_file(path);
}

pub fn app_file(relative_path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(relative_path)
}
