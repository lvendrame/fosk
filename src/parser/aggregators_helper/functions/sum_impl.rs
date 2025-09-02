use serde_json::Value;

use crate::{parser::{aggregators_helper::{Accumulator, AggregateImpl}, analyzer::{AnalysisContext, AnalyzerError}, ast::Function}, JsonPrimitive};

pub struct SumImpl;
impl AggregateImpl for SumImpl {
    fn name(&self) -> &'static str { "sum" }
    fn infer_type(&self, fun: &Function, ctx: &AnalysisContext) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        // Reuse your analyzer logic: only numeric allowed; nullable true.
        if let Some(arg) = fun.args.first() {
            let (t, _n) = crate::parser::analyzer::TypeInference::infer_scalar(arg, ctx)?;
            match t {
                JsonPrimitive::Int   => Ok((JsonPrimitive::Int, true)),
                JsonPrimitive::Float => Ok((JsonPrimitive::Float, true)),
                other => Err(AnalyzerError::FunctionArgMismatch { name: fun.name.clone(), expected: "numeric".into(), got: vec![other] }),
            }
        } else {
            Err(AnalyzerError::FunctionArgMismatch { name: fun.name.clone(), expected: "SUM(arg)".into(), got: vec![] })
        }
    }
    fn create_accumulator(&self) -> Box<dyn Accumulator> { Box::new(SumAcc::Empty) }
}

// Track the concrete numeric kind seen first.
enum SumAcc {
    Empty,
    Int(i128),
    Float(f64),
}
impl Accumulator for SumAcc {
    fn update(&mut self, args: &[Value]) -> Result<(), AnalyzerError> {
        let [v] = args else {
            return Err(AnalyzerError::FunctionArgMismatch { name: "SUM".into(), expected: "SUM(expr)".into(), got: vec![] })
        };
        if matches!(v, Value::Null) { return Ok(()); }

        match (&mut *self, v) {
            (SumAcc::Empty, Value::Number(n)) => {
                if let Some(i) = n.as_i64() {
                    *self = SumAcc::Int(i as i128);
                } else if let Some(f) = n.as_f64() {
                    *self = SumAcc::Float(f);
                } else {
                    return Err(AnalyzerError::Other("SUM got non numeric number".into()));
                }
            }
            (SumAcc::Int(acc), Value::Number(n)) => {
                if let Some(i) = n.as_i64() { *acc += i as i128; }
                else if let Some(_f) = n.as_f64() {
                    return Err(AnalyzerError::Other("SUM received float for INT aggregation".into()));
                } else { return Err(AnalyzerError::Other("SUM got non numeric number".into())); }
            }
            (SumAcc::Float(acc), Value::Number(n)) => {
                if let Some(i) = n.as_i64() { *acc += i as f64; }
                else if let Some(f) = n.as_f64() { *acc += f; }
                else { return Err(AnalyzerError::Other("SUM got non numeric number".into())); }
            }
            (_, other) => return Err(AnalyzerError::Other(format!("SUM got non numeric arg: {:?}", other))),
        }
        Ok(())
    }
    fn finalize(&self) -> Value {
        match self {
            SumAcc::Empty       => Value::Null, // SQL SUM over all NULLs -> NULL
            SumAcc::Int(i)      => Value::Number(serde_json::Number::from(*i as i64)), // safe if your ints fit i64; otherwise customize
            SumAcc::Float(f)    => serde_json::Number::from_f64(*f).map(Value::Number).unwrap_or(Value::Null),
        }
    }
}
