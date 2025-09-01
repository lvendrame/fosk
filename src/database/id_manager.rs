use std::fmt::Display;

use uuid::Uuid;

use crate::IdType;

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Default, Clone, PartialEq)]
pub struct IdManager {
    pub id_type: IdType,
    pub current: Option<IdValue>,
}

impl IdManager {
    pub fn new(id_type: IdType) -> Self {
        Self {
            id_type,
            current: None
        }
    }

    pub fn set_current(&mut self, value: IdValue) -> Result<(), String> {
        match (&self.id_type, &value) {
            (IdType::Int, IdValue::Int(_)) => {
                self.current = Some(value);
                Ok(())
            },
            (IdType::Uuid, IdValue::Uuid(_)) => {
                self.current = Some(value);
                Ok(())
            },
            (IdType::None, _) => {
                Err("Cannot set current value for IdType::None".to_string())
            },
            (IdType::Int, IdValue::Uuid(_)) => {
                Err("Cannot set UUID value for Int IdManager".to_string())
            },
            (IdType::Uuid, IdValue::Int(_)) => {
                Err("Cannot set Int value for UUID IdManager".to_string())
            },
        }
    }
}

impl Iterator for IdManager{
    type Item = IdValue;
    fn next(&mut self) -> Option<Self::Item> {
        let item = match &self.current {
            Some(IdValue::Int(id)) => match *id {
                u64::MAX => IdValue::Int(0),
                _ => IdValue::Int(id + 1)
            },
            Some(IdValue::Uuid(_)) => IdValue::Uuid(Uuid::new_v4().to_string()),
            None => match self.id_type {
                IdType::Int => IdValue::Int(1),
                IdType::Uuid => IdValue::Uuid(Uuid::new_v4().to_string()),
                IdType::None => return None,
            }
        };

        self.current = Some(item.clone());
        Some(item)
    }
}
