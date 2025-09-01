#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum IdType {
    #[default]
    Uuid,
    Int,
    None,
}

