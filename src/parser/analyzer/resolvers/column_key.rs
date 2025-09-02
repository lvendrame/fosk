use crate::parser::ast::Column;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnKey {
    pub column: String,
    pub name: String
}

impl ColumnKey {
    pub fn of(col: &Column) -> Self {
        match col {
            Column::WithCollection { collection, name } => Self {
                column: collection.clone(),
                name: name.clone(),
            },
            Column::Name { name } => Self {
                column: String::new(),
                name: name.clone()
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashSet, HashMap};

    // Helper to build columns quickly
    fn col_wc(coll: &str, name: &str) -> Column {
        Column::WithCollection { collection: coll.to_string(), name: name.to_string() }
    }
    fn col_name(name: &str) -> Column {
        Column::Name { name: name.to_string() }
    }

    #[test]
    fn of_withcollection_basic() {
        let c = col_wc("users", "id");
        let key = ColumnKey::of(&c);
        assert_eq!(key.column, "users");
        assert_eq!(key.name, "id");
    }

    #[test]
    fn of_name_basic_sets_empty_column() {
        let c = col_name("id");
        let key = ColumnKey::of(&c);
        assert_eq!(key.column, "");
        assert_eq!(key.name, "id");
    }

    #[test]
    fn equality_and_hash_same_values_are_equal() {
        let k1 = ColumnKey { column: "t".into(), name: "a".into() };
        let k2 = ColumnKey { column: "t".into(), name: "a".into() };
        assert_eq!(k1, k2);

        // HashSet behavior
        let mut set = HashSet::new();
        assert!(set.insert(k1.clone()));
        assert!(!set.insert(k2.clone())); // duplicate should not insert
        assert!(set.contains(&k1));
        assert!(set.contains(&k2));

        // HashMap behavior
        let mut map: HashMap<ColumnKey, i32> = HashMap::new();
        map.insert(k1.clone(), 1);
        map.insert(k2.clone(), 2); // overwrites
        assert_eq!(map.get(&k1), Some(&2));
        assert_eq!(map.get(&k2), Some(&2));
    }

    #[test]
    fn withcollection_and_name_are_distinct_keys() {
        // Same column name "a", but one qualified and one unqualified
        let k_wc = ColumnKey::of(&col_wc("t", "a"));
        let k_nm = ColumnKey::of(&col_name("a"));

        // They must not be equal (column differs: "t" vs "")
        assert_ne!(k_wc, k_nm);

        // In a set, both should coexist
        let mut set = HashSet::new();
        set.insert(k_wc.clone());
        set.insert(k_nm.clone());
        assert_eq!(set.len(), 2);
        assert!(set.contains(&k_wc));
        assert!(set.contains(&k_nm));
    }

    #[test]
    fn unicode_and_special_characters_preserved() {
        let c = col_wc("Σχήμα-1", "колонка_✓");
        let k = ColumnKey::of(&c);
        assert_eq!(k.column, "Σχήμα-1");
        assert_eq!(k.name, "колонка_✓");

        let c2 = col_name("字段-名/äöü");
        let k2 = ColumnKey::of(&c2);
        assert_eq!(k2.column, "");
        assert_eq!(k2.name, "字段-名/äöü");
    }

    #[test]
    fn empty_strings_are_supported() {
        // Edge: empty collection (rare, but testable)
        let k = ColumnKey::of(&col_wc("", "x"));
        assert_eq!(k.column, "");
        assert_eq!(k.name, "x");

        // Edge: empty name
        let k2 = ColumnKey::of(&col_name(""));
        assert_eq!(k2.column, "");
        assert_eq!(k2.name, "");
    }
}
