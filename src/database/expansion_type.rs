#[derive(Debug, Default, Clone)]
pub enum ExpansionType {
    #[default]
    None,
    Single(String),
    Child(String, Box<ExpansionType>),
}

impl From<&str> for ExpansionType {
    fn from(value: &str) -> Self {
        let parts: Vec<_> = value.split(".").collect();
        let mut last = None;
        for part in parts.iter().rev() {
            if !part.is_empty() {
                last = match last {
                    Some(exp) => Some(ExpansionType::Child(part.to_string(), Box::new(exp))),
                    None => Some(ExpansionType::Single(part.to_string())),
                }
            }
        }

        match last {
            Some(expansion_type) => expansion_type,
            None => ExpansionType::None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::ExpansionType;

    #[test]
    fn test_from_single() {
        let exp = ExpansionType::from("orders");
        if let ExpansionType::Single(s) = exp {
            assert_eq!(s, "orders");
        } else {
            panic!("Expected ExpansionType::Single variant");
        }
    }

    #[test]
    fn test_from_two_levels() {
        let exp = ExpansionType::from("orders.order_items");
        if let ExpansionType::Child(p1, boxed) = exp {
            assert_eq!(p1, "orders");
            if let ExpansionType::Single(s) = *boxed {
                assert_eq!(s, "order_items");
            } else {
                panic!("Expected nested ExpansionType::Single variant");
            }
        } else {
            panic!("Expected ExpansionType::Child variant");
        }
    }

    #[test]
    fn test_from_three_levels() {
        let exp = ExpansionType::from("orders.order_items.products");
        if let ExpansionType::Child(p1, boxed1) = exp {
            assert_eq!(p1, "orders");
            if let ExpansionType::Child(p2, boxed2) = *boxed1 {
                assert_eq!(p2, "order_items");
                if let ExpansionType::Single(s) = *boxed2 {
                    assert_eq!(s, "products");
                } else {
                    panic!("Expected innermost ExpansionType::Single variant");
                }
            } else {
                panic!("Expected nested ExpansionType::Child variant");
            }
        } else {
            panic!("Expected root ExpansionType::Child variant");
        }
    }

    #[test]
    fn test_from_empty() {
        let exp = ExpansionType::from("");
        match exp {
            ExpansionType::None => {},
            _ => panic!("Expected ExpansionType::None variant for empty input"),
        }
    }
}
