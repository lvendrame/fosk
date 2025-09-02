use crate::parser::ast::{ComparatorOp, Literal, Truth};

pub struct LiteralResolver;

impl LiteralResolver {
    #[inline]
    fn float_eq(a: f64, b: f64) -> bool {
        let diff = (a - b).abs();
        let eps = 1e-9_f64.max(1e-9_f64 * a.abs()).max(1e-9_f64 * b.abs());
        diff <= eps
    }

    pub fn literal_equal(a: &Literal, b: &Literal) -> bool {
        match (a, b) {
            (Literal::Null, Literal::Null) => true,
            (Literal::Bool(x), Literal::Bool(y)) => x == y,
            (Literal::Int(x),  Literal::Int(y))  => x == y,
            (Literal::Float(x), Literal::Float(y)) => Self::float_eq(x.into_inner(), y.into_inner()),
            (Literal::Int(x),  Literal::Float(y)) => Self::float_eq(*x as f64, y.into_inner()),
            (Literal::Float(x), Literal::Int(y))  => Self::float_eq(x.into_inner(), *y as f64),
            (Literal::String(x), Literal::String(y)) => x == y,
            _ => false,
        }
    }

    pub fn eval_compare3(l: &Literal, op: ComparatorOp, r: &Literal) -> Truth {
        use Truth::*;

        if matches!(l, Literal::Null) || matches!(r, Literal::Null) { return Unknown; }

        let num_cmp = |x: f64, y: f64| -> Truth {
            match op {
                ComparatorOp::Eq    => if Self::float_eq(x, y) { True } else { False },
                ComparatorOp::NotEq => if Self::float_eq(x, y) { False } else { True },
                ComparatorOp::Lt    => if x <  y { True } else { False },
                ComparatorOp::LtEq  => if x <= y || Self::float_eq(x, y) { True } else { False },
                ComparatorOp::Gt    => if x >  y { True } else { False },
                ComparatorOp::GtEq  => if x >= y || Self::float_eq(x, y) { True } else { False },
            }
        };

        match (l, r) {
            (Literal::Bool(a), Literal::Bool(b)) => match op {
                ComparatorOp::Eq => if a == b { True } else { False },
                ComparatorOp::NotEq => if a != b { True } else { False },
                _ => Unknown, // SQL doesn't define <,> on booleans
            },
            // exact integer/same-signed
            (Literal::Int(a),  Literal::Int(b))  => num_cmp(*a as f64, *b as f64),
            // floats
            (Literal::Float(a), Literal::Float(b)) => num_cmp(a.into_inner(), b.into_inner()),
            (Literal::Int(a),   Literal::Float(b)) => num_cmp(*a as f64, b.into_inner()),
            (Literal::Float(a), Literal::Int(b))   => num_cmp(a.into_inner(), *b as f64),
            (Literal::String(a), Literal::String(b)) => match op {
                ComparatorOp::Eq    => if a == b { True } else { False },
                ComparatorOp::NotEq => if a != b { True } else { False },
                _ => Unknown,
            },
            _ => Unknown,
        }
    }

