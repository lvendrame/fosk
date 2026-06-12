#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Truth {
    True,
    False,
    Unknown,
}

impl Truth {
    pub fn not(&self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown,
        }
    }

    pub fn and(&self, b: Self) -> Self {
        match (self, b) {
            (Self::False, _) | (_, Self::False) => Self::False,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::True, Self::True) => Self::True,
        }
    }

    pub fn or(&self, b: Self) -> Self {
        match (self, b) {
            (Self::True, _) | (_, Self::True) => Self::True,
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::False, Self::False) => Self::False,
        }
    }
}

use std::fmt;

impl fmt::Display for Truth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::True => write!(f, "True"),
            Self::False => write!(f, "False"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl fmt::Debug for Truth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Truth({})", self)
    }
}

#[cfg(test)]
mod tests {
    use super::Truth;

    #[test]
    fn not_uses_three_valued_logic() {
        assert_eq!(Truth::True.not(), Truth::False);
        assert_eq!(Truth::False.not(), Truth::True);
        assert_eq!(Truth::Unknown.not(), Truth::Unknown);
    }

    #[test]
    fn and_uses_three_valued_logic() {
        assert_eq!(Truth::False.and(Truth::True), Truth::False);
        assert_eq!(Truth::True.and(Truth::False), Truth::False);
        assert_eq!(Truth::Unknown.and(Truth::True), Truth::Unknown);
        assert_eq!(Truth::True.and(Truth::Unknown), Truth::Unknown);
        assert_eq!(Truth::True.and(Truth::True), Truth::True);
    }

    #[test]
    fn or_uses_three_valued_logic() {
        assert_eq!(Truth::True.or(Truth::False), Truth::True);
        assert_eq!(Truth::False.or(Truth::True), Truth::True);
        assert_eq!(Truth::Unknown.or(Truth::False), Truth::Unknown);
        assert_eq!(Truth::False.or(Truth::Unknown), Truth::Unknown);
        assert_eq!(Truth::False.or(Truth::False), Truth::False);
    }

    #[test]
    fn display_and_debug_name_truth_values() {
        assert_eq!(Truth::True.to_string(), "True");
        assert_eq!(Truth::False.to_string(), "False");
        assert_eq!(Truth::Unknown.to_string(), "Unknown");
        assert_eq!(format!("{:?}", Truth::Unknown), "Truth(Unknown)");
    }
}
