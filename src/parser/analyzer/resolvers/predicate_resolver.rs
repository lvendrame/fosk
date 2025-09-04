use crate::parser::{analyzer::{AnalysisContext, AnalyzerError, LiteralResolver, ScalarResolver}, ast::{Literal, Predicate, Truth}};

pub struct  PredicateResolver;

impl PredicateResolver {

    pub fn fold_predicate(pred: &Predicate) -> Predicate {
        match pred {
            Predicate::And(list) => {
                let mut acc = Truth::True;
                let mut out = Vec::with_capacity(list.len());
                for p in list {
                    match Self::fold_predicate(p) {
                        Predicate::Const3(t) => { acc = acc.and(t); if acc == Truth::False { return Predicate::Const3(Truth::False); } }
                        other => out.push(other),
                    }
                }
                if out.is_empty() { Predicate::Const3(acc) } else { Predicate::And(out) }
            }
            Predicate::Or(list) => {
                let mut acc = Truth::False;
                let mut out = Vec::with_capacity(list.len());
                for p in list {
                    match Self::fold_predicate(p) {
                        Predicate::Const3(t) => { acc = acc.or(t); if acc == Truth::True { return Predicate::Const3(Truth::True); } }
                        other => out.push(other),
                    }
                }
                if out.is_empty() { Predicate::Const3(acc) } else { Predicate::Or(out) }
            }

            Predicate::Compare { left, op, right } => {
                let l = ScalarResolver::fold_scalar(left);
                let r = ScalarResolver::fold_scalar(right);
                if let (Some(ll), Some(rr)) = (ScalarResolver::scalar_literal(&l), ScalarResolver::scalar_literal(&r)) {
                    Predicate::Const3(LiteralResolver::eval_compare3(&ll, *op, &rr))
                } else {
                    Predicate::Compare { left: l, op: *op, right: r }
                }
            }

            Predicate::IsNull { expr, negated } => {
                let e = ScalarResolver::fold_scalar(expr);
                if let Some(lit) = ScalarResolver::scalar_literal(&e) {
                    let t = match lit { Literal::Null => Truth::True, _ => Truth::False };
                    Predicate::Const3(if *negated { t.not() } else { t })
                } else { Predicate::IsNull { expr: e, negated: *negated } }
            }

            Predicate::InList { expr, list, negated } => {
                let e = ScalarResolver::fold_scalar(expr);
                let list_folded: Vec<_> = list.iter().map(ScalarResolver::fold_scalar).collect();

                let el = ScalarResolver::scalar_literal(&e);
                let lits: Option<Vec<Literal>> = if list_folded.iter().all(|x| ScalarResolver::scalar_literal(x).is_some()) {
                    Some(list_folded.iter().map(|x| ScalarResolver::scalar_literal(x).unwrap()).collect())
                } else { None };

                if let (Some(elit), Some(set)) = (el, lits) {
                    // SQL IN/NOT IN with NULLs:
                    // If any element is NULL and no match found => Unknown
                    let mut has_null = false;
                    let mut found = false;
                    for v in &set {
                        if matches!(v, Literal::Null) { has_null = true; }
                        else if LiteralResolver::literal_equal(&elit, v) { found = true; break; }
                    }
                    let t = if found { Truth::True } else if has_null { Truth::Unknown } else { Truth::False };
                    let t = if *negated { t.not() } else { t };
                    Predicate::Const3(t)
                } else {
                    Predicate::InList { expr: e, list: list_folded, negated: *negated }
                }
            }

            Predicate::Like { expr, pattern, negated } => {
                let e = ScalarResolver::fold_scalar(expr);
                let p = ScalarResolver::fold_scalar(pattern);
                match (ScalarResolver::scalar_literal(&e), ScalarResolver::scalar_literal(&p)) {
                    (Some(Literal::String(s)), Some(Literal::String(pat))) => {
                        let t = LiteralResolver::eval_like(&s, &pat);
                        let t = if *negated { t.not() } else { t };
                        Predicate::Const3(t)
                    }
                    (Some(Literal::Null), _) | (_, Some(Literal::Null)) => Predicate::Const3(Truth::Unknown),
                    _ => Predicate::Like { expr: e, pattern: p, negated: *negated },
                }
            }

            Predicate::Const3(t) => Predicate::Const3(*t),
        }
    }

