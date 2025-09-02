use serde_json::Value;

use crate::{parser::{aggregators_helper::{Accumulator, AggregateImpl}, analyzer::{AnalysisContext, AnalyzerError, TypeInference}, ast::Function}, JsonPrimitive};

pub struct MinImpl;
pub struct MaxImpl;

impl AggregateImpl for MinImpl {
    fn name(&self) -> &'static str { "min" }
    fn infer_type(&self, fun: &Function, ctx: &AnalysisContext) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        let (t, _n) = fun.args.first()
            .ok_or_else(|| AnalyzerError::FunctionArgMismatch { name: fun.name.clone(), expected: "MIN(arg)".into(), got: vec![] })
            .and_then(|a| TypeInference::infer_scalar(a, ctx))?;
        Ok((t, true))
    }
    fn create_accumulator(&self) -> Box<dyn Accumulator> { Box::new(ExtremaAcc::new_min()) }
}
impl AggregateImpl for MaxImpl {
    fn name(&self) -> &'static str { "max" }
    fn infer_type(&self, fun: &Function, ctx: &AnalysisContext) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        let (t, _n) = fun.args.first()
            .ok_or_else(|| AnalyzerError::FunctionArgMismatch { name: fun.name.clone(), expected: "MAX(arg)".into(), got: vec![] })
            .and_then(|a| TypeInference::infer_scalar(a, ctx))?;
        Ok((t, true))
    }
    fn create_accumulator(&self) -> Box<dyn Accumulator> { Box::new(ExtremaAcc::new_max()) }
}

enum Mode { Min, Max }

struct ExtremaAcc {
    mode: Mode,
    current: Option<Value>,
}

impl ExtremaAcc {
    fn new_min() -> Self { Self { mode: Mode::Min, current: None } }
    fn new_max() -> Self { Self { mode: Mode::Max, current: None } }
    fn better(mode: &Mode, a: &Value, b: &Value) -> Result<bool, AnalyzerError> {
        use Value::*;
        let ord = match (a, b) {
            (Null, _) | (_, Null) => return Ok(false), // Nulls ignored by caller
            (Bool(x),  Bool(y))  => x.cmp(y),
            (Number(x), Number(y)) => {
                // strict: keep kind; analyzer should prevent mixing
                match (x.as_i64(), y.as_i64(), x.as_f64(), y.as_f64()) {
                    (Some(ix), Some(iy), _, _) => ix.cmp(&iy),
                    (None, None, Some(fx), Some(fy)) => fx.partial_cmp(&fy).ok_or_else(|| AnalyzerError::Other("NaN in MIN/MAX".into()))?,
                    _ => return Err(AnalyzerError::Other("MIN/MAX mixed numeric kinds".into())),
                }
            }
            (String(x), String(y)) => x.cmp(y),
            // Arrays/Objects: usually unsupported in SQL; keep strict and error
            (Array(_), _) | (Object(_), _) | (_, Array(_)) | (_, Object(_)) =>
                return Err(AnalyzerError::Other("MIN/MAX unsupported type".into())),
            // different types
            _ => return Err(AnalyzerError::Other("MIN/MAX mixed types".into())),
        };
        Ok(match mode { Mode::Min => ord.is_gt(), Mode::Max => ord.is_lt() })
    }
}

impl Accumulator for ExtremaAcc {
    fn update(&mut self, args: &[Value]) -> Result<(), AnalyzerError> {
        let [v] = args else {
            return Err(AnalyzerError::FunctionArgMismatch { name: "MIN/MAX".into(), expected: "MIN/MAX(expr)".into(), got: vec![] })
        };
        if matches!(v, Value::Null) { return Ok(()); }
        match &mut self.current {
            None => { self.current = Some(v.clone()); }
            Some(cur) => {
                if Self::better(&self.mode, cur, v)? {
                    *cur = v.clone();
                }
            }
        }
        Ok(())
    }
    fn finalize(&self) -> Value {
        self.current.clone().unwrap_or(Value::Null)
    }
}
