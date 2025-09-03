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
        let args = self.args.iter().map(|a| format!("{}", a)).collect::<Vec<_>>().join(", ");
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
