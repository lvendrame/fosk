use serde_json::Value;

use crate::{parser::{aggregators_helper::{Accumulator, AggregateImpl}, analyzer::{AnalysisContext, AnalyzerError}, ast::Function}, JsonPrimitive};

pub struct AvgImpl;
impl AggregateImpl for AvgImpl {
    fn name(&self) -> &'static str { "avg" }
    fn infer_type(&self, fun: &Function, ctx: &AnalysisContext) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        if let Some(arg) = fun.args.first() {
            let (t, _n) = crate::parser::analyzer::TypeInference::infer_scalar(arg, ctx)?;
            match t {
                JsonPrimitive::Int | JsonPrimitive::Float => Ok((JsonPrimitive::Float, true)),
                other => Err(AnalyzerError::FunctionArgMismatch { name: fun.name.clone(), expected: "numeric".into(), got: vec![other] }),
            }
        } else {
            Err(AnalyzerError::FunctionArgMismatch { name: fun.name.clone(), expected: "AVG(arg)".into(), got: vec![] })
        }
    }
    fn create_accumulator(&self) -> Box<dyn Accumulator> { Box::new(AvgAcc { sum: 0.0, cnt: 0 }) }
}

struct AvgAcc { sum: f64, cnt: i64 }
impl Accumulator for AvgAcc {
    fn update(&mut self, args: &[Value]) -> Result<(), AnalyzerError> {
        let [v] = args else {
            return Err(AnalyzerError::FunctionArgMismatch { name: "AVG".into(), expected: "AVG(expr)".into(), got: vec![] })
        };
        match v {
            Value::Null => {}
            Value::Number(n) => {
                if let Some(i) = n.as_i64() { self.sum += i as f64; self.cnt += 1; }
                else if let Some(f) = n.as_f64() { self.sum += f; self.cnt += 1; }
                else { return Err(AnalyzerError::Other("AVG got non numeric number".into())); }
            }
            _ => return Err(AnalyzerError::Other("AVG got non numeric arg".into())),
        }
        Ok(())
    }
    fn finalize(&self) -> Value {
        if self.cnt == 0 { Value::Null }
        else {
            let avg = self.sum / (self.cnt as f64);
            serde_json::Number::from_f64(avg).map(Value::Number).unwrap_or(Value::Null)
        }
    }
}
