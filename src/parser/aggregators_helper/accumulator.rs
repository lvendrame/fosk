use serde_json::Value;

use crate::parser::analyzer::AnalyzerError;

/// The per-group state.
/// The executor will:
///   1) evaluate the function's arguments per row into serde_json::Value
///   2) call `update(&mut self, &args)` (args.len() == fun.args.len())
///   3) after all rows in the group, call `finalize()`
///
/// DISTINCT is usually handled by the executor (wrapping the accumulator with a
/// value set) so `update` can just implement the non-distinct semantics.
pub trait Accumulator: Send {
    /// Update the running state with the evaluated arguments of this row.
    fn update(&mut self, args: &[Value]) -> Result<(), AnalyzerError>;

    /// Produce the final result as a JSON value.
    fn finalize(&self) -> Value;
}
