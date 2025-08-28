use crate::parser::{Collection, ProjectionField, Query};

#[derive(Debug, Default)]
pub struct TokenPosition {
    pub pivot: usize,
    pub end: usize,
}

#[derive(Debug, Default)]
pub struct WordComparer {
    pub length: usize,
    pub word: Vec<char>,
    whitespace_postfix: bool,
    break_line_postfix: bool,
    full_block_delimiter_postfix: bool,
    eof: bool,
}

impl WordComparer {
    pub fn new(word: &str) -> Self {
        Self {
            length: word.len(),
            word: word.to_uppercase().chars().collect(),
            whitespace_postfix: false,
            break_line_postfix: false,
            full_block_delimiter_postfix: false,
            eof: false,
        }
    }

    pub fn reach_eof(&self, parser: &QueryParser) -> bool {
        parser.position + self.length >= parser.length
    }

    pub fn is_block_delimiter(ch: char) -> bool {
        ch.is_ascii_whitespace()
    }

    pub fn is_any_delimiter(ch: char) -> bool {
        ch == ',' || ch == '(' || ch == ')' || ch == '.' || Self::is_block_delimiter(ch)
    }

    pub fn is_break_line(ch: char) -> bool {
        ch == '\r' || ch == '\n'
    }

    pub fn is_current_block_delimiter(parser: &QueryParser) -> bool {
        Self::is_block_delimiter(parser.current())
    }

    pub fn is_current_breal_line(parser: &QueryParser) -> bool {
        Self::is_break_line(parser.current())
    }

    pub fn compare(&self, parser: &QueryParser) -> bool {
        let mut position = 0;
        while position < self.length && (parser.position + position) < parser.length {
            if self.word[position] != parser.text_v[parser.position + position].to_ascii_uppercase() {
                return false;
            }
            position += 1;
        }

        if self.reach_eof(parser) {
             return self.eof;
        }

        let next = parser.text_v[parser.position + position];

        if self.full_block_delimiter_postfix && !Self::is_any_delimiter(next) {
            return false;
        }

        if self.whitespace_postfix && !Self::is_block_delimiter(next) {
            return false;
        }

        if self.break_line_postfix && Self::is_break_line(next) {
            return false;
        }

        true
    }

    pub fn with_eof(mut self) -> Self { self.eof = true; self }
    pub fn with_whitespace_postfix(mut self) -> Self { self.whitespace_postfix = true; self }
    pub fn with_break_line_postfix(mut self) -> Self { self.break_line_postfix = true; self }
    pub fn with_any_delimiter_postfix(mut self) -> Self { self.full_block_delimiter_postfix = true; self }

