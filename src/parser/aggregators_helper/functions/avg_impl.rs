use serde_json::Value;

use crate::{
    JsonPrimitive,
    parser::{
        aggregators_helper::{Accumulator, AggregateImpl},
        analyzer::{AnalysisContext, AnalyzerError},
        ast::Function,
    },
};

pub struct AvgImpl;
impl AggregateImpl for AvgImpl {
    fn name(&self) -> &'static str {
        "avg"
    }
    fn infer_type(
        &self,
        fun: &Function,
        ctx: &AnalysisContext,
    ) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        if let Some(arg) = fun.args.first() {
            let (t, _n) = crate::parser::analyzer::TypeInference::infer_scalar(arg, ctx)?;
            match t {
                JsonPrimitive::Int | JsonPrimitive::Float => Ok((JsonPrimitive::Float, true)),
                other => Err(AnalyzerError::FunctionArgMismatch {
                    name: fun.name.clone(),
                    expected: "numeric".into(),
                    got: vec![other],
                }),
            }
        } else {
            Err(AnalyzerError::FunctionArgMismatch {
                name: fun.name.clone(),
                expected: "AVG(arg)".into(),
                got: vec![],
            })
        }
    }
    fn create_accumulator(&self) -> Box<dyn Accumulator> {
        Box::new(AvgAcc { sum: 0.0, cnt: 0 })
    }
}

struct AvgAcc {
    sum: f64,
    cnt: i64,
}
impl Accumulator for AvgAcc {
    fn update(&mut self, args: &[Value]) -> Result<(), AnalyzerError> {
        let [v] = args else {
            return Err(AnalyzerError::FunctionArgMismatch {
                name: "AVG".into(),
                expected: "AVG(expr)".into(),
                got: vec![],
            });
        };
        match v {
            Value::Null => {}
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    self.sum += i as f64;
                    self.cnt += 1;
                } else if let Some(f) = n.as_f64() {
                    self.sum += f;
                    self.cnt += 1;
                } else {
                    return Err(AnalyzerError::Other("AVG got non numeric number".into()));
                }
            }
            _ => return Err(AnalyzerError::Other("AVG got non numeric arg".into())),
        }
        Ok(())
    }
    fn finalize(&self) -> Value {
        if self.cnt == 0 {
            Value::Null
        } else {
            let avg = self.sum / (self.cnt as f64);
            serde_json::Number::from_f64(avg)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AvgImpl;
    use crate::parser::aggregators_helper::AggregateImpl;
    use serde_json::json;

    #[test]
    fn accumulator_averages_numbers_and_ignores_nulls() {
        let mut acc = AvgImpl.create_accumulator();

        acc.update(&[json!(2)]).unwrap();
        acc.update(&[json!(null)]).unwrap();
        acc.update(&[json!(4.0)]).unwrap();

        assert_eq!(acc.finalize(), json!(3.0));
    }

    #[test]
    fn accumulator_returns_null_when_no_values_seen() {
        let acc = AvgImpl.create_accumulator();

        assert_eq!(acc.finalize(), json!(null));
    }

    #[test]
    fn accumulator_rejects_bad_arg_count_and_non_numeric_values() {
        let mut acc = AvgImpl.create_accumulator();

        assert!(acc.update(&[]).is_err());
        assert!(acc.update(&[json!("bad")]).is_err());
    }
}
