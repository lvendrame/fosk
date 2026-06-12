use serde_json::Value;

use crate::{
    JsonPrimitive,
    parser::{
        aggregators_helper::{Accumulator, AggregateImpl},
        analyzer::{AnalysisContext, AnalyzerError, TypeInference},
        ast::Function,
    },
};

pub struct MinImpl;
pub struct MaxImpl;

impl AggregateImpl for MinImpl {
    fn name(&self) -> &'static str {
        "min"
    }
    fn infer_type(
        &self,
        fun: &Function,
        ctx: &AnalysisContext,
    ) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        let (t, _n) = fun
            .args
            .first()
            .ok_or_else(|| AnalyzerError::FunctionArgMismatch {
                name: fun.name.clone(),
                expected: "MIN(arg)".into(),
                got: vec![],
            })
            .and_then(|a| TypeInference::infer_scalar(a, ctx))?;
        Ok((t, true))
    }
    fn create_accumulator(&self) -> Box<dyn Accumulator> {
        Box::new(ExtremaAcc::new_min())
    }
}
impl AggregateImpl for MaxImpl {
    fn name(&self) -> &'static str {
        "max"
    }
    fn infer_type(
        &self,
        fun: &Function,
        ctx: &AnalysisContext,
    ) -> Result<(JsonPrimitive, bool), AnalyzerError> {
        let (t, _n) = fun
            .args
            .first()
            .ok_or_else(|| AnalyzerError::FunctionArgMismatch {
                name: fun.name.clone(),
                expected: "MAX(arg)".into(),
                got: vec![],
            })
            .and_then(|a| TypeInference::infer_scalar(a, ctx))?;
        Ok((t, true))
    }
    fn create_accumulator(&self) -> Box<dyn Accumulator> {
        Box::new(ExtremaAcc::new_max())
    }
}

enum Mode {
    Min,
    Max,
}

struct ExtremaAcc {
    mode: Mode,
    current: Option<Value>,
}

impl ExtremaAcc {
    fn new_min() -> Self {
        Self {
            mode: Mode::Min,
            current: None,
        }
    }
    fn new_max() -> Self {
        Self {
            mode: Mode::Max,
            current: None,
        }
    }
    fn better(mode: &Mode, a: &Value, b: &Value) -> Result<bool, AnalyzerError> {
        use Value::*;
        let ord = match (a, b) {
            (Null, _) | (_, Null) => return Ok(false), // Nulls ignored by caller
            (Bool(x), Bool(y)) => x.cmp(y),
            (Number(x), Number(y)) => {
                // strict: keep kind; analyzer should prevent mixing
                match (x.as_i64(), y.as_i64(), x.as_f64(), y.as_f64()) {
                    (Some(ix), Some(iy), _, _) => ix.cmp(&iy),
                    (None, None, Some(fx), Some(fy)) => fx
                        .partial_cmp(&fy)
                        .ok_or_else(|| AnalyzerError::Other("NaN in MIN/MAX".into()))?,
                    _ => return Err(AnalyzerError::Other("MIN/MAX mixed numeric kinds".into())),
                }
            }
            (String(x), String(y)) => x.cmp(y),
            // Arrays/Objects: usually unsupported in SQL; keep strict and error
            (Array(_), _) | (Object(_), _) | (_, Array(_)) | (_, Object(_)) => {
                return Err(AnalyzerError::Other("MIN/MAX unsupported type".into()));
            }
            // different types
            _ => return Err(AnalyzerError::Other("MIN/MAX mixed types".into())),
        };
        Ok(match mode {
            Mode::Min => ord.is_gt(),
            Mode::Max => ord.is_lt(),
        })
    }
}

impl Accumulator for ExtremaAcc {
    fn update(&mut self, args: &[Value]) -> Result<(), AnalyzerError> {
        let [v] = args else {
            return Err(AnalyzerError::FunctionArgMismatch {
                name: "MIN/MAX".into(),
                expected: "MIN/MAX(expr)".into(),
                got: vec![],
            });
        };
        if matches!(v, Value::Null) {
            return Ok(());
        }
        match &mut self.current {
            None => {
                self.current = Some(v.clone());
            }
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

#[cfg(test)]
mod tests {
    use super::{MaxImpl, MinImpl};
    use crate::parser::aggregators_helper::AggregateImpl;
    use serde_json::json;

    #[test]
    fn min_and_max_accumulators_ignore_nulls_and_track_int_extrema() {
        let mut min = MinImpl.create_accumulator();
        let mut max = MaxImpl.create_accumulator();

        for value in [json!(null), json!(3), json!(1), json!(2)] {
            min.update(std::slice::from_ref(&value)).unwrap();
            max.update(&[value]).unwrap();
        }

        assert_eq!(min.finalize(), json!(1));
        assert_eq!(max.finalize(), json!(3));
    }

    #[test]
    fn min_and_max_accumulators_track_string_and_bool_extrema() {
        let mut string_min = MinImpl.create_accumulator();
        string_min.update(&[json!("carol")]).unwrap();
        string_min.update(&[json!("ada")]).unwrap();
        assert_eq!(string_min.finalize(), json!("ada"));

        let mut bool_max = MaxImpl.create_accumulator();
        bool_max.update(&[json!(false)]).unwrap();
        bool_max.update(&[json!(true)]).unwrap();
        assert_eq!(bool_max.finalize(), json!(true));
    }

    #[test]
    fn accumulator_returns_null_when_no_values_seen() {
        assert_eq!(MinImpl.create_accumulator().finalize(), json!(null));
        assert_eq!(MaxImpl.create_accumulator().finalize(), json!(null));
    }

    #[test]
    fn accumulator_rejects_bad_arg_count_and_unsupported_types() {
        let mut min = MinImpl.create_accumulator();
        assert!(min.update(&[]).is_err());
        assert!(min.update(&[json!([1, 2])]).is_ok());
        assert!(min.update(&[json!([3])]).is_err());

        let mut max = MaxImpl.create_accumulator();
        max.update(&[json!(1)]).unwrap();
        assert!(max.update(&[json!("bad")]).is_err());
    }
}
