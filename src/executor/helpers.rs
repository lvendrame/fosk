use serde_json::Value;

pub struct Helpers;

impl Helpers {
    pub fn canonical_tuple(vals: &[Value]) -> String {
        // Note: serde_json::to_string preserves map key order by default; your input objects
        // should be constructed deterministically (we use arrays here, so it's stable).
        serde_json::to_string(vals).unwrap()
    }

    // NULLS LAST comparator helper (ascending flag)
    pub fn cmp_json_for_sort(a: &Value, b: &Value, ascending: bool) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        use serde_json::Value::*;
        // NULLS LAST
        match (a, b) {
            (Null, Null) => Equal,
            (Null, _)    => Greater,      // null after non-null
            (_, Null)    => Less,
            (Bool(x), Bool(y)) => if ascending { x.cmp(y) } else { y.cmp(x) },
            (Number(x), Number(y)) => {
                let ax = x.as_f64().unwrap();
                let by = y.as_f64().unwrap();
                let ord = ax.partial_cmp(&by).unwrap_or(Equal);
                if ascending { ord } else { ord.reverse() }
            },
            (String(x), String(y)) => {
                let ord = x.cmp(y);
                if ascending { ord } else { ord.reverse() }
            },
            // fallback: compare type tags to keep total order stable
            (Array(_), Array(_)) | (Object(_), Object(_)) => {
                let sa = serde_json::to_string(a).unwrap();
                let sb = serde_json::to_string(b).unwrap();
                let ord = sa.cmp(&sb);
                if ascending { ord } else { ord.reverse() }
            },
            (lhs, rhs) => {
                let lt = Self::type_rank(lhs);
                let rt = Self::type_rank(rhs);
                let ord = lt.cmp(&rt);
                if ascending { ord } else { ord.reverse() }
            },
        }
    }
    fn type_rank(v: &Value) -> u8 {
        match v {
            Value::Null => 0, Value::Bool(_) => 1, Value::Number(_) => 2, Value::String(_) => 3,
            Value::Array(_) => 4, Value::Object(_) => 5
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Helpers;
    use serde_json::{json, Value};
    use std::cmp::Ordering::*;

    // ---------- canonical_tuple ----------

    #[test]
    fn canonical_tuple_is_deterministic_for_same_values() {
        let a = vec![json!(1), json!("x"), json!(true)];
        let b = vec![json!(1), json!("x"), json!(true)];
        assert_eq!(Helpers::canonical_tuple(&a), Helpers::canonical_tuple(&b));
    }

    #[test]
    fn canonical_tuple_differs_for_different_values() {
        let a = vec![json!(1), json!("x")];
        let b = vec![json!(1), json!("y")];
        assert_ne!(Helpers::canonical_tuple(&a), Helpers::canonical_tuple(&b));
    }

    #[test]
    fn canonical_tuple_is_stable_for_arrays_and_objects() {
        let a = vec![json!([1, 2, 3]), json!({"a":1,"b":2})];
        let b = vec![json!([1, 2, 3]), json!({"a":1,"b":2})];
        assert_eq!(Helpers::canonical_tuple(&a), Helpers::canonical_tuple(&b));
    }

    // ---------- cmp_json_for_sort ----------

    #[test]
    fn sort_nulls_last_in_ascending_and_descending() {
        let n = Value::Null;
        let z = json!(0);

        // Asc: non-null < null
        assert_eq!(Helpers::cmp_json_for_sort(&z, &n, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&n, &z, true), Greater);
        assert_eq!(Helpers::cmp_json_for_sort(&n, &n, true), Equal);

        // Desc: still NULLS LAST
        assert_eq!(Helpers::cmp_json_for_sort(&z, &n, false), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&n, &z, false), Greater);
        assert_eq!(Helpers::cmp_json_for_sort(&n, &n, false), Equal);
    }

    #[test]
    fn sort_numbers_respects_ascending_and_descending() {
        let a = json!(1.0);
        let b = json!(2.0);
        assert_eq!(Helpers::cmp_json_for_sort(&a, &b, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&a, &b, false), Greater);
        assert_eq!(Helpers::cmp_json_for_sort(&a, &a, true), Equal);
    }

    #[test]
    fn sort_strings_is_lexicographic_and_directional() {
        let a = json!("Alice");
        let b = json!("Bob");
        assert_eq!(Helpers::cmp_json_for_sort(&a, &b, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&a, &b, false), Greater);
        assert_eq!(Helpers::cmp_json_for_sort(&a, &a, true), Equal);
    }

    #[test]
    fn sort_bools_false_before_true_in_ascending() {
        let f = json!(false);
        let t = json!(true);
        assert_eq!(Helpers::cmp_json_for_sort(&f, &t, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&f, &t, false), Greater);
    }

    #[test]
    fn arrays_and_objects_use_canonical_string_compare() {
        // Arrays — same prefix, differing tail
        let a1 = json!([1, 2, 3]);
        let a2 = json!([1, 2, 4]);
        assert_eq!(Helpers::cmp_json_for_sort(&a1, &a2, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&a1, &a2, false), Greater);

        // Objects — deterministic by serialized form
        let o1 = json!({"a": 1, "b": 2});
        let o2 = json!({"a": 1, "b": 3});
        assert_eq!(Helpers::cmp_json_for_sort(&o1, &o2, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&o1, &o2, false), Greater);

        // Equal structures => Equal
        assert_eq!(Helpers::cmp_json_for_sort(&o1, &o1, true), Equal);
        assert_eq!(Helpers::cmp_json_for_sort(&a1, &a1, true), Equal);
    }

    #[test]
    fn cross_type_order_uses_type_rank_excluding_null() {
        use serde_json::json;
        use std::cmp::Ordering::*;
        use crate::executor::helpers::Helpers;

        // rank (excluding NULL special-case): Bool(1) < Number(2) < String(3) < Array(4) < Object(5)
        let v_bool = json!(true);
        let v_num  = json!(0);
        let v_str  = json!("s");
        let v_arr  = json!([1]);
        let v_obj  = json!({"a":1});

        // Ascending: increasing rank
        assert_eq!(Helpers::cmp_json_for_sort(&v_bool, &v_num, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_num,  &v_str, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_str,  &v_arr, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_arr,  &v_obj, true), Less);

        // Descending: reverse rank
        assert_eq!(Helpers::cmp_json_for_sort(&v_obj, &v_arr, false), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_arr, &v_str, false), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_str, &v_num, false), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_num, &v_bool, false), Less);
    }

    #[test]
    fn nulls_are_last_in_both_directions() {
        use serde_json::json;
        use std::cmp::Ordering::*;
        use crate::executor::helpers::Helpers;

        let v_null = serde_json::Value::Null;
        let v_num  = json!(0);
        let v_str  = json!("s");
        let v_arr  = json!([1]);
        let v_obj  = json!({"a":1});
        let v_bool = json!(false);

        // Asc: non-null < null
        assert_eq!(Helpers::cmp_json_for_sort(&v_num,  &v_null, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_str,  &v_null, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_arr,  &v_null, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_obj,  &v_null, true), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_bool, &v_null, true), Less);

        // Desc: still NULLS LAST
        assert_eq!(Helpers::cmp_json_for_sort(&v_num,  &v_null, false), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_str,  &v_null, false), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_arr,  &v_null, false), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_obj,  &v_null, false), Less);
        assert_eq!(Helpers::cmp_json_for_sort(&v_bool, &v_null, false), Less);

        // Null vs Null => Equal
        assert_eq!(Helpers::cmp_json_for_sort(&v_null, &v_null, true), Equal);
        assert_eq!(Helpers::cmp_json_for_sort(&v_null, &v_null, false), Equal);
    }
}
