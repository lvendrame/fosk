use std::fmt::Display;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::IdType;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IdValue {
    Uuid(String),
    Int(u64),
}

impl Display for IdValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IdValue::Uuid(uuid) => f.write_str(uuid),
            IdValue::Int(id) => write!(f, "{id}"),
        }
    }
}

impl From<Value> for IdValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Number(number) => IdValue::Int(number.as_u64().unwrap()),
            Value::String(value) => IdValue::Uuid(value),
            _ => IdValue::Uuid(value.to_string()),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct IdManager {
    pub id_type: IdType,
    pub current: Option<IdValue>,
}

impl IdManager {
    pub fn new(id_type: IdType) -> Self {
        Self {
            id_type,
            current: None,
        }
    }

    pub fn set_current(&mut self, value: IdValue) -> Result<(), String> {
        match (&self.id_type, &value) {
            (IdType::Int, IdValue::Int(_)) => {
                self.current = Some(value);
                Ok(())
            }
            (IdType::Uuid, IdValue::Uuid(_)) => {
                self.current = Some(value);
                Ok(())
            }
            (IdType::None, _) => Err("Cannot set current value for IdType::None".to_string()),
            (IdType::Int, IdValue::Uuid(_)) => {
                Err("Cannot set UUID value for Int IdManager".to_string())
            }
            (IdType::Uuid, IdValue::Int(_)) => {
                Err("Cannot set Int value for UUID IdManager".to_string())
            }
        }
    }
}

impl Iterator for IdManager {
    type Item = IdValue;
    fn next(&mut self) -> Option<Self::Item> {
        let item = match &self.current {
            Some(IdValue::Int(id)) => match *id {
                u64::MAX => IdValue::Int(0),
                _ => IdValue::Int(id + 1),
            },
            Some(IdValue::Uuid(_)) => IdValue::Uuid(Uuid::new_v4().to_string()),
            None => match self.id_type {
                IdType::Int => IdValue::Int(1),
                IdType::Uuid => IdValue::Uuid(Uuid::new_v4().to_string()),
                IdType::None => return None,
            },
        };

        self.current = Some(item);
        self.current.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{IdManager, IdValue};
    use crate::IdType;
    use serde_json::json;

    #[test]
    fn id_value_display_formats_inner_value() {
        assert_eq!(IdValue::Int(42).to_string(), "42");
        assert_eq!(IdValue::Uuid("abc".to_string()).to_string(), "abc");
    }

    #[test]
    fn id_value_from_json_maps_numbers_to_int_and_others_to_uuid() {
        assert_eq!(IdValue::from(json!(7)), IdValue::Int(7));
        assert_eq!(IdValue::from(json!("abc")), IdValue::Uuid("abc".to_string()));
        assert_eq!(IdValue::from(json!(true)), IdValue::Uuid("true".to_string()));
    }

    #[test]
    fn set_current_accepts_values_matching_manager_type() {
        let mut int_manager = IdManager::new(IdType::Int);
        assert_eq!(int_manager.set_current(IdValue::Int(9)), Ok(()));
        assert_eq!(int_manager.current, Some(IdValue::Int(9)));

        let mut uuid_manager = IdManager::new(IdType::Uuid);
        assert_eq!(uuid_manager.set_current(IdValue::Uuid("abc".to_string())), Ok(()));
        assert_eq!(uuid_manager.current, Some(IdValue::Uuid("abc".to_string())));
    }

    #[test]
    fn set_current_rejects_mismatched_or_disabled_id_types() {
        let mut none_manager = IdManager::new(IdType::None);
        assert_eq!(
            none_manager.set_current(IdValue::Int(1)),
            Err("Cannot set current value for IdType::None".to_string())
        );

        let mut int_manager = IdManager::new(IdType::Int);
        assert_eq!(
            int_manager.set_current(IdValue::Uuid("abc".to_string())),
            Err("Cannot set UUID value for Int IdManager".to_string())
        );

        let mut uuid_manager = IdManager::new(IdType::Uuid);
        assert_eq!(
            uuid_manager.set_current(IdValue::Int(1)),
            Err("Cannot set Int value for UUID IdManager".to_string())
        );
    }

    #[test]
    fn iterator_generates_int_ids_from_empty_current_and_existing_current() {
        let mut manager = IdManager::new(IdType::Int);

        assert_eq!(manager.next(), Some(IdValue::Int(1)));
        assert_eq!(manager.next(), Some(IdValue::Int(2)));
    }

    #[test]
    fn iterator_wraps_int_id_after_max_value() {
        let mut manager = IdManager::new(IdType::Int);
        manager.set_current(IdValue::Int(u64::MAX)).unwrap();

        assert_eq!(manager.next(), Some(IdValue::Int(0)));
    }

    #[test]
    fn iterator_generates_uuid_ids_and_none_for_disabled_ids() {
        let mut uuid_manager = IdManager::new(IdType::Uuid);
        let first = uuid_manager.next();
        let second = uuid_manager.next();

        assert!(matches!(first, Some(IdValue::Uuid(ref id)) if id.contains('-')));
        assert!(matches!(second, Some(IdValue::Uuid(ref id)) if id.contains('-')));
        assert_ne!(first, second);

        let mut none_manager = IdManager::new(IdType::None);
        assert_eq!(none_manager.next(), None);
    }
}