    pub fn eval_like(value: &str, pattern: &str) -> Truth {
        // very small LIKE: % -> .*  _ -> .  (no escapes)
        let mut re = String::from("(?i)^");
        let mut chars = pattern.chars().peekable();
        while let Some(ch) = chars.next() {
            match ch {
                '\\' => { // escape next char literally
                    if let Some(n) = chars.next() {
                        re.push_str(&regex::escape(&n.to_string()));
                    } else {
                        re.push_str("\\\\"); // trailing backslash, treat as literal
                    }
                }
                '%' => re.push_str(".*"),
                '_' => re.push('.'),
                other => re.push_str(&regex::escape(&other.to_string())),
            }
        }
        re.push('$');

        match regex::Regex::new(&re) {
            Ok(rx) => if rx.is_match(value) { Truth::True } else { Truth::False },
            Err(_) => Truth::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lf(v: f64) -> Literal { Literal::Float(ordered_float::NotNan::new(v).unwrap()) }

    // --- float_eq -------------------------------------------------------------

    #[test]
    fn float_eq_tolerates_small_eps() {
        // 1e-12 difference should be considered equal with eps=1e-9
        assert!(LiteralResolver::float_eq(1.0, 1.0 + 1e-12));
        assert!(LiteralResolver::float_eq(-2.5, -2.5 - 1e-12));
        // Larger diff shouldn't be equal
        assert!(!LiteralResolver::float_eq(1.0, 1.0 + 1e-6));
    }

    // --- literal_equal --------------------------------------------------------

    #[test]
    fn literal_equal_basic_and_cross_numeric() {
        use Literal::*;
        assert!(LiteralResolver::literal_equal(&Null, &Null));
        assert!(LiteralResolver::literal_equal(&Bool(true), &Bool(true)));
        assert!(!LiteralResolver::literal_equal(&Bool(true), &Bool(false)));

        assert!(LiteralResolver::literal_equal(&Int(42), &Int(42)));
        assert!(!LiteralResolver::literal_equal(&Int(42), &Int(41)));

    assert!(LiteralResolver::literal_equal(&lf(3.0), &lf(3.0 + 1e-12)));
    assert!(!LiteralResolver::literal_equal(&lf(3.0), &lf(3.001)));

        // cross int/float numeric equality
    assert!(LiteralResolver::literal_equal(&Int(3), &lf(3.0)));
    assert!(LiteralResolver::literal_equal(&lf(5.0), &Int(5)));
    assert!(!LiteralResolver::literal_equal(&Int(3), &lf(3.01)));

        // strings
        assert!(LiteralResolver::literal_equal(&String("abc".into()), &String("abc".into())));
        assert!(!LiteralResolver::literal_equal(&String("abc".into()), &String("Abc".into())));

        // mismatched types
        assert!(!LiteralResolver::literal_equal(&String("1".into()), &Int(1)));
        assert!(!LiteralResolver::literal_equal(&Bool(true), &Int(1)));
        assert!(!LiteralResolver::literal_equal(&Null, &Int(0)));
    }

    // --- eval_compare3: numbers ----------------------------------------------

    #[test]
    fn compare3_numeric_variants_and_edges() {
        use ComparatorOp::*;
        use Literal::*;
        use Truth::*;

        // equality with tolerance
    assert_eq!(LiteralResolver::eval_compare3(&lf(1.0), Eq, &lf(1.0 + 1e-12)), True);
    assert_eq!(LiteralResolver::eval_compare3(&Int(5), Eq, &lf(5.0)), True);

        // inequalities int-int
        assert_eq!(LiteralResolver::eval_compare3(&Int(2), Lt, &Int(3)), True);
        assert_eq!(LiteralResolver::eval_compare3(&Int(2), Gt, &Int(3)), False);
        assert_eq!(LiteralResolver::eval_compare3(&Int(3), LtEq, &Int(3)), True);
        assert_eq!(LiteralResolver::eval_compare3(&Int(3), GtEq, &Int(3)), True);

    // inequalities int-float
    assert_eq!(LiteralResolver::eval_compare3(&Int(2), Lt, &lf(2.000001)), True);
    assert_eq!(LiteralResolver::eval_compare3(&lf(2.0), Gt, &Int(1)), True);

        // strings: only Eq/NotEq defined
        assert_eq!(LiteralResolver::eval_compare3(&String("a".into()), Eq, &String("a".into())), True);
        assert_eq!(LiteralResolver::eval_compare3(&String("a".into()), NotEq, &String("b".into())), True);
        assert_eq!(LiteralResolver::eval_compare3(&String("a".into()), Lt, &String("b".into())), Unknown);
    }

    // --- eval_compare3: booleans & nulls -------------------------------------

    #[test]
    fn compare3_booleans_and_null_semantics() {
        use ComparatorOp::*;
        use Literal::*;
        use Truth::*;

        // booleans: only Eq/NotEq defined
        assert_eq!(LiteralResolver::eval_compare3(&Bool(true), Eq, &Bool(true)), True);
        assert_eq!(LiteralResolver::eval_compare3(&Bool(true), NotEq, &Bool(false)), True);
        assert_eq!(LiteralResolver::eval_compare3(&Bool(true), Lt, &Bool(false)), Unknown);

        // any compare with NULL -> Unknown
        assert_eq!(LiteralResolver::eval_compare3(&Null, Eq, &Int(1)), Unknown);
        assert_eq!(LiteralResolver::eval_compare3(&Int(1), Gt, &Null), Unknown);
        assert_eq!(LiteralResolver::eval_compare3(&Null, NotEq, &Null), Unknown); // SQL 3VL: NULL <> NULL => Unknown
    }

    // --- eval_compare3: boundary with tolerance on <= and >= -----------------

    #[test]
    fn compare3_le_ge_with_tiny_delta() {
        use ComparatorOp::*;
        use Truth::*;

        // x <= y when x is just slightly above y within epsilon? Our impl treats
        // <= as (x <= y) OR float_eq(x,y). With x slightly below y, definitely True.
        assert_eq!(LiteralResolver::eval_compare3(&lf(1.0), LtEq, &lf(1.0 + 1e-12)), True);
        assert_eq!(LiteralResolver::eval_compare3(&lf(1.0 + 1e-12), GtEq, &lf(1.0)), True);
    }

    // --- eval_like ------------------------------------------------------------

    #[test]
    fn like_case_insensitive_and_simple_wildcards() {
        use Truth::*;
        // case-insensitive
        assert_eq!(LiteralResolver::eval_like("Hello", "he%"), True);
        // underscore
        assert_eq!(LiteralResolver::eval_like("abc", "a_c"), True);
        // basic mismatch
        assert_eq!(LiteralResolver::eval_like("abc", "a_d"), False);
    }

    #[test]
    fn like_with_escape_sequences_and_trailing_backslash() {
        use Truth::*;

        // Escape % so it's literal
        assert_eq!(LiteralResolver::eval_like("he%llo", r"he\%l%"), True);

        // Escape _ so it's literal
        assert_eq!(LiteralResolver::eval_like("a_c", r"a\_c"), True);

        // Trailing backslash treated literally
        assert_eq!(LiteralResolver::eval_like(r"abc\", r"abc\"), True);

        // Escaping a normal char just matches that char
        assert_eq!(LiteralResolver::eval_like("a.b", r"a\.b"), True);

        // Wrong escape (pattern expects literal %) but value has no %
        assert_eq!(LiteralResolver::eval_like("hello", r"he\%llo"), False);
    }
}
