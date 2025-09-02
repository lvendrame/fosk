use std::cmp::Ordering;

use serde_json::{Map, Value};

use crate::parser::{analyzer::LiteralResolver, ast::{Column, ComparatorOp, Function, Literal, Predicate, ScalarExpr, Truth}};

pub struct Eval;

impl Eval {
    pub fn eval_scalar(expr: &ScalarExpr, row: &Map<String, Value>) -> Value {
        match expr {
            ScalarExpr::Literal(l) => match l {
                Literal::Null => Value::Null,
                Literal::Bool(b) => Value::Bool(*b),
                Literal::Int(i)  => Self::json_i(*i),
                Literal::Float(f)=> Self::json_f(f.into_inner()),
                Literal::String(s)=> Value::String(s.clone()),
            },
            ScalarExpr::Column(c) => {
                let key = match c {
                    Column::WithCollection { collection, name } => format!("{}.{}", collection, name),
                    Column::Name { name } => name.clone(), // analyzer should qualify earlier; kept for safety
                };
                row.get(&key).cloned().unwrap_or(Value::Null)
            }
            ScalarExpr::Function(f) => Self::eval_scalar_function(f, row),
            ScalarExpr::WildCard | ScalarExpr::WildCardWithCollection(_) => Value::Null, // should not appear after analysis
        }
    }

    fn eval_scalar_function(f: &Function, row: &Map<String, Value>) -> Value {
        let lname = f.name.to_ascii_lowercase();
        let args: Vec<Value> = f.args.iter().map(|a| Self::eval_scalar(a, row)).collect();
        match (lname.as_str(), args.as_slice()) {
            ("upper",  [Value::String(s)]) => Value::String(s.to_uppercase()),
            ("lower",  [Value::String(s)]) => Value::String(s.to_lowercase()),
            ("trim",   [Value::String(s)]) => Value::String(s.trim().to_string()),
            ("length", [Value::String(s)]) => Self::json_i(s.chars().count() as i64),
            _ => Value::Null, // aggregates are not evaluated here; they are handled by Aggregate executor
        }
    }

    pub fn eval_predicate3(predicate: &Predicate, row: &Map<String, Value>) -> Truth {
        match predicate {
            Predicate::And(v) => v.iter().fold(Truth::True, |acc, x| acc.and(Self::eval_predicate3(x, row))),
            Predicate::Or(v)  => v.iter().fold(Truth::False, |acc, x| acc.or(Self::eval_predicate3(x, row))),
            Predicate::Compare { left, op, right } => {
                let l = Self::eval_scalar(left, row);
                let r = Self::eval_scalar(right, row);
                Self::lit_cmp3(&l, *op, &r)
            }
            Predicate::IsNull { expr, negated } => {
                let v = Self::eval_scalar(expr, row);
                let t = if v.is_null() { Truth::True } else { Truth::False };
                if *negated { t.not() } else { t }
            }
            Predicate::InList { expr, list, negated } => {
                let v = Self::eval_scalar(expr, row);
                let mut found = false;
                let mut has_null = false;
                for e in list {
                    let ev = Self::eval_scalar(e, row);
                    if ev.is_null() { has_null = true; continue; }
                    if Self::value_equal(&v, &ev) { found = true; break; }
                }
                let t = if found { Truth::True } else if has_null { Truth::Unknown } else { Truth::False };
                if *negated { t.not() } else { t }
            }
            Predicate::Like { expr, pattern, negated } => {
                let v = Self::eval_scalar(expr, row);
                let p = Self::eval_scalar(pattern, row);
                let t = match (v, p) {
                    (Value::String(s), Value::String(pat)) => LiteralResolver::eval_like(&s, &pat),
                    (Value::Null, _) | (_, Value::Null) => Truth::Unknown,
                    _ => Truth::Unknown,
                };
                if *negated { t.not() } else { t }
            }
            Predicate::Const3(t) => *t,
        }
    }

