use crate::parser::{Field, Projection};

#[derive(Debug, Default)]
pub struct TokenPosition {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Default)]
pub struct WordComparer {
    length: usize,
    word: Vec<char>,
}

impl WordComparer {
    pub fn new(word: &str) -> Self {
        Self {
            length: word.len(),
            word: word.to_uppercase().chars().collect(),
        }
    }

    pub fn compare(&self, parser: &ParserQuery) -> bool {
        let mut position = 0;
        while position < self.length {
            if self.word[position] != parser.query[parser.position + position].to_ascii_uppercase() {
                return false;
            }
            position += 1;
        }

        true
    }
}

#[derive(Debug)]
pub struct QueryComparers {
    pub select: WordComparer,
    pub alias: WordComparer,
    pub from: WordComparer,
    pub inner_join: WordComparer,
    pub left_join: WordComparer,
    pub right_join: WordComparer,
    pub on: WordComparer,
    pub criteria: WordComparer,
    pub group_by: WordComparer,
    pub having: WordComparer,
    pub order_by: WordComparer,
    pub and: WordComparer,
    pub or: WordComparer,
    pub equal: WordComparer,
    pub different_b: WordComparer,
    pub different_c: WordComparer,
    pub greater_than: WordComparer,
    pub greater_than_or_equal: WordComparer,
    pub less_than: WordComparer,
    pub less_than_or_equal: WordComparer,
    pub like: WordComparer,
    pub is_null: WordComparer,
    pub is_not_null: WordComparer,
}

impl Default for QueryComparers {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryComparers {
    pub fn new() -> Self {
        Self {
            select: WordComparer::new("SELECT "),
            alias: WordComparer::new("AS "),
            from: WordComparer::new("FROM "),
            inner_join: WordComparer::new("INNER JOIN "),
            left_join: WordComparer::new("LEFT JOIN "),
            right_join: WordComparer::new("RIGHT JOIN "),
            on: WordComparer::new("ON "),
            criteria: WordComparer::new("WHERE "),
            group_by: WordComparer::new("GROUP BY "),
            having: WordComparer::new("HAVING "),
            order_by: WordComparer::new("ORDER BY "),
            and: WordComparer::new("AND "),
            or: WordComparer::new("OR "),
            equal: WordComparer::new("= "),
            different_b: WordComparer::new("<> "),
            different_c: WordComparer::new("!= "),
            greater_than: WordComparer::new("> "),
            greater_than_or_equal: WordComparer::new(">= "),
            less_than: WordComparer::new("< "),
            less_than_or_equal: WordComparer::new("<= "),
            like: WordComparer::new("LIKE "),
            is_null: WordComparer::new("IS NULL "),
            is_not_null: WordComparer::new("IS NOT NULL "),
        }
    }
}

#[derive(Debug, Default, PartialEq)]
pub enum Phase {
    #[default]
    Projection,
    Collections,
    CollectionsOr,
    Inners,
    Constraints,
    Aggregates,
    Having,
    OrderBy
}

#[derive(Debug, Default)]
pub struct ParserQuery {
    position: usize,
    length: usize,
    query: Vec<char>,
    phase: Phase,
    original_query: String,
    token_position: TokenPosition,
    parentheses_depth: usize,
}

impl ParserQuery {
    pub fn new(query: &str) -> Self {
        Self {
            position: 0,
            length: query.len(),
            query: query.chars().collect(),
            original_query: query.to_string(),
            ..Default::default()
        }
    }

    pub fn current(&self) -> char {
        println!("{}", self.query[self.position]);
        self.query[self.position]
    }

    pub fn peek(&self, ahead: usize) -> char {
        self.query[self.position + ahead]
    }

    pub fn next(&mut self) {
        self.position += 1;
    }

    pub fn jump(&mut self, ahead: usize) {
        self.position += ahead;
    }

    pub fn parse(&mut self) -> Result<String, String> {
        let query_comparers = QueryComparers::new();
        while self.position < self.length {
            let a: String = self.parse_current(&query_comparers)?;
            self.next();
        }
        Ok("".into())
    }

