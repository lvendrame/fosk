use crate::parser::ast::ScalarExpr;
use std::fmt;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Function {
    pub name: String,
    pub args: Vec<ScalarExpr>,
    pub distinct: bool,
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let args = self
            .args
            .iter()
            .map(|a| format!("{}", a))
            .collect::<Vec<_>>()
            .join(", ");
        if self.distinct {
            write!(f, "{}(distinct {})", self.name, args)
        } else {
            write!(f, "{}({})", self.name, args)
        }
    }
}

impl fmt::Debug for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Function({})", self)
    }
}

#[cfg(test)]
mod tests {
    use super::Function;
    use crate::parser::ast::{Literal, ScalarExpr};

    fn int_arg(value: i64) -> ScalarExpr {
        ScalarExpr::Literal(Literal::Int(value))
    }

    fn string_arg(value: &str) -> ScalarExpr {
        ScalarExpr::Literal(Literal::String(value.to_string()))
    }

    #[test]
    fn display_formats_regular_function_call() {
        let function = Function {
            name: "lower".to_string(),
            args: vec![string_arg("Ada")],
            distinct: false,
        };

        assert_eq!(function.to_string(), "lower(lit: s: \"Ada\")");
        assert_eq!(
            format!("{:?}", function),
            "Function(lower(lit: s: \"Ada\"))"
        );
    }

    #[test]
    fn display_formats_distinct_function_call_with_multiple_args() {
        let function = Function {
            name: "count".to_string(),
            args: vec![int_arg(1), string_arg("Ada")],
            distinct: true,
        };

        assert_eq!(
            function.to_string(),
            "count(distinct lit: i: 1, lit: s: \"Ada\")"
        );
    }
}
