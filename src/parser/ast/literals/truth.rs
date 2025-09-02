#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Truth {
    True,
    False,
    Unknown
}

impl Truth {
    pub fn not(&self) -> Self {
        match self {
            Self::True => Self::False,
            Self::False => Self::True,
            Self::Unknown => Self::Unknown
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


