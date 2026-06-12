#[derive(Debug, Default, Clone, PartialEq, Eq)]
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
        assert_eq!(
            ExpansionChain::from("orders"),
            ExpansionChain::Single("orders".to_string())
        );
    }

    #[test]
    fn test_from_two_levels() {
        assert_eq!(
            ExpansionChain::from("orders.order_items"),
            ExpansionChain::Child(
                "orders".to_string(),
                Box::new(ExpansionChain::Single("order_items".to_string()))
            )
        );
    }

    #[test]
    fn test_from_three_levels() {
        assert_eq!(
            ExpansionChain::from("orders.order_items.products"),
            ExpansionChain::Child(
                "orders".to_string(),
                Box::new(ExpansionChain::Child(
                    "order_items".to_string(),
                    Box::new(ExpansionChain::Single("products".to_string()))
                ))
            )
        );
    }

    #[test]
    fn test_from_empty() {
        assert_eq!(ExpansionChain::from(""), ExpansionChain::None);
    }

    #[test]
    fn test_from_ignores_empty_path_segments() {
        assert_eq!(
            ExpansionChain::from(".orders..order_items."),
            ExpansionChain::Child(
                "orders".to_string(),
                Box::new(ExpansionChain::Single("order_items".to_string()))
            )
        );
    }

    #[test]
    fn default_is_none() {
        assert!(matches!(ExpansionChain::default(), ExpansionChain::None));
    }
}
