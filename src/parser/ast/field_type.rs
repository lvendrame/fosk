#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum FieldType {
    #[default]
    String,
    Integer,
    Float,
    Boolean,
}
