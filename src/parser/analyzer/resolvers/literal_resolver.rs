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
            (Literal::Float(x), Literal::Float(y)) => Self::float_eq(*x, *y),
            (Literal::Int(x),  Literal::Float(y)) => Self::float_eq(*x as f64, *y),
            (Literal::Float(x), Literal::Int(y))  => Self::float_eq(*x, *y as f64),
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
            (Literal::Float(a), Literal::Float(b)) => num_cmp(*a, *b),
            (Literal::Int(a),   Literal::Float(b)) => num_cmp(*a as f64, *b),
            (Literal::Float(a), Literal::Int(b))   => num_cmp(*a, *b as f64),
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
                        re.push('\\'); // trailing backslash, treat as literal
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