    pub fn compare_with_block_delimiter(&self, parser: &QueryParser) -> bool {
        self.compare(parser) &&
            (self.reach_eof(parser) || WordComparer::is_any_delimiter(parser.peek(self.length)))
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
    pub not_equal_b: WordComparer, // basic
    pub not_equal_c: WordComparer, // c
    pub greater_than: WordComparer,
    pub greater_than_or_equal: WordComparer,
    pub less_than: WordComparer,
    pub less_than_or_equal: WordComparer,
    pub like: WordComparer,
    pub is_null: WordComparer,
    pub is_not_null: WordComparer,
    pub r#in: WordComparer,
    pub not_in: WordComparer,
    pub b_true: WordComparer,
    pub b_false: WordComparer,
    pub null: WordComparer,
}

impl Default for QueryComparers {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryComparers {
    pub fn new() -> Self {
        Self {
            select: WordComparer::new("SELECT").with_whitespace_postfix(),
            alias: WordComparer::new("AS").with_whitespace_postfix(),
            from: WordComparer::new("FROM").with_whitespace_postfix(),
            inner_join: WordComparer::new("INNER JOIN").with_whitespace_postfix(),
            left_join: WordComparer::new("LEFT JOIN").with_whitespace_postfix(),
            right_join: WordComparer::new("RIGHT JOIN").with_whitespace_postfix(),
            on: WordComparer::new("ON").with_whitespace_postfix(),
            criteria: WordComparer::new("WHERE").with_whitespace_postfix(),
            group_by: WordComparer::new("GROUP BY").with_whitespace_postfix(),
            having: WordComparer::new("HAVING").with_whitespace_postfix(),
            order_by: WordComparer::new("ORDER BY").with_whitespace_postfix(),
            and: WordComparer::new("AND").with_whitespace_postfix(),
            or: WordComparer::new("OR").with_whitespace_postfix(),
            equal: WordComparer::new("=").with_whitespace_postfix(),
            not_equal_b: WordComparer::new("<>").with_whitespace_postfix(),
            not_equal_c: WordComparer::new("!=").with_whitespace_postfix(),
            greater_than: WordComparer::new(">").with_whitespace_postfix(),
            greater_than_or_equal: WordComparer::new(">=").with_whitespace_postfix(),
            less_than: WordComparer::new("<").with_whitespace_postfix(),
            less_than_or_equal: WordComparer::new("<=").with_whitespace_postfix(),
            like: WordComparer::new("LIKE").with_whitespace_postfix(),
            is_null: WordComparer::new("IS NULL").with_whitespace_postfix(),
            is_not_null: WordComparer::new("IS NOT NULL").with_whitespace_postfix(),
            r#in: WordComparer::new("IN").with_whitespace_postfix(),
            not_in: WordComparer::new("NOT IN").with_whitespace_postfix(),
            b_true: WordComparer::new("TRUE").with_any_delimiter_postfix().with_eof(),
            b_false: WordComparer::new("FALSE").with_any_delimiter_postfix().with_eof(),
            null: WordComparer::new("NULL").with_any_delimiter_postfix().with_eof(),
        }
    }
}

#[derive(Debug, Default, PartialEq, PartialOrd)]
pub enum Phase {
    #[default]
    Projection = 0,
    Collections = 1,
    Joins = 2,
    Criteria = 3,
    Aggregates = 4,
    Having = 5,
    OrderBy = 6
}

#[derive(Debug, Default)]
pub struct QueryParser {
    pub position: usize,
    pub length: usize,
    pub text_v: Vec<char>,
    pub phase: Phase,
    pub text: String,
    pub token_position: TokenPosition,
    pub parentheses_depth: usize,
    pub query: Query,

    pub comparers: QueryComparers,

    dbg: String,
}

impl QueryParser {
    pub fn new(query: &str) -> Self {
        Self {
            position: 0,
            length: query.len(),
            text_v: query.chars().collect(),
            text: query.to_string(),
            comparers: QueryComparers::new(),
            dbg: query.chars().next().unwrap_or('\0').to_string(),
            ..Default::default()
        }
    }

    pub fn eof(&self) -> bool {
        self.position >= self.length
    }

    pub fn current(&self) -> char {
        if self.position < self.length {
            return self.text_v[self.position];
        }

        '\0'
    }

    pub fn peek(&self, ahead: usize) -> char {
        self.text_v[self.position + ahead]
    }

    pub fn next(&mut self) {
        self.position += 1;

        self.dbg = self.current().to_string();
    }

    pub fn next_non_whitespace(&mut self) {
        while self.current().is_whitespace() {
            self.next();
        }

        self.dbg = self.current().to_string();
    }

    pub fn jump(&mut self, ahead: usize) {
        if self.position + ahead < self.length {
            self.position += ahead;
        } else {
            self.position = self.length - 1;
        }

        self.dbg = self.current().to_string();
    }

    pub fn text_from_range(&self, start: usize, end: usize) -> String {
        let mut end = end;
        if end > self.length {
            end = self.length;
        }
        self.text_v[start..end].iter().collect()
    }

    pub fn text_from_pivot(&self, pivot: usize) -> String {
        self.text_from_range(pivot, self.position)
    }

    pub fn parse(&mut self) -> Result<String, String> {
        while self.position < self.length - 1 {
            self.parse_current()?;
            // self.next();
        }
        Ok("".into())
    }

    pub fn check_next_phase(&mut self) -> bool {
        // Projection = 0,
        // Collections = 1,
        // Inners = 2,
        // Criteria = 3,
        // Aggregates = 4,
        // Having = 5,
        // OrderBy = 6
        if self.phase < Phase::OrderBy && self.comparers.order_by.compare(self) {
            self.phase = Phase::OrderBy;
            return true;
        }

        if self.phase < Phase::Having && self.comparers.having.compare(self) {
            self.phase = Phase::Having;
            return true;
        }

        if self.phase < Phase::Aggregates && self.comparers.group_by.compare(self) {
            self.phase = Phase::Aggregates;
            return true;
        }

        if self.phase < Phase::Criteria && self.comparers.criteria.compare(self) {
            self.phase = Phase::Criteria;
            return true;
        }

        if self.phase <= Phase::Joins &&
            (self.comparers.inner_join.compare(self) || self.comparers.left_join.compare(self) || self.comparers.right_join.compare(self)) {
            self.phase = Phase::Joins;
            return true;
        }

        if self.phase < Phase::Collections && self.comparers.from.compare(self) {
            self.phase = Phase::Collections;
            return true;
        }

        false
    }