    pub fn qualify_predicate(predicate: &Predicate, ctx: &mut AnalysisContext) -> Result<Predicate, AnalyzerError> {
        Ok(match predicate {
            Predicate::And(v) => Predicate::And(v.iter().map(|x| Self::qualify_predicate(x, ctx)).collect::<Result<Vec<_>,_>>()?),
            Predicate::Or(v)  => Predicate::Or(v.iter().map(|x| Self::qualify_predicate(x, ctx)).collect::<Result<Vec<_>,_>>()?),
            Predicate::Compare { left, op, right } => {
                        Predicate::Compare { left: ScalarResolver::qualify_scalar(left, ctx, false)?, op: *op, right: ScalarResolver::qualify_scalar(right, ctx, false)? }
                    },
            Predicate::IsNull { expr, negated } => {
                        Predicate::IsNull { expr: ScalarResolver::qualify_scalar(expr, ctx, false)?, negated: *negated }
                    },
            Predicate::InList { expr, list, negated } => {
                        Predicate::InList { expr: ScalarResolver::qualify_scalar(expr, ctx, false)?, list: list.iter().map(|x| ScalarResolver::qualify_scalar(x, ctx, true)).collect::<Result<Vec<_>,_>>()?, negated: *negated }
                    },
            Predicate::Like { expr, pattern, negated } => {
                        Predicate::Like { expr: ScalarResolver::qualify_scalar(expr, ctx, false)?, pattern: ScalarResolver::qualify_scalar(pattern, ctx, false)?, negated: *negated }
                    },
            Predicate::Const3(value) => Predicate::Const3(*value),
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use indexmap::IndexMap;
    use crate::{database::{FieldInfo, SchemaProvider}, parser::{analyzer::PredicateResolver, ast::{
        Column, ComparatorOp, Function, Predicate, ScalarExpr, Truth
    }}, JsonPrimitive, SchemaDict};

    #[test]
    fn fold_is_null_and_not_null() {
        // IS NULL
        let p = Predicate::IsNull { expr: ScalarExpr::Literal(Literal::Null), negated: false };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
        // IS NOT NULL
        let p = Predicate::IsNull { expr: ScalarExpr::Literal(Literal::String("x".into())), negated: true };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
    }

    #[test]
    fn fold_in_and_not_in() {
        // 5 IN (1,5,7) -> true
        let p = Predicate::InList { expr: ScalarExpr::Literal(Literal::Int(5)), list: vec![ScalarExpr::Literal(Literal::Int(1)), ScalarExpr::Literal(Literal::Int(5)), ScalarExpr::Literal(Literal::Int(7))], negated: false };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
        // 5 NOT IN (1,5,7) -> false
        let p = Predicate::InList { expr: ScalarExpr::Literal(Literal::Int(5)), list: vec![ScalarExpr::Literal(Literal::Int(1)), ScalarExpr::Literal(Literal::Int(5)), ScalarExpr::Literal(Literal::Int(7))], negated: true };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::False));
    }

    #[test]
    fn fold_like_and_not_like() {
        // 'hello' LIKE 'he%%' -> true
        let p = Predicate::Like { expr: ScalarExpr::Literal(Literal::String("hello".into())), pattern: ScalarExpr::Literal(Literal::String("he%%".into())), negated: false };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
        // NOT LIKE
        let p = Predicate::Like { expr: ScalarExpr::Literal(Literal::String("hello".into())), pattern: ScalarExpr::Literal(Literal::String("x%".into())), negated: true };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
    }

    // ---------- tiny SchemaProvider + ctx helpers ----------
    struct DummySchemas {
        by_name: std::collections::HashMap<String, SchemaDict>,
    }
    impl DummySchemas {
        fn new() -> Self { Self { by_name: std::collections::HashMap::new() } }
        fn with(mut self, name: &str, fields: Vec<(&str, JsonPrimitive, bool)>) -> Self {
            let mut m = IndexMap::new();
            for (k, ty, nullable) in fields {
                m.insert(k.to_string(), FieldInfo { ty, nullable });
            }
            self.by_name.insert(name.to_string(), SchemaDict { fields: m });
            self
        }
    }
    impl SchemaProvider for DummySchemas {
        fn schema_of(&self, backing_collection: &str) -> Option<SchemaDict> {
            self.by_name.get(backing_collection).cloned()
        }
    }

    fn ctx_for<'a>(sp: &'a DummySchemas, pairs: &'a [(&'a str, Option<&'a str>)]) -> AnalysisContext<'a> {
        let mut ctx = AnalysisContext::new(sp);
        for (backing, alias) in pairs {
            ctx.add_collection(alias.unwrap_or(backing).to_string(), (*backing).to_string());
        }
        ctx
    }

    // ---------- helpers ----------
    fn lit_i(v: i64) -> ScalarExpr { ScalarExpr::Literal(Literal::Int(v)) }
    fn lit_f(v: f64) -> ScalarExpr { ScalarExpr::Literal(Literal::Float(ordered_float::NotNan::new(v).unwrap())) }
    fn lit_s(v: &str) -> ScalarExpr { ScalarExpr::Literal(Literal::String(v.to_string())) }
    fn lit_b(v: bool) -> ScalarExpr { ScalarExpr::Literal(Literal::Bool(v)) }
    fn lit_n() -> ScalarExpr { ScalarExpr::Literal(Literal::Null) }
    fn col_unq(name: &str) -> ScalarExpr { ScalarExpr::Column(Column::Name { name: name.to_string() }) }
    fn col_q(coll: &str, name: &str) -> ScalarExpr { ScalarExpr::Column(Column::WithCollection { collection: coll.to_string(), name: name.to_string() }) }

    // ======================================================
    // Folding: Compare / Null / Numbers / Strings / Bools
    // ======================================================

    #[test]
    fn fold_compare_numeric_constants_to_const3() {
        let p = Predicate::Compare { left: lit_i(2), op: ComparatorOp::Lt, right: lit_f(2.5) };
        match PredicateResolver::fold_predicate(&p) {
            Predicate::Const3(Truth::True) => {}
            other => panic!("expected Const3(True), got {other:?}"),
        }

        let p2 = Predicate::Compare { left: lit_f(3.0), op: ComparatorOp::Gt, right: lit_i(5) };
        match PredicateResolver::fold_predicate(&p2) {
            Predicate::Const3(Truth::False) => {}
            other => panic!("expected Const3(False), got {other:?}"),
        }
    }

    #[test]
    fn fold_compare_with_null_is_unknown() {
        let p = Predicate::Compare { left: lit_n(), op: ComparatorOp::Eq, right: lit_i(1) };
        match PredicateResolver::fold_predicate(&p) {
            Predicate::Const3(Truth::Unknown) => {}
            other => panic!("expected Const3(Unknown), got {other:?}"),
        }
    }

    #[test]
    fn fold_is_null_and_is_not_null() {
        // IS NULL on literal
        let p = Predicate::IsNull { expr: lit_n(), negated: false };
        match PredicateResolver::fold_predicate(&p) {
            Predicate::Const3(Truth::True) => {}
            other => panic!("expected True, got {other:?}"),
        }

        let p2 = Predicate::IsNull { expr: lit_i(0), negated: false };
        match PredicateResolver::fold_predicate(&p2) {
            Predicate::Const3(Truth::False) => {}
            other => panic!("expected False, got {other:?}"),
        }

        // IS NOT NULL
        let p3 = Predicate::IsNull { expr: lit_n(), negated: true };
        match PredicateResolver::fold_predicate(&p3) {
            Predicate::Const3(Truth::False) => {}
            other => panic!("expected False, got {other:?}"),
        }
    }

    // ======================================================
    // Folding: IN / NOT IN with NULL semantics
    // ======================================================

    #[test]
    fn fold_in_list_found_match_true() {
        let p = Predicate::InList {
            expr: lit_i(2),
            list: vec![lit_i(1), lit_i(2), lit_i(3)],
            negated: false,
        };
        match PredicateResolver::fold_predicate(&p) {
            Predicate::Const3(Truth::True) => {}
            other => panic!("expected True, got {other:?}"),
        }
    }

    #[test]
    fn fold_in_list_no_match_with_null_yields_unknown() {
        let p = Predicate::InList {
            expr: lit_i(2),
            list: vec![lit_i(1), lit_n()],
            negated: false,
        };
        match PredicateResolver::fold_predicate(&p) {
            Predicate::Const3(Truth::Unknown) => {}
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    #[test]
    fn fold_not_in_list_no_match_with_null_stays_unknown() {
        // SQL: x NOT IN (1, NULL) => Unknown
        let p = Predicate::InList {
            expr: lit_i(2),
            list: vec![lit_i(1), lit_n()],
            negated: true,
        };
        match PredicateResolver::fold_predicate(&p) {
            Predicate::Const3(Truth::Unknown) => {}
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    // ======================================================
    // Folding: LIKE / NOT LIKE with case-insensitive + escape
    // ======================================================

    #[test]
    fn fold_like_with_escape_and_case_insensitive() {
        // 'Hello' LIKE 'he%'  (ci)
        let p = Predicate::Like { expr: lit_s("Hello"), pattern: lit_s("he%"), negated: false };
        match PredicateResolver::fold_predicate(&p) {
            Predicate::Const3(Truth::True) => {}
            other => panic!("expected True, got {other:?}"),
        }

        // escape % to literal
        let p2 = Predicate::Like { expr: lit_s("a%c"), pattern: lit_s(r"a\%c"), negated: false };
        match PredicateResolver::fold_predicate(&p2) {
            Predicate::Const3(Truth::True) => {}
            other => panic!("expected True, got {other:?}"),
        }
    }#[test]
    fn like_percent_matches_zero_or_more_chars() {
        // "%" matches empty, "a%", "%a", "%", etc.
        let p1 = Predicate::Like { expr: lit_s(""),    pattern: lit_s("%"),  negated: false };
        let p2 = Predicate::Like { expr: lit_s("abc"), pattern: lit_s("%"),  negated: false };
        let p3 = Predicate::Like { expr: lit_s("abc"), pattern: lit_s("a%"), negated: false };
        let p4 = Predicate::Like { expr: lit_s("abc"), pattern: lit_s("%c"), negated: false };
        for p in [p1,p2,p3,p4] {
            assert!(matches!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True)));
        }
    }

    #[test]
    fn like_underscore_is_exactly_one_char() {
        // "a_d" matches "abd" but not "ad" (0 char) or "abdd" (2 chars)
        let ok = Predicate::Like { expr: lit_s("abd"),  pattern: lit_s("a_d"), negated: false };
        let no0 = Predicate::Like { expr: lit_s("ad"),  pattern: lit_s("a_d"), negated: false };
        let no2 = Predicate::Like { expr: lit_s("abdd"),pattern: lit_s("a_d"), negated: false };
        assert!(matches!(PredicateResolver::fold_predicate(&ok),  Predicate::Const3(Truth::True)));
        assert!(matches!(PredicateResolver::fold_predicate(&no0), Predicate::Const3(Truth::False)));
        assert!(matches!(PredicateResolver::fold_predicate(&no2), Predicate::Const3(Truth::False)));
    }

    #[test]
    fn like_is_case_insensitive() {
        let p = Predicate::Like { expr: lit_s("HelloWorld"), pattern: lit_s("he%world"), negated: false };
        assert!(matches!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True)));
    }

    #[test]
    fn like_escape_percent_and_underscore() {
        // r"\%" matches literal '%'
        let p1 = Predicate::Like { expr: lit_s("he%llo"), pattern: lit_s(r"he\%l%"), negated: false };
        assert!(matches!(PredicateResolver::fold_predicate(&p1), Predicate::Const3(Truth::True)));

        // r"\_" matches literal '_'
        let p2 = Predicate::Like { expr: lit_s("a_c"), pattern: lit_s(r"a\_c"), negated: false };
        assert!(matches!(PredicateResolver::fold_predicate(&p2), Predicate::Const3(Truth::True)));
    }

    #[test]
    fn like_trailing_backslash_matches_literal_backslash() {
        // value ends with a literal backslash; pattern r"abc\" should match
        let p = Predicate::Like { expr: lit_s(r"abc\"), pattern: lit_s(r"abc\"), negated: false };
        assert!(matches!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True)));
    }

    #[test]
    fn like_escape_non_meta_char_is_just_literal() {
        // Escaping a normal char should behave as the char itself
        let p = Predicate::Like { expr: lit_s("a.b"), pattern: lit_s(r"a\.b"), negated: false };
        assert!(matches!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True)));
    }

    #[test]
    fn like_with_null_operands_yields_unknown() {
        let p1 = Predicate::Like { expr: lit_n(), pattern: lit_s("%"), negated: false };
        let p2 = Predicate::Like { expr: lit_s("x"), pattern: lit_n(), negated: false };
        assert!(matches!(PredicateResolver::fold_predicate(&p1), Predicate::Const3(Truth::Unknown)));
        assert!(matches!(PredicateResolver::fold_predicate(&p2), Predicate::Const3(Truth::Unknown)));
    }

    #[test]
    fn fold_like_with_null_is_unknown() {
        let p = Predicate::Like { expr: lit_n(), pattern: lit_s("%"), negated: false };
        match PredicateResolver::fold_predicate(&p) {
            Predicate::Const3(Truth::Unknown) => {}
            other => panic!("expected Unknown, got {other:?}"),
        }
        let p2 = Predicate::Like { expr: lit_s("x"), pattern: lit_n(), negated: false };
        match PredicateResolver::fold_predicate(&p2) {
            Predicate::Const3(Truth::Unknown) => {}
            other => panic!("expected Unknown, got {other:?}"),
        }
    }

    // ======================================================
    // Folding: AND / OR short-circuit + 3VL
    // ======================================================

    #[test]
    fn fold_and_or_short_circuits_and_3vl() {
        // AND: True AND Unknown AND False => should short-circuit to False
        let p_and = Predicate::And(vec![
            Predicate::Const3(Truth::True),
            Predicate::Const3(Truth::Unknown),
            Predicate::Const3(Truth::False),
        ]);
        match PredicateResolver::fold_predicate(&p_and) {
            Predicate::Const3(Truth::False) => {}
            other => panic!("expected False, got {other:?}"),
        }

        // OR: False OR Unknown OR True => short-circuit to True
        let p_or = Predicate::Or(vec![
            Predicate::Const3(Truth::False),
            Predicate::Const3(Truth::Unknown),
            Predicate::Const3(Truth::True),
        ]);
        match PredicateResolver::fold_predicate(&p_or) {
            Predicate::Const3(Truth::True) => {}
            other => panic!("expected True, got {other:?}"),
        }
    }

    // ======================================================
    // qualify_predicate: success
    // ======================================================

    #[test]
    fn qualify_predicate_qualifies_columns_and_nested_structures() {
        // schema: t(a:int, s:string)
        let sp = DummySchemas::new().with("t", vec![
            ("a", JsonPrimitive::Int, false),
            ("s", JsonPrimitive::String, false),
        ]);
        let mut ctx = ctx_for(&sp, &[("t", None)]);

        // lower(s) = 'x' AND a IN (1,2)
        let pred = Predicate::And(vec![
            Predicate::Compare {
                left: ScalarExpr::Function(Function {
                    name: "lower".into(),
                    distinct: false,
                    args: vec![col_unq("s")],
                }),
                op: ComparatorOp::Eq,
                right: lit_s("x"),
            },
            Predicate::InList {
                expr: col_unq("a"),
                list: vec![lit_i(1), lit_i(2)],
                negated: false,
            }
        ]);

        let q = PredicateResolver::qualify_predicate(&pred, &mut ctx).expect("qualify");
        // Ensure both columns got qualified to t.*
        let ok = match q {
            Predicate::And(v) => {
                // left part: Compare(lower(t.s), 'x')
                let lq_ok = match &v[0] {
                    Predicate::Compare { left: ScalarExpr::Function(Function { args, .. }), .. } => match &args[0] {
                        ScalarExpr::Column(Column::WithCollection{ collection, name }) => collection == "t" && name == "s",
                        _ => false
                    },
                    _ => false
                };
                // right part: InList(t.a, [1,2])
                let rq_ok = match &v[1] {
                    Predicate::InList { expr: ScalarExpr::Column(Column::WithCollection{ collection, name }), .. } => collection == "t" && name == "a",
                    _ => false
                };
                lq_ok && rq_ok
            }
            _ => false
        };
        assert!(ok, "columns should be qualified to t.*");
    }

    // ======================================================
    // qualify_predicate: errors
    // ======================================================

    #[test]
    fn qualify_predicate_errors_on_wildcard_outside_count() {
        // schema present but wildcard is not allowed in qualification
        let sp = DummySchemas::new().with("t", vec![("a", JsonPrimitive::Int, false)]);
        let mut ctx = ctx_for(&sp, &[("t", None)]);

        // LIKE(*, 'a%') → invalid due to wildcard outside COUNT(*)
        let bad = Predicate::Like {
            expr: ScalarExpr::WildCard,
            pattern: lit_s("a%"),
            negated: false
        };
        let err = PredicateResolver::qualify_predicate(&bad, &mut ctx);
        assert!(err.is_err());
        let msg = format!("{err:?}").to_lowercase();
        assert!(msg.contains("wildcards") || msg.contains("wildcard"), "unexpected error: {msg}");
    }

    #[test]
    fn qualify_predicate_unknown_collection_in_qualified_column() {
        // provider has "t", ctx exposes only "t"
        let sp = DummySchemas::new().with("t", vec![("a", JsonPrimitive::Int, false)]);
        let mut ctx = ctx_for(&sp, &[("t", None)]);

        // Compare(v.a, 1) where 'v' is not a visible collection
        let bad = Predicate::Compare {
            left: col_q("v", "a"),
            op: ComparatorOp::Eq,
            right: lit_i(1),
        };
        let err = PredicateResolver::qualify_predicate(&bad, &mut ctx);
        assert!(matches!(err, Err(AnalyzerError::UnknownCollection(c)) if c == "v"));
    }

    #[test]
    fn fold_compare_booleans_only_eq_noteq_defined() {
        use ComparatorOp::*;
        assert!(matches!(
            PredicateResolver::fold_predicate(&Predicate::Compare { left: lit_b(true), op: Eq, right: lit_b(true) }),
            Predicate::Const3(Truth::True)
        ));
        assert!(matches!(
            PredicateResolver::fold_predicate(&Predicate::Compare { left: lit_b(true), op: Lt, right: lit_b(false) }),
            Predicate::Const3(Truth::Unknown)
        ));
    }
}
