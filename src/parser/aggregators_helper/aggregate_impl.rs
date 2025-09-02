use crate::{parser::{
    aggregators_helper::Accumulator,
    analyzer::{AnalysisContext, AnalyzerError},
    ast::Function
}, JsonPrimitive};

/// Per-aggregate metadata + factory.
/// One instance is registered globally per function name.
/// It is stateless and thread-safe to share.
pub trait AggregateImpl: Send + Sync {
    /// Canonical lowercase function name ("count", "sum", ...).
    fn name(&self) -> &'static str;

    /// Type inference for this function.
    /// - args are as in the parsed Function (qualified & folded already).
    /// - return (type, nullable)
    fn infer_type(&self, fun: &Function, ctx: &AnalysisContext) -> Result<(JsonPrimitive, bool), AnalyzerError>;

    /// Whether constant folding is allowed for this aggregate (usually false).
    fn allow_fold(&self) -> bool { false }

    /// Create a fresh accumulator instance for one group.
    fn create_accumulator(&self) -> Box<dyn Accumulator>;
}
