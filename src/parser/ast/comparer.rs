#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Comparer {
    #[default]
    Equal,
    Different,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    Like,
    IsNull,
    IsNotNull,
    In,
    NotIn,
}

impl TryFrom<&str> for Comparer {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_uppercase().as_str() {
            "=" => Ok(Comparer::Equal),
            "!=" | "<>" => Ok(Comparer::Different),
            ">" => Ok(Comparer::GreaterThan),
            ">=" => Ok(Comparer::GreaterThanOrEqual),
            "<" => Ok(Comparer::LessThan),
            "<=" => Ok(Comparer::LessThanOrEqual),
            "LIKE" => Ok(Comparer::Like),
            "IS NULL" => Ok(Comparer::IsNull),
            "IS NOT NULL" => Ok(Comparer::IsNotNull),
            "IN" => Ok(Comparer::IsNull),
            "NOT IN" => Ok(Comparer::IsNotNull),
            _ => Err(format!("Invalid comparer operator: '{}'", value)),
        }
    }
}
