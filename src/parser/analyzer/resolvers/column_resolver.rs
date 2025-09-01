use crate::parser::{analyzer::{AnalysisContext, AnalyzerError, ResolvedField}, ast::Column};

pub struct ColumnResolver;

impl ColumnResolver {
    pub fn qualify_column(col: &Column, ctx: &AnalysisContext) -> Result<(Column, ResolvedField), AnalyzerError> {
        match col {
            Column::WithCollection { collection, name } => {
                let coll_ref = ctx.collections.get(collection)
                    .ok_or_else(|| AnalyzerError::UnknownCollection(collection.clone()))?;
                let schema = ctx.schemas.schema_of(coll_ref)
                    .ok_or_else(|| AnalyzerError::UnknownCollection(coll_ref.clone()))?;
                let field_info = schema.get(name).ok_or_else(|| {
                    AnalyzerError::UnknownColumn {
                        name: format!("{}.{}", collection, name),
                        candidates: schema.fields.keys().cloned().collect()
                    }
                })?;

                Ok((col.clone(), ResolvedField {
                    collection: collection.clone(),
                    name: name.clone(),
                    ty: field_info.ty,
                    nullable: field_info.nullable
                }))
            }
            Column::Name { name } => {
                // search each visible collection’s schema for this column
                let mut matches: Vec<(String, ResolvedField)> = Vec::new();
                for (visible_coll, backing) in &ctx.collections {
                    if let Some(schema) = ctx.schemas.schema_of(backing) {
                        if let Some(field_info) = schema.get(name) {
                            matches.push((
                                visible_coll.clone(),
                                ResolvedField {
                                    collection: visible_coll.clone(),
                                    name: name.clone(),
                                    ty: field_info.ty,
                                    nullable: field_info.nullable
                                }
                            ));
                        }
                    }
                }
                match matches.len() {
                    0 => Err(AnalyzerError::UnknownColumn { name: name.clone(), candidates: vec![] }),
                    1 => {
                        let (collection, resolved_field) = matches.into_iter().next().unwrap();
                        Ok((
                            Column::WithCollection {
                                collection,
                                name: name.clone()
                            },
                            resolved_field
                        ))
                    }
                    _ => Err(AnalyzerError::AmbiguousColumn {
                        name: name.clone(),
                        matches: matches.into_iter().map(|(coll_name, rf)| (coll_name, rf.name)).collect()
                    }),
                }
            }
        }
    }
}