    fn check_phase(&mut self) -> Result<(), String> {
        if self.phase == Phase::Projection {
            if self.comparers.select.compare(self) {
                self.jump(self.comparers.select.length);
            } else {
                return Err(format!("Invalid character '{}' at position {}", self.current(), self.position));
            }
        }

        if self.phase == Phase::Collections {
            if self.comparers.from.compare(self) {
                self.jump(self.comparers.from.length);
            } else if self.query.collections.is_empty() {
                return Err(format!("Invalid character '{}' at position {}", self.current(), self.position));
            }
        }

        if self.comparers.inner_join.compare(self) || self.comparers.left_join.compare(self) ||
        self.comparers.right_join.compare(self) {
            // if collection.is_empty() {
                //     return Err(format!("Invalid character '{}' at position {}", self.current(), self.position));
                // }
            self.phase = Phase::Joins;
        }

        Ok(())
    }

    fn parse_current(&mut self) -> Result<(), String> {
        let current = self.current();
        if current.is_whitespace() || current == '\r' || current == '\n' {
            self.next();
            return Ok(());
        }

        self.check_phase()?;

        match self.phase {
            Phase::Projection => {
                self.query.projection_fields = self.parse_projection()?;
            },
            Phase::Collections => {
                let coll =  self.parse_collection()?;
                self.query.collections.push(coll);
            },
            Phase::Joins => {self.next();},
            Phase::Criteria => {},
            Phase::Aggregates => {},
            Phase::Having => {},
            Phase::OrderBy => {},
        };

        Ok(())
    }

    fn parse_projection(&mut self) -> Result<Vec<ProjectionField>, String> {
        let mut fields: Vec<ProjectionField> = vec![];

        while !self.comparers.from.compare(self) {
            let current = self.current();
            if char::is_whitespace(current) || current == ',' {
                self.next();
                continue;
            }
            if char::is_alphabetic(current) || current == '*' {
                let field = self.parse_projection_field()?;
                fields.push(field);
                continue;
            } else {
                return Err(format!("Invalid character '{}' at position {}", current, self.position));
            }
        }

        self.phase = Phase::Collections;
        Ok(fields)
    }

    fn parse_projection_field(&mut self) -> Result<ProjectionField, String> {
        if self.current().is_ascii_digit() {
            return Err(format!("Invalid character '{}' at position {}", self.current(), self.position));
        }
        let initial_position = self.position;

        let mut pivot = self.position;
        let mut collection: Option<String> = None;
        let mut name: Option<String> = None;
        let mut alias: Option<String> = None;
        let mut args: Vec<String> = vec![];
        let mut in_fn = false;
        let mut is_fn = false;
        let mut in_alias = false;


        while (self.current() != ',' || in_fn) && !self.comparers.from.compare(self) {
            let current = self.current();
            if current == '*' && !in_fn {
                self.next();
                let field = match collection {
                    Some(collection) => ProjectionField::CollectionWildcard { collection },
                    None => ProjectionField::Wildcard,
                };
                return Ok(field);
            }

            if current == '.' && !in_fn {
                collection = Some(self.text_v[pivot..self.position].iter().collect());
                self.next();
                pivot = self.position;
                continue;
            }

            if current == '(' {
                if name.is_some() || in_fn {
                    return Err(format!("Invalid character '{}' at position {}", current, self.position));
                }

                name = Some(self.text_v[pivot..self.position].iter().collect());
                in_fn = true;
                is_fn = true;
                self.next();
                pivot = self.position;
                continue;
            }

            if current == ')' || current == ',' {
                if !in_fn {
                    return Err(format!("Invalid character '{}' at position {}", current, self.position));
                }
                args.push(self.text_v[pivot..self.position].iter().collect());
                in_fn = current == ',';
                self.next();
                pivot = self.position;
                continue;
            }

            if WordComparer::is_current_block_delimiter(self) {
                self.next();
                if self.comparers.alias.compare(self) {
                    if in_alias {
                        return Err(format!("Invalid character '{}' at position {}", current, self.position));
                    }
                    if name.is_none() {
                        name = Some(self.text_v[pivot..self.position-1].iter().collect());
                    }
                    self.jump(self.comparers.alias.length);
                    pivot = self.position;
                    in_alias = true;
                    continue;
                }

                if in_alias {
                    if alias.is_some() {
                        return Err(format!("Invalid character '{}' at position {}", current, self.position));
                    }
                    alias = Some(self.text_v[pivot..self.position].iter().collect());
                    self.next();
                    pivot = self.position;
                    in_alias = false;
                    continue;
                }

                if in_fn {
                    pivot = self.position;
                }

                continue;
            }
            self.next();
        }

        if in_alias {
            alias = Some(self.text_v[pivot..self.position].iter().collect());
        } else if name.is_none() {
            name = Some(self.text_v[pivot..self.position].iter().collect());
        }

        let field = match (collection, name, !args.is_empty(), alias, is_fn) {
            (None, Some(name), false, None, false) => ProjectionField::Name{ name },
            (None, Some(name), false, Some(alias), false) => ProjectionField::NameAlias{ name, alias },
            (Some(collection), Some(name), false, None, false) => ProjectionField::CollectionName{ collection, name },
            (Some(collection), Some(name), false, Some(alias), false) => ProjectionField::CollectionNameAlias{ collection, name, alias },
            (None, Some(name), true, None, true) => ProjectionField::Function{ name, args },
            (None, Some(name), true, Some(alias), true) => ProjectionField::FunctionAlias{ name, args, alias },
            _ => return Err(format!("Invalid field '{}' at position {}", String::from_iter(self.text_v[pivot..self.position].iter()), initial_position)),
        };

        Ok(field)
    }

