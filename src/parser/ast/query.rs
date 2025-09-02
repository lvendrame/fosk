// SELECT b.*, a.full_name as name, COUNT(*) as TotBy, * V
// FROM TableA A V
// INNER JOIN TableB B ON A.id = B.id V
// INNER JOIN (query...) Q ON Q.id = B.q_id V
// WHERE A.Age > 16 AND (B.city = 'Porto' OR B.city like "Matosinhos") V
// GROUP BY a.full_name V
// HAVING COUNT(*) > 3
// ORDER BY b.description DESC V

use crate::parser::{ast::{Collection, CollectionsParser, Column, GroupBy, HavingParser, Identifier, Join, LimitAndOffsetParser, OrderBy, Predicate, ProjectionParser, WhereParser}, ParseError, Phase, QueryParser};

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Query {
    pub projection: Vec<Identifier>,
    pub collections: Vec<Collection>,
    pub joins: Vec<Join>,
    pub criteria: Option<Predicate>,
    pub group_by: Vec<Column>,
    pub having: Option<Predicate>,
    pub order_by: Vec<OrderBy>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl Query {
    pub fn parse(parser: &mut QueryParser) -> Result<Self, ParseError> {
        parser.next_non_whitespace();

        let mut query = Query::default();

        while parser.phase != Phase::EOF {
            match parser.phase {
                Phase::Projection => query.projection = ProjectionParser::parse(parser)?,
                Phase::Collections => query.collections = CollectionsParser::parse(parser)?,
                Phase::Joins => query.joins = Join::parse(parser)?,
                Phase::Criteria => query.criteria = Some(WhereParser::parse(parser)?),
                Phase::Aggregates => query.group_by = GroupBy::parse(parser)?,
                Phase::Having => query.having = Some(HavingParser::parse(parser)?),
                Phase::OrderBy => query.order_by = OrderBy::parse(parser)?,
                Phase::LimitAndOffset => {
                    let (limit, offset) = LimitAndOffsetParser::parse(parser)?;
                    query.limit = limit;
                    query.offset = offset;
                },
                Phase::EOF => todo!(),
            }
        }

        Ok(query)
    }
}

impl TryFrom<&str> for Query {
    type Error = ParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let mut parser = QueryParser::new(value);
        Query::parse(&mut parser)
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::ast::Query;

    #[test]
    pub fn test_query() {
        let text = r#"
SELECT b.*, a.full_name as name, COUNT(*) as TotBy, *
FROM TableA A, OtherTable
INNER JOIN TableB B ON A.id = B.id
LEFT JOIN TableC C ON C.id = B.q_id
WHERE A.Age > 16 AND (B.city = 'Porto' OR B.city like "Matosinhos")
GROUP BY a.full_name
HAVING COUNT(*) > 3
ORDER BY b.description DESC
OFFSET 60
LIMIT 20
        "#;

        let query = Query::try_from(text).expect("Failed to parse predicate");

        assert_eq!(query.projection.len(), 4);
        assert_eq!(query.collections.len(), 2);
        assert_eq!(query.joins.len(), 2);
        assert!(query.criteria.is_some());
        assert_eq!(query.group_by.len(), 1);
        assert!(query.having.is_some());
        assert_eq!(query.order_by.len(), 1);
        assert_eq!(query.offset.unwrap(), 60);
        assert_eq!(query.limit.unwrap(), 20);
    }
}
