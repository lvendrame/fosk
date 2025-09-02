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

    pub fn qualify_predicate(predicate: &Predicate, ctx: &AnalysisContext) -> Result<Predicate, AnalyzerError> {
        Ok(match predicate {
            Predicate::And(v) => Predicate::And(v.iter().map(|x| Self::qualify_predicate(x, ctx)).collect::<Result<Vec<_>,_>>()?),
            Predicate::Or(v)  => Predicate::Or(v.iter().map(|x| Self::qualify_predicate(x, ctx)).collect::<Result<Vec<_>,_>>()?),
            Predicate::Compare { left, op, right } => {
                        Predicate::Compare { left: ScalarResolver::qualify_scalar(left, ctx)?, op: *op, right: ScalarResolver::qualify_scalar(right, ctx)? }
                    },
            Predicate::IsNull { expr, negated } => {
                        Predicate::IsNull { expr: ScalarResolver::qualify_scalar(expr, ctx)?, negated: *negated }
                    },
            Predicate::InList { expr, list, negated } => {
                        Predicate::InList { expr: ScalarResolver::qualify_scalar(expr, ctx)?, list: list.iter().map(|x| ScalarResolver::qualify_scalar(x, ctx)).collect::<Result<Vec<_>,_>>()?, negated: *negated }
                    },
            Predicate::Like { expr, pattern, negated } => {
                        Predicate::Like { expr: ScalarResolver::qualify_scalar(expr, ctx)?, pattern: ScalarResolver::qualify_scalar(pattern, ctx)?, negated: *negated }
                    },
            Predicate::Const3(value) => Predicate::Const3(*value),
        })
    }

}


#[cfg(test)]
mod test {
    use crate::parser::{analyzer::PredicateResolver, ast::{
        Literal::*, Predicate, ScalarExpr::*, Truth
    }};

    #[test]
    fn fold_is_null_and_not_null() {
        // IS NULL
        let p = Predicate::IsNull { expr: Literal(Null), negated: false };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
        // IS NOT NULL
        let p = Predicate::IsNull { expr: Literal(String("x".into())), negated: true };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
    }

    #[test]
    fn fold_in_and_not_in() {
        // 5 IN (1,5,7) -> true
        let p = Predicate::InList { expr: Literal(Int(5)), list: vec![Literal(Int(1)), Literal(Int(5)), Literal(Int(7))], negated: false };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
        // 5 NOT IN (1,5,7) -> false
        let p = Predicate::InList { expr: Literal(Int(5)), list: vec![Literal(Int(1)), Literal(Int(5)), Literal(Int(7))], negated: true };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::False));
    }

    #[test]
    fn fold_like_and_not_like() {
        // 'hello' LIKE 'he%%' -> true
        let p = Predicate::Like { expr: Literal(String("hello".into())), pattern: Literal(String("he%%".into())), negated: false };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
        // NOT LIKE
        let p = Predicate::Like { expr: Literal(String("hello".into())), pattern: Literal(String("x%".into())), negated: true };
        assert_eq!(PredicateResolver::fold_predicate(&p), Predicate::Const3(Truth::True));
    }
}
