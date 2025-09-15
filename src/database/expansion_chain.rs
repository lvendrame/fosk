#[derive(Debug, Default, Clone)]
pub enum ExpansionChain {
    #[default]
    None,
    Single(String),
    Child(String, Box<ExpansionChain>),
}

impl From<&str> for ExpansionChain {
    fn from(value: &str) -> Self {
        let parts: Vec<_> = value.split(".").collect();
        let mut last = None;
        for part in parts.iter().rev() {
            if !part.is_empty() {
                last = match last {
                    Some(exp) => Some(ExpansionChain::Child(part.to_string(), Box::new(exp))),
                    None => Some(ExpansionChain::Single(part.to_string())),
                }
            }
        }

        match last {
            Some(expansion_type) => expansion_type,
            None => ExpansionChain::None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::ExpansionChain;

    #[test]
    fn test_from_single() {
        let exp = ExpansionChain::from("orders");
        if let ExpansionChain::Single(s) = exp {
            assert_eq!(s, "orders");
        } else {
            panic!("Expected ExpansionChain::Single variant");
        }
    }

    #[test]
    fn test_from_two_levels() {
        let exp = ExpansionChain::from("orders.order_items");
        if let ExpansionChain::Child(p1, boxed) = exp {
            assert_eq!(p1, "orders");
            if let ExpansionChain::Single(s) = *boxed {
                assert_eq!(s, "order_items");
            } else {
                panic!("Expected nested ExpansionChain::Single variant");
            }
        } else {
            panic!("Expected ExpansionChain::Child variant");
        }
    }

    #[test]
    fn test_from_three_levels() {
        let exp = ExpansionChain::from("orders.order_items.products");
        if let ExpansionChain::Child(p1, boxed1) = exp {
            assert_eq!(p1, "orders");
            if let ExpansionChain::Child(p2, boxed2) = *boxed1 {
                assert_eq!(p2, "order_items");
                if let ExpansionChain::Single(s) = *boxed2 {
                    assert_eq!(s, "products");
                } else {
                    panic!("Expected innermost ExpansionChain::Single variant");
                }
            } else {
                panic!("Expected nested ExpansionChain::Child variant");
            }
        } else {
            panic!("Expected root ExpansionChain::Child variant");
        }
    }

    #[test]
    fn test_from_empty() {
        let exp = ExpansionChain::from("");
        match exp {
            ExpansionChain::None => {},
            _ => panic!("Expected ExpansionChain::None variant for empty input"),
        }
    }
}
