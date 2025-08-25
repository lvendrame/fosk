#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Operator {
    #[default]
    And,
    Or,
}

impl TryFrom<&str> for Operator {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_uppercase().as_str() {
            "AND" => Ok(Operator::And),
            "OR" => Ok(Operator::Or),
            _ => Err(format!("Invalid operator: '{}'", value)),
        }
    }
}