    fn parse_collection(&mut self) -> Result<Collection, String> {

        let mut pivot  = self.position;
        let mut collection: Option<String> = None;

        while self.position < self.length {
            let current = self.current();
            if WordComparer::is_current_block_delimiter(self) {
                let end = self.position;
                self.next();

                let next_phase = self.check_collection_next_phase();

                if next_phase != Phase::Collections {
                    let next: String = self.text_v[pivot..end].iter().collect();
                    let coll =  match collection {
                        Some(name) => Collection::NameAlias(name, next),
                        None => Collection::Name(next),
                    };

                    self.phase = next_phase;
                    return Ok(coll);
                } else {
                    collection = Some(self.text_v[pivot..end].iter().collect());
                    pivot = self.position;
                    continue;
                }
            }

            if current == ',' {
                let next: String = self.text_v[pivot..self.position].iter().collect();
                let coll =  match collection {
                    Some(name) => Collection::NameAlias(name, next),
                    None => Collection::Name(next),
                };

                self.next();
                return Ok(coll);
            }

            self.next();
        }

        Err("".into())
    }

    fn check_collection_next_phase(&mut self) -> Phase {
        if self.comparers.inner_join.compare(self) ||
            self.comparers.left_join.compare(self) ||
            self.comparers.right_join.compare(self) {
            return Phase::Joins;
        } else if self.comparers.criteria.compare(self) {
            return Phase::Criteria;
        } else if self.comparers.group_by.compare(self) {
            return Phase::Aggregates;
        } else if self.comparers.order_by.compare(self) {
            return Phase::OrderBy;
        }
        Phase::Collections
    }

}

#[cfg(test)]
mod test {
    use crate::parser::*;

    #[test]
    pub fn dummy() {
        let query = r#"SELECT b.*, a.full_name as name, COUNT(*) as TotBy, *, AVG(a.sum), MyFn(a.sum, 2, 3), one as alias, field, other_field
FROM TableA a, TableB b, TableC,
     TableD
INNER JOIN TableB B ON A.id = B.id
INNER JOIN (query...) Q ON Q.id = B.q_id
WHERE A.Age > 16 AND (B.city = 'Porto' OR B.city like "Matosinhos")
GROUP BY a.full_name
HAVING COUNT(*) > 3
ORDER BY b.description DESC"#;

        let mut parser = QueryParser::new(query);

        let result = parser.parse();
        println!("{:?}", parser.query);
    }

    #[test]
    pub fn test_projection_simple() {
        let query =
            "SELECT b.*, a.full_name as name, COUNT(*) as TotBy, *, AVG(a.sum), MyFn(a.sum, 2, 3), one as alias, field, other_field FROM ";

        let mut parser = QueryParser::new(query);

        let _ = parser.check_phase();

        let result = parser.parse_projection();

        assert!(result.is_ok());
        let result = result.unwrap();

        assert_eq!(result.len(), 9);

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

