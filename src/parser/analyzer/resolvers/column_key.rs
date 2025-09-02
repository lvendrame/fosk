use crate::parser::ast::Column;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ColumnKey { column: String, name: String }

impl ColumnKey {
    pub fn of(col: &Column) -> Self {
        match col {
            Column::WithCollection { collection, name } => Self {
                column: collection.clone(),
                name: name.clone(),
            },
            Column::Name { name } => Self {
                column: String::new(),
                name: name.clone()
            },
        }
    }
}
