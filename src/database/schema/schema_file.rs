use std::{ffi::OsString, fs, path::Path};

use serde_json::Value;

/// Read and parse a JSON file used by schema loaders.
pub(crate) fn read_schema_json_file(file_path: &OsString) -> Result<Value, String> {
    let file_path_lossy = file_path.to_string_lossy();
    let file_content = fs::read_to_string(file_path)
        .map_err(|_| format!("Could not read schema file {file_path_lossy}"))?;

    serde_json::from_str::<Value>(&file_content)
        .map_err(|_| format!("Schema file {file_path_lossy} does not contain valid JSON"))
}

/// Infer a single collection name from a schema file path.
pub(crate) fn collection_name_from_file_stem(file_path: &OsString) -> Result<String, String> {
    let path = Path::new(file_path);
    let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
        return Err(format!(
            "Could not infer collection name from schema file {}",
            file_path.to_string_lossy()
        ));
    };

    let stem = stem.trim();
    if stem.is_empty() {
        return Err(format!(
            "Could not infer collection name from schema file {}",
            file_path.to_string_lossy()
        ));
    }

    Ok(stem.to_string())
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write};

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn infers_collection_name_from_file_stem() {
        assert_eq!(
            collection_name_from_file_stem(&OsString::from("people.schema.json")).unwrap(),
            "people.schema"
        );
        assert_eq!(
            collection_name_from_file_stem(&OsString::from("people.json")).unwrap(),
            "people"
        );
    }

    #[test]
    fn reads_valid_schema_json_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("people.json");
        let mut file = File::create(&path).unwrap();
        file.write_all(json!({ "id": "Id" }).to_string().as_bytes())
            .unwrap();

        let value =
            read_schema_json_file(&OsString::from(path.to_string_lossy().into_owned())).unwrap();

        assert_eq!(value["id"], "Id");
    }

    #[test]
    fn errors_for_missing_schema_json_file() {
        let err = read_schema_json_file(&OsString::from("missing-schema-file.json")).unwrap_err();

        assert!(err.contains("Could not read schema file"));
    }

    #[test]
    fn errors_for_invalid_schema_json_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("invalid.json");
        let mut file = File::create(&path).unwrap();
        file.write_all(b"{ invalid json }").unwrap();

        let err = read_schema_json_file(&OsString::from(path.to_string_lossy().into_owned()))
            .unwrap_err();

        assert!(err.contains("does not contain valid JSON"));
    }

    #[test]
    fn errors_when_collection_name_cannot_be_inferred() {
        let empty_path = collection_name_from_file_stem(&OsString::from("")).unwrap_err();
        assert!(empty_path.contains("Could not infer collection name"));

        let blank_stem = collection_name_from_file_stem(&OsString::from("   .json")).unwrap_err();
        assert!(blank_stem.contains("Could not infer collection name"));
    }
}