    fn check_phase(&mut self, query_comparers: &QueryComparers) -> Result<(), String> {
        if self.phase == Phase::Projection {
            if query_comparers.select.compare(self) {
                self.jump(query_comparers.select.length);
            } else {
                return Err(format!("Invalid character '{}' at position {}", self.current(), self.position));
            }
        }

        if self.phase == Phase::Collections {
            if query_comparers.from.compare(self) {
                self.jump(query_comparers.from.length);
            } else {
                return Err(format!("Invalid character '{}' at position {}", self.current(), self.position));
            }
        }

        if query_comparers.inner_join.compare(self) || query_comparers.left_join.compare(self) ||
        query_comparers.right_join.compare(self) {
            // if collection.is_empty() {
                //     return Err(format!("Invalid character '{}' at position {}", self.current(), self.position));
                // }
            self.phase = Phase::Inners;
        }

        Ok(())
    }

    fn parse_current(&mut self, query_comparers: &QueryComparers) -> Result<String, String> {
        let current = self.current();
        if current.is_whitespace() || current == '\r' || current == '\n' {
            return Ok("".into());
        }

        self.check_phase(query_comparers)?;

        match self.phase {
            Phase::Projection => {self.parse_projection(query_comparers)?;},
            Phase::Collections => {self.parse_collections(query_comparers)?;},
            Phase::CollectionsOr => {self.parse_collections(query_comparers)?;},
            Phase::Constraints => todo!(),
            Phase::Aggregates => todo!(),
            Phase::Inners => todo!(),
            Phase::Having => todo!(),
            Phase::OrderBy => todo!(),
        };
        Ok("".into())
    }

    fn parse_projection(&mut self, query_comparers: &QueryComparers) -> Result<Projection, String> {
        self.parse_projection_fields(query_comparers)
    }

    fn parse_projection_fields(&mut self, query_comparers: &QueryComparers) -> Result<Projection, String> {
        let mut projection = Projection::default();
        while !query_comparers.from.compare(self) {
            let current = self.current();
            if char::is_whitespace(current) || current == ',' {
                self.next();
                continue;
            }
            if char::is_alphabetic(current) || current == '*' {
                let field = self.parse_projection_field(query_comparers)?;
                projection.fields.push(field);
                continue;
            } else {
                return Err(format!("Invalid character '{}' at position {}", current, self.position));
            }
            self.next();
        }

        self.phase = Phase::Collections;
        Ok(projection)
    }

