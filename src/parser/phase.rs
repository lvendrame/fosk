#[derive(Debug, Default, PartialEq, PartialOrd)]
pub enum Phase {
    #[default]
    Projection = 0,
    Collections = 1,
    Joins = 2,
    Criteria = 3,
    Aggregates = 4,
    Having = 5,
    OrderBy = 6,
    LimitAndOffset = 7,
    EOF = 8,
}
