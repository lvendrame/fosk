use serde_json::Value;

use crate::{parser::{aggregators_helper::{Accumulator, AggregateImpl}, analyzer::{AnalysisContext, AnalyzerError}, ast::Function}, JsonPrimitive};

pub struct CountImpl;

impl AggregateImpl for CountImpl {
    fn name(&self) -> &'static str { "count" }

    fn infer_type(&self, fun: &Function, _ctx: &AnalysisContext) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        // COUNT(*) | COUNT(expr) | COUNT(DISTINCT expr) -> Int, non-nullable
        if fun.args.len() == 1 {
            Ok((JsonPrimitive::Int, false))
        } else {
            Err(AnalyzerError::FunctionArgMismatch {
                name: fun.name.clone(),
                expected: "COUNT(*|expr)".into(),
                got: vec![],
            })
        }
    }

    fn create_accumulator(&self) -> Box<dyn Accumulator> {
        Box::new(CountAcc { cnt: 0, is_star: false })
    }
}

struct CountAcc {
    cnt: i64,
    // We will detect `*` at the first update call via args.len()
    // but you can set this from executor if you prefer.
    is_star: bool,
}

impl Accumulator for CountAcc {
    fn update(&mut self, args: &[Value]) -> Result<(), AnalyzerError> {
        match args {
            // COUNT(*) -> executor should pass an empty slice or a sentinel.
            // Here we accept 0 args as COUNT(*).
            [] => {
                self.is_star = true;
                self.cnt += 1;
            }
            // COUNT(expr): increment if expr != NULL
            [v] => {
                if matches!(v, Value::Null) {
                    // do nothing
                } else {
                    self.cnt += 1;
                }
            }
            _ => {
                return Err(AnalyzerError::FunctionArgMismatch {
                    name: "COUNT".into(),
                    expected: "COUNT(*|expr)".into(),
                    got: vec![],
                })
            }
        }
        Ok(())
    }

    fn finalize(&self) -> Value {
        Value::Number(serde_json::Number::from(self.cnt))
    }
}

#[cfg(test)]
mod tests {
    use super::CountImpl;
    use crate::parser::aggregators_helper::AggregateImpl;
    use serde_json::json;

    #[test]
    fn accumulator_counts_star_and_non_null_values_only() {
        let mut acc = CountImpl.create_accumulator();

        acc.update(&[]).unwrap();
        acc.update(&[json!("Ada")]).unwrap();
        acc.update(&[json!(null)]).unwrap();

        assert_eq!(acc.finalize(), json!(2));
    }

    #[test]
    fn accumulator_rejects_multiple_arguments() {
        let mut acc = CountImpl.create_accumulator();

        assert!(acc.update(&[json!(1), json!(2)]).is_err());
    }

    #[test]
    fn aggregate_impl_default_does_not_allow_folding() {
        assert!(!CountImpl.allow_fold());
    }
}
