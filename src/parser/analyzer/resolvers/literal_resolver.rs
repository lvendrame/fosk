use crate::parser::ast::{ComparatorOp, Literal};

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
            (Literal::Int(x), Literal::Int(y)) => x == y,
            (Literal::Float(x), Literal::Float(y)) => Self::float_eq(*x as f64, *y as f64),
            (Literal::Int(x), Literal::Float(y)) => Self::float_eq(*x as f64, *y as f64),
            (Literal::Float(x), Literal::Int(y)) => Self::float_eq(*x as f64, *y as f64),
            (Literal::String(x), Literal::String(y)) => x == y,
            _ => false,
        }
    }

    pub fn eval_compare(l: &Literal, op: ComparatorOp, r: &Literal) -> bool {
        let compare = |x: f64, y: f64| match op {
            ComparatorOp::Eq    => Self::float_eq(x, y),
            ComparatorOp::NotEq => !Self::float_eq(x, y),
            ComparatorOp::Lt    => x <  y,
            ComparatorOp::LtEq  => x <= y || Self::float_eq(x, y),
            ComparatorOp::Gt    => x >  y,
            ComparatorOp::GtEq  => x >= y || Self::float_eq(x, y),
        };

        match (l, r) {
            (Literal::Null, _) | (_, Literal::Null) => false,
            (Literal::Bool(a), Literal::Bool(b)) => match op {
                ComparatorOp::Eq => a == b, ComparatorOp::NotEq => a != b, _ => false
            },
            (Literal::Int(a), Literal::Int(b)) => compare(*a as f64, *b as f64),
            (Literal::Float(a), Literal::Float(b)) => compare(*a as f64, *b as f64),
            (Literal::Int(a), Literal::Float(b)) => compare(*a as f64, *b as f64),
            (Literal::Float(a), Literal::Int(b)) => compare(*a as f64, *b as f64),
            (Literal::String(a), Literal::String(b)) => match op {
                ComparatorOp::Eq => a == b, ComparatorOp::NotEq => a != b, _ => false
            },
            _ => false
        }
    }

    pub fn eval_like(value: &str, pattern: &str) -> bool {
        // very small LIKE: % -> .*  _ -> .  (no escapes)
        let mut regex = String::from("^");
        for ch in pattern.chars() {
            match ch {
                '%' => regex.push_str(".*"),
                '_' => regex.push('.'),
                // naive escaping of regex meta
                '.' | '+' | '(' | ')' | '|' | '^' | '$' | '{' | '}' | '[' | ']' | '\\' => {
                    regex.push('\\'); regex.push(ch);
                }
                c => regex.push(c),
            }
        }
        regex.push('$');
        regex::Regex::new(&regex).map(|re| re.is_match(value)).unwrap_or(false)
    }
}