    fn lit_cmp3(l: &Value, op: ComparatorOp, r: &Value) -> Truth {
        if l.is_null() || r.is_null() { return Truth::Unknown; }
        // Only numeric, bool eq/neq, string eq/neq — mirrors analyzer folding
        match (l, r) {
            (Value::Bool(a), Value::Bool(b)) => match op {
                ComparatorOp::Eq => if a == b { Truth::True } else { Truth::False },
                ComparatorOp::NotEq => if a != b { Truth::True } else { Truth::False },
                _ => Truth::Unknown
            },
            (Value::Number(a), Value::Number(b)) => {
                let x = a.as_f64().unwrap(); let y = b.as_f64().unwrap();
                let ord = x.partial_cmp(&y);
                match (op, ord) {
                    (ComparatorOp::Eq, Some(Ordering::Equal)) => Truth::True,
                    (ComparatorOp::NotEq, Some(Ordering::Equal)) => Truth::False,
                    (ComparatorOp::NotEq, _) => Truth::True,
                    (ComparatorOp::Lt, Some(Ordering::Less)) => Truth::True,
                    (ComparatorOp::LtEq, Some(Ordering::Less|Ordering::Equal)) => Truth::True,
                    (ComparatorOp::Gt, Some(Ordering::Greater)) => Truth::True,
                    (ComparatorOp::GtEq, Some(Ordering::Greater|Ordering::Equal)) => Truth::True,
                    _ => Truth::False,
                }
            }
            (Value::String(a), Value::String(b)) => match op {
                ComparatorOp::Eq => if a == b { Truth::True } else { Truth::False },
                ComparatorOp::NotEq => if a != b { Truth::True } else { Truth::False },
                _ => Truth::Unknown
            },
            _ => Truth::Unknown,
        }
    }

