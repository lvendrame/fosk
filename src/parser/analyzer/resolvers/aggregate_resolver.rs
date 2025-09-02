use std::collections::HashSet;

use crate::parser::{analyzer::ColumnKey, ast::{Predicate, ScalarExpr}};

pub struct AggregateResolver;

impl AggregateResolver {
    pub fn is_aggregate_name(name: &str) -> bool {
        matches!(name.to_ascii_lowercase().as_str(), "count" | "sum" | "avg" | "min" | "max")
    }

    pub fn contains_aggregate(expr: &ScalarExpr) -> bool {
        match expr {
            ScalarExpr::Function(f) if Self::is_aggregate_name(&f.name) => true,
            ScalarExpr::Function(f) => f.args.iter().any(Self::contains_aggregate),
            _ => false,
        }
    }

    pub fn uses_only_group_by(
        expr: &ScalarExpr,
        group: &HashSet<ColumnKey>,
        inside_agg_arg: bool,
    ) -> bool {
        match expr {
            ScalarExpr::Literal(_) => true,
            ScalarExpr::Column(c) => {
                if inside_agg_arg { true } else {
                    // must be qualified before calling this check
                    group.contains(&ColumnKey::of(c))
                }
            }
            ScalarExpr::Function(f) => {
                if Self::is_aggregate_name(&f.name) {
                    // args are inside aggregate
                    f.args.iter().all(|a| Self::uses_only_group_by(a, group, true))
                } else {
                    // scalar function: preserve flag
                    f.args.iter().all(|a| Self::uses_only_group_by(a, group, inside_agg_arg))
                }
            }
            ScalarExpr::WildCard | ScalarExpr::WildCardWithCollection(_) => false, // should have been expanded/qualified already
        }
    }

    pub fn predicate_contains_aggregate(predicate: &Predicate) -> bool {
        match predicate {
            Predicate::And(predicates) | Predicate::Or(predicates) => predicates.iter().any(Self::predicate_contains_aggregate),
            Predicate::Compare { left, right, .. } => Self::contains_aggregate(left) || Self::contains_aggregate(right),
            Predicate::IsNull { expr, .. } => Self::contains_aggregate(expr),
            Predicate::InList { expr, list, .. } => Self::contains_aggregate(expr) || list.iter().any(Self::contains_aggregate),
            Predicate::Like { expr, pattern, .. } => Self::contains_aggregate(expr) || Self::contains_aggregate(pattern),
            Predicate::Const3(_) => false,
        }
    }

    pub fn predicate_uses_only_group_by_or_agg(predicate: &Predicate, group: &HashSet<ColumnKey>) -> bool {
        match predicate {
            Predicate::And(v) | Predicate::Or(v) => v.iter()
                .all(|x| Self::predicate_uses_only_group_by_or_agg(x, group)),
            Predicate::Compare { left, right, .. } =>
                Self::uses_only_group_by(left, group, false) && Self::uses_only_group_by(right, group, false),
            Predicate::IsNull { expr, .. } => Self::uses_only_group_by(expr, group, false),
            Predicate::InList { expr, list, .. } =>
                Self::uses_only_group_by(expr, group, false) && list.iter().all(|e| Self::uses_only_group_by(e, group, false)),
            Predicate::Like { expr, pattern, .. } =>
                Self::uses_only_group_by(expr, group, false) && Self::uses_only_group_by(pattern, group, false),
            Predicate::Const3(_) => true,
        }
    }
}