    fn parse_projection_field(&mut self, query_comparers: &QueryComparers) -> Result<Field, String> {
        if self.current().is_ascii_digit() {
            return Err(format!("Invalid character '{}' at position {}", self.current(), self.position));
        }
        let initial_position = self.position;

        let mut pivot = self.position;
        let mut collection: Option<String> = None;
        let mut name: Option<String> = None;
        let mut alias: Option<String> = None;
        let mut args: Option<String> = None;
        let mut in_fn = false;
        let mut is_fn = false;
        let mut in_alias = false;


        while self.current() != ',' && !query_comparers.from.compare(self) {
            ///DEBUG
            let s_ch: String = self.current().to_string();
            let s_cur: String = self.query[pivot..=self.position].iter().collect();
            ///DEBUG

            let current = self.current();
            if current == '*' && !in_fn {
                self.next();
                let field = match collection {
                    Some(collection) => Field::CollectionAll(collection),
                    None => Field::All,
                };
                return Ok(field);
            }

            if current == '.' && !in_fn {
                collection = Some(self.query[pivot..self.position].iter().collect());
                self.next();
                pivot = self.position;
                continue;
            }

            if current == '(' {
                if name.is_some() || in_fn {
                    return Err(format!("Invalid character '{}' at position {}", current, self.position));
                }

                name = Some(self.query[pivot..self.position].iter().collect());
                in_fn = true;
                is_fn = true;
                self.next();
                pivot = self.position;
                continue;
            }

            if current == ')' {
                if !in_fn || args.is_some() {
                    return Err(format!("Invalid character '{}' at position {}", current, self.position));
                }
                args = Some(self.query[pivot..self.position].iter().collect());
                in_fn = false;
                self.next();
                pivot = self.position;
                continue;
            }

            if char::is_whitespace(current) {
                self.next();
                if query_comparers.alias.compare(self) {
                    if in_alias {
                        return Err(format!("Invalid character '{}' at position {}", current, self.position));
                    }
                    if name.is_none() {
                        name = Some(self.query[pivot..self.position-1].iter().collect());
                    }
                    self.jump(query_comparers.alias.length);
                    pivot = self.position;
                    in_alias = true;
                    continue;
                }

                if in_alias {
                    if alias.is_some() {
                        return Err(format!("Invalid character '{}' at position {}", current, self.position));
                    }
                    alias = Some(self.query[pivot..self.position].iter().collect());
                    self.next();
                    pivot = self.position;
                    in_alias = false;
                    continue;
                }

                continue;
            }
            self.next();
        }

        if in_alias {
            alias = Some(self.query[pivot..self.position].iter().collect());
        } else if name.is_none() {
            name = Some(self.query[pivot..self.position].iter().collect());
        }

        let field = match (collection, name, args, alias, is_fn) {
            (None, Some(name), None, None, false) => Field::Name(name),
            (None, Some(name), None, Some(alias), false) => Field::NameAlias(name, alias),
            (Some(collection), Some(name), None, None, false) => Field::CollectionName(collection, name),
            (Some(collection), Some(name), None, Some(alias), false) => Field::CollectionNameAlias(collection, name, alias),
            (None, Some(function), Some(args), None, true) => Field::Function(function, args),
            (None, Some(function), Some(args), Some(alias), true) => Field::FunctionAlias(function, args, alias),
            _ => return Err(format!("Invalid field '{}' at position {}", String::from_iter(self.query[pivot..self.position].iter()), initial_position)),
        };

        Ok(field)
    }

    fn parse_collections(&mut self, query_comparers: &QueryComparers) -> Result<String, String> {
        if self.phase == Phase::Collections {
            if !query_comparers.from.compare(self) {
                return Err(format!("Invalid character '{}' at position {}", self.current(), self.position));
            }
            self.jump(query_comparers.from.length);
        }

        let mut collection = "".to_string();

        // while self.position < self.length {

        // }

        Ok("".into())
    }

}

#[cfg(test)]
mod test {
    use crate::parser::ParserQuery;

    #[test]
    pub fn dummy() {
        let query = r#"SELECT b.*, a.full_name as name, COUNT(*) as TotBy, *, AVG(a.sum), one as alias, field, other_field
FROM TableA A
INNER JOIN TableB B ON A.id = B.id
INNER JOIN (query...) Q ON Q.id = B.q_id
WHERE A.Age > 16 AND (B.city = 'Porto' OR B.city like "Matosinhos")
GROUP BY a.full_name
HAVING COUNT(*) > 3
ORDER BY b.description DESC"#;

        let mut parser = ParserQuery::new(query);

        let result = parser.parse();
        println!("{:?}", result);
    }

}

// QueryToken
//     ProjectionToken
//         ProjectionFieldToken
//             - FieldNameToken
//             - FieldTableToken
//             - FieldAliasToken
//     CollectionToken
//         - CollectionNameToken
//         - CollectionAlias
//     CollectionJoinToken
//         - CollectionNameToken
//         - CollectionAlias
//         JoinConstraintToken
//             LeftSideToken
//             OperatorToken
//             RightSideToken
//     CriteriaToken
//         LeftSideToken
//         OperatorToken
//         RightSideToken
//     AggregatorToken
//         AggregatorFieldToken
//     AggregatorConstraintToken
//         ...Constraints
//     OrderToken
//         OrderFieldTableToken
//         OrderFieldNameToken
//         OrderDirectionToken