    fn json_i(i: i64) -> Value { Value::Number(serde_json::Number::from(i)) }
    fn json_f(f: f64) -> Value { serde_json::Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null) }
    fn value_equal(a: &Value, b: &Value) -> bool {
        use serde_json::Value::*;
        match (a, b) {
            (Null, Null) => true,
            (Bool(x), Bool(y)) => x == y,
            (Number(x), Number(y)) => x.as_f64() == y.as_f64(),
            (String(x), String(y)) => x == y,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {

    // --- helpers -------------------------------------------------------------

    use serde_json::{Map, Value};

    use crate::{executor::eval::Eval, parser::ast::{Column, ComparatorOp, Function, Literal, Predicate, ScalarExpr, Truth}};

    fn row(pairs: &[(&str, Value)]) -> Map<String, Value> {
        let mut m = Map::new();
        for (k, v) in pairs {
            m.insert((*k).to_string(), v.clone());
        }
        m
    }
    fn lit_i(i: i64) -> ScalarExpr { ScalarExpr::Literal(Literal::Int(i)) }
    fn lit_f(f: f64) -> ScalarExpr { ScalarExpr::Literal(Literal::Float(ordered_float::NotNan::new(f).unwrap())) }
    fn lit_s(s: &str) -> ScalarExpr { ScalarExpr::Literal(Literal::String(s.to_string())) }
    fn lit_b(b: bool) -> ScalarExpr { ScalarExpr::Literal(Literal::Bool(b)) }
    fn lit_null() -> ScalarExpr { ScalarExpr::Literal(Literal::Null) }
    fn col_q(coll: &str, name: &str) -> ScalarExpr {
        ScalarExpr::Column(Column::WithCollection{ collection: coll.into(), name: name.into() })
    }
    fn col_u(name: &str) -> ScalarExpr {
        ScalarExpr::Column(Column::Name{ name: name.into() })
    }
    fn fun(name: &str, args: Vec<ScalarExpr>) -> ScalarExpr {
        ScalarExpr::Function(Function{ name: name.into(), args, distinct: false })
    }

    // --- scalar evaluation ---------------------------------------------------

    #[test]
    fn scalar_literals_eval_correctly() {
        let m = Map::new();
        assert_eq!(Eval::eval_scalar(&lit_null(), &m), Value::Null);
        assert_eq!(Eval::eval_scalar(&lit_b(true), &m), Value::Bool(true));
        assert_eq!(Eval::eval_scalar(&lit_i(42), &m), Value::Number(42.into()));
        assert_eq!(Eval::eval_scalar(&lit_f(1.5), &m), Value::Number(serde_json::Number::from_f64(1.5).unwrap()));
        assert_eq!(Eval::eval_scalar(&lit_s("x"), &m), Value::String("x".into()));
    }

    #[test]
    fn scalar_column_lookup_qualified_and_unqualified() {
        let m = row(&[
            ("t.id", Value::Number(1.into())),
            ("name", Value::String("Ana".into())),
        ]);
        assert_eq!(Eval::eval_scalar(&col_q("t","id"), &m), Value::Number(1.into()));
        assert_eq!(Eval::eval_scalar(&col_u("name"), &m), Value::String("Ana".into()));
        // missing -> Null
        assert_eq!(Eval::eval_scalar(&col_q("t","missing"), &m), Value::Null);
    }

    #[test]
    fn scalar_wildcard_is_never_evaluated_but_returns_null_if_seen() {
        let m = Map::new();
        assert!(matches!(Eval::eval_scalar(&ScalarExpr::WildCard, &m), Value::Null));
        assert!(matches!(Eval::eval_scalar(&ScalarExpr::WildCardWithCollection("t".into()), &m), Value::Null));
    }

    #[test]
    fn scalar_functions_work_upper_lower_trim_length() {
        let m = Map::new();
        assert_eq!(
            Eval::eval_scalar(&fun("upper", vec![lit_s("aBc")]), &m),
            Value::String("ABC".into())
        );
        assert_eq!(
            Eval::eval_scalar(&fun("lower", vec![lit_s("aBc")]), &m),
            Value::String("abc".into())
        );
        assert_eq!(
            Eval::eval_scalar(&fun("trim", vec![lit_s("  hi  ")]), &m),
            Value::String("hi".into())
        );
        assert_eq!(
            Eval::eval_scalar(&fun("length", vec![lit_s("hé")]), &m),
            Value::Number(2.into()) // grapheme vs chars: we use chars(), so 2 here
        );
        // non-supported sigs -> Null
        assert_eq!(Eval::eval_scalar(&fun("upper", vec![lit_i(1)]), &m), Value::Null);
    }

    // --- predicate: comparisons & 3VL ---------------------------------------

    #[test]
    fn compare_numbers_behave() {
        let m = Map::new();
        let make = |l: ScalarExpr, op: ComparatorOp, r: ScalarExpr| Predicate::Compare{ left: l, op, right: r };

        assert!(matches!(Eval::eval_predicate3(&make(lit_i(2), ComparatorOp::Gt, lit_i(1)), &m), Truth::True));
        assert!(matches!(Eval::eval_predicate3(&make(lit_f(2.0), ComparatorOp::Lt, lit_f(3.0)), &m), Truth::True));
        assert!(matches!(Eval::eval_predicate3(&make(lit_i(2), ComparatorOp::Eq, lit_f(2.0)), &m), Truth::True));
        assert!(matches!(Eval::eval_predicate3(&make(lit_f(2.0), ComparatorOp::NotEq, lit_f(2.1)), &m), Truth::True));
    }

    #[test]
    fn compare_strings_and_bools_eq_only_and_null_is_unknown() {
        let m = Map::new();
        let make = |l, op, r| Predicate::Compare{ left: l, op, right: r };

        // strings: only Eq/NotEq True/False; others Unknown
        assert!(matches!(Eval::eval_predicate3(&make(lit_s("a"), ComparatorOp::Eq, lit_s("a")), &m), Truth::True));
        assert!(matches!(Eval::eval_predicate3(&make(lit_s("a"), ComparatorOp::NotEq, lit_s("b")), &m), Truth::True));
        assert!(matches!(Eval::eval_predicate3(&make(lit_s("a"), ComparatorOp::Lt, lit_s("b")), &m), Truth::Unknown));

        // bools: only Eq/NotEq; others Unknown
        assert!(matches!(Eval::eval_predicate3(&make(lit_b(true), ComparatorOp::Eq, lit_b(true)), &m), Truth::True));
        assert!(matches!(Eval::eval_predicate3(&make(lit_b(true), ComparatorOp::Gt, lit_b(false)), &m), Truth::Unknown));

        // null involvement => Unknown
        assert!(matches!(Eval::eval_predicate3(&make(lit_null(), ComparatorOp::Eq, lit_i(1)), &m), Truth::Unknown));
        assert!(matches!(Eval::eval_predicate3(&make(lit_i(1), ComparatorOp::Lt, lit_null()), &m), Truth::Unknown));
    }

    // --- IS NULL / IS NOT NULL ---------------------------------------------

    #[test]
    fn is_null_and_is_not_null() {
        let m = row(&[
            ("t.a", Value::Null),
            ("t.b", Value::String("x".into())),
        ]);
        let is_null_a = Predicate::IsNull { expr: col_q("t","a"), negated: false };
        let is_not_null_b = Predicate::IsNull { expr: col_q("t","b"), negated: true };
        assert!(matches!(Eval::eval_predicate3(&is_null_a, &m), Truth::True));
        assert!(matches!(Eval::eval_predicate3(&is_not_null_b, &m), Truth::True));
    }

    // --- IN / NOT IN with NULL semantics ------------------------------------

    #[test]
    fn in_and_not_in_with_null_semantics() {
        let m = Map::new();

        // 2 IN (1,NULL,2) -> True  (match short-circuits)
        let p_true = Predicate::InList {
            expr: lit_i(2),
            list: vec![lit_i(1), lit_null(), lit_i(2)],
            negated: false
        };
        assert!(matches!(Eval::eval_predicate3(&p_true, &m), Truth::True));

        // 3 IN (1,NULL,2) -> Unknown (no match but NULL present)
        let p_unknown = Predicate::InList {
            expr: lit_i(3),
            list: vec![lit_i(1), lit_null(), lit_i(2)],
            negated: false
        };
        assert!(matches!(Eval::eval_predicate3(&p_unknown, &m), Truth::Unknown));

        // NOT IN: 3 NOT IN (1,2) -> True
        let p_notin_true = Predicate::InList {
            expr: lit_i(3),
            list: vec![lit_i(1), lit_i(2)],
            negated: true
        };
        assert!(matches!(Eval::eval_predicate3(&p_notin_true, &m), Truth::True));

        // NOT IN with NULL and no match -> Unknown (SQL 3VL)
        let p_notin_unknown = Predicate::InList {
            expr: lit_i(3),
            list: vec![lit_i(1), lit_null()],
            negated: true
        };
        assert!(matches!(Eval::eval_predicate3(&p_notin_unknown, &m), Truth::Unknown));
    }

    // --- LIKE / NOT LIKE -----------------------------------------------------

    #[test]
    fn like_and_not_like_case_insensitive_and_escape() {
        let m = Map::new();

        // Case-insensitive; % and _ wildcards
        // "he%2_" matches "Hello123" because: "he" + "llo1" (via %) + "2" + "_" -> matches "3"
        let like_p = Predicate::Like {
            expr: ScalarExpr::Literal(Literal::String("Hello123".into())),
            pattern: ScalarExpr::Literal(Literal::String("he%2_".into())),
            negated: false
        };
        assert!(matches!(Eval::eval_predicate3(&like_p, &m), Truth::True));

        // Escaping '_' -> must match literal underscore
        let like_escape = Predicate::Like {
            expr: ScalarExpr::Literal(Literal::String("a_c".into())),
            pattern: ScalarExpr::Literal(Literal::String("a\\_c".into())),
            negated: false
        };
        assert!(matches!(Eval::eval_predicate3(&like_escape, &m), Truth::True));

        // NOT LIKE when pattern matches -> False
        let not_like = Predicate::Like {
            expr: ScalarExpr::Literal(Literal::String("abc".into())),
            pattern: ScalarExpr::Literal(Literal::String("a_c".into())),
            negated: true
        };
        assert!(matches!(Eval::eval_predicate3(&not_like, &m), Truth::False));

        // NULL involvement -> Unknown
        let like_null = Predicate::Like {
            expr: ScalarExpr::Literal(Literal::Null),
            pattern: ScalarExpr::Literal(Literal::String("a%".into())),
            negated: false
        };
        assert!(matches!(Eval::eval_predicate3(&like_null, &m), Truth::Unknown));
    }

    // --- AND / OR with 3-valued truth ---------------------------------------

    #[test]
    fn and_or_three_valued_logic() {
        let m = Map::new();
        let t = Predicate::Compare { left: lit_i(2), op: ComparatorOp::Gt, right: lit_i(1) }; // True
        let f = Predicate::Compare { left: lit_i(2), op: ComparatorOp::Lt, right: lit_i(1) }; // False
        let u = Predicate::Compare { left: lit_i(1), op: ComparatorOp::Eq, right: lit_null() }; // Unknown

        // True AND Unknown -> Unknown
        assert!(matches!(Eval::eval_predicate3(&Predicate::And(vec![t.clone(), u.clone()]), &m), Truth::Unknown));
        // False AND Unknown -> False (short-circuit semantics in our fold/eval combination)
        assert!(matches!(Eval::eval_predicate3(&Predicate::And(vec![f.clone(), u.clone()]), &m), Truth::False));
        // True OR Unknown -> True
        assert!(matches!(Eval::eval_predicate3(&Predicate::Or(vec![t, u.clone()]), &m), Truth::True));
        // False OR Unknown -> Unknown
        assert!(matches!(Eval::eval_predicate3(&Predicate::Or(vec![f, u]), &m), Truth::Unknown));
    }
}
