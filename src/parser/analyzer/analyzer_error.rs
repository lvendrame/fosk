use crate::JsonPrimitive;

#[derive(Debug, Clone, PartialEq)]
pub enum AnalyzerError {
    UnknownCollection(String),
    UnknownColumn { name: String, candidates: Vec<String> },
    AmbiguousColumn { name: String, matches: Vec<(String,String)> }, // (coll, col)
    NotACollection(String),
    FunctionNotFound(String),
    FunctionArgMismatch { name: String, expected: String, got: Vec<JsonPrimitive> },
    NonConstInConstFold,
    InvalidLikePattern,
    InvalidParameterValue,
    Other(String),
}
